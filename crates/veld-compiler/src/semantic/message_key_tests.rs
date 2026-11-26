//! Message keys (`spec/platform.md` section 22): the key of every kind of rule, and which keys a
//! member's catalog may use.

use crate::testing::ir as build;
use crate::signature::message_keys as keys;
use veld_ir::Ir;

const SRC: &str = r#"
enum Status {
  pending, paid, cancelled
  flow pending -> paid
  flow pending -> cancelled
  final paid, cancelled
}

model User { id: uuid
  suspended: bool
  banned: bool }
principal User

model Booking {
  id: uuid
  status: Status = .pending
  @version
  revision: long
  start: datetime
  end: datetime
  rule endAfterStart: end > start
    message "A booking must end after it starts"
  rule silent: revision >= 0
}

model Payment { amount: decimal }
error AmountMismatch 422 { expected: decimal }

policy Manage(user: User, booking: Booking) {
  permit when true
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
  forbid when user.banned
  forbid when user.suspended and user.banned
    message "Banned and suspended"
}

@auth(bearer)
service Bookings at "/bookings" {
  post "/{id}/pay" pay(id: uuid, body: Payment) -> Booking | AmountMismatch {
    fact booking: Booking = id
    allow Manage(user, booking)
    transition booking.status: pending -> paid
    require notZero: body.amount > 0
      message "The amount must be positive"
    require body.amount < 1000000
      message "Too much"
    require body.amount != 13
    ensure result.revision >= 0
      message "Revision never goes down"
    ensure result.revision >= 1
  }
}
"#;

fn ir() -> Ir {
    build(SRC)
}

fn id_of(ir: &Ir, pick: impl Fn(&veld_ir::EndpointRules) -> Option<String>) -> String {
    ir.services[0]
        .endpoints
        .iter()
        .find_map(|e| pick(&e.rules))
        .unwrap_or_default()
}

#[test]
fn a_rule_id_loses_the_prefix_and_the_hash() {
    let id = "veld:bookings/NewBooking.atMost30Days@91c0d2aa0b1c2d3e";
    assert_eq!(
        keys::rule_key(id).as_deref(),
        Some("bookings/NewBooking.atMost30Days")
    );
    assert_eq!(
        keys::rule_key("veld:@shop/money/Money.positive@0123456789abcdef").as_deref(),
        Some("@shop/money/Money.positive")
    );
    assert_eq!(keys::rule_key("bookings/X.y@91c0d2aa0b1c2d3e"), None);
    assert_eq!(keys::rule_key("veld:bookings/X.y"), None);
    assert_eq!(keys::rule_key("veld:bookings/X.y@zz"), None);
}

#[test]
fn the_key_is_the_id_of_the_rule_without_its_hash() {
    let ir = ir();
    let booking = ir.decls.iter().find(|d| d.name == "Booking").unwrap();
    let rule = booking
        .rules()
        .iter()
        .find(|r| r.name == "endAfterStart")
        .unwrap();
    assert_eq!(
        ir.violation_key(&rule.id, None).as_deref(),
        Some("app/Booking.endAfterStart")
    );
    let ep = &ir.services[0].endpoints[0];
    let key = |name: &str| {
        let r = ep
            .rules
            .requires
            .iter()
            .find(|r| r.name.as_deref() == Some(name))
            .unwrap();
        ir.violation_key(&r.id, None)
    };
    assert_eq!(key("notZero").as_deref(), Some("app/Bookings.pay.notZero"));
}

#[test]
fn unnamed_statements_share_one_key_per_kind() {
    let ir = ir();
    let ep = &ir.services[0].endpoints[0];
    let unnamed: Vec<_> = ep.rules.requires.iter().filter(|r| r.name.is_none()).collect();
    assert_eq!(unnamed.len(), 2);
    for r in unnamed {
        assert_eq!(
            ir.violation_key(&r.id, None).as_deref(),
            Some("app/Bookings.pay.require")
        );
    }
    for e in ep.rules.ensures.iter().filter(|e| e.name.is_none()) {
        assert_eq!(
            ir.violation_key(&e.id, None).as_deref(),
            Some("app/Bookings.pay.ensure")
        );
    }
}

#[test]
fn a_transition_and_a_version_guard_have_the_fixed_keys() {
    let ir = ir();
    let t = id_of(&ir, |r| r.transition.as_ref().map(|t| t.id.clone()));
    let v = id_of(&ir, |r| r.version.as_ref().map(|v| v.id.clone()));
    assert!(!t.is_empty() && !v.is_empty());
    assert_eq!(ir.violation_key(&t, None).as_deref(), Some("veld/transition"));
    assert_eq!(ir.violation_key(&v, None).as_deref(), Some("veld/version"));
}

#[test]
fn a_policy_denial_has_the_key_of_its_forbid_or_not_permitted() {
    let ir = ir();
    let p = &ir.policies[0].id;
    assert_eq!(
        ir.violation_key(p, Some(1)).as_deref(),
        Some("app/Manage.forbid1")
    );
    assert_eq!(
        ir.violation_key(p, Some(3)).as_deref(),
        Some("app/Manage.forbid3")
    );
    assert_eq!(
        ir.violation_key(p, None).as_deref(),
        Some("veld/not-permitted")
    );
}

#[test]
fn the_fixed_keys_are_four_and_sorted() {
    assert_eq!(
        keys::FIXED,
        [
            "veld/transition",
            "veld/transition-not-due",
            "veld/version",
            "veld/not-permitted"
        ]
    );
}

#[test]
fn a_catalog_may_translate_every_rule_of_the_member_and_only_those() {
    let ir = ir();
    let keys: Vec<(String, Option<String>)> =
        ir.catalog_keys(ir.services[0].package).into_iter().collect();
    let text = |k: &str| {
        keys.iter()
            .find(|(key, _)| key == k)
            .map(|(_, t)| t.clone())
    };
    assert_eq!(
        text("app/Booking.endAfterStart"),
        Some(Some("A booking must end after it starts".into()))
    );
    assert_eq!(text("app/Booking.silent"), Some(None));
    assert_eq!(
        text("app/Bookings.pay.notZero"),
        Some(Some("The amount must be positive".into()))
    );
    assert!(text("app/Bookings.pay.require").is_some());
    assert_eq!(
        text("app/Manage.forbid1"),
        Some(Some("A suspended account cannot manage bookings".into()))
    );
    assert_eq!(text("app/Manage.forbid2"), None, "a forbid without a message has no key");
    assert_eq!(
        text("app/Manage.forbid3"),
        Some(Some("Banned and suspended".into()))
    );
    assert_eq!(text("veld/transition"), None, "the fixed keys are valid but not listed");
}

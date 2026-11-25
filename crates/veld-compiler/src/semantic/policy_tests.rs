//! Policies, the principal and `allow` (`spec/rules.md` section 11): checks, lowering and ids.

use crate::testing::{codes, ir, multi, one};
use veld_ir::rules::{ExprKind, Root};
use veld_ir::{Ir, PolicyEffect};

const MODELS: &str = r#"
model User { id: uuid, roles: string[], suspended: bool, nick?: string }
model Booking { id: uuid, guestId: uuid, total: decimal }
principal User

policy ManageBooking(user: User, booking: Booking) {
  permit when booking.guestId == user.id
  permit when "support" in user.roles
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
}
"#;

fn service(auth: &str, block: &str) -> String {
    format!(
        "{MODELS}\n{auth}\nservice Bookings at \"/b\" {{\n  post \"/{{id}}/pay\" pay(id: uuid) -> Booking {{\n    fact booking: Booking = id\n{block}\n  }}\n}}\n"
    )
}

fn authed(block: &str) -> String {
    service("@auth(bearer)", block)
}

fn first_error(src: &str) -> String {
    one(src)
        .diagnostics
        .iter()
        .find(|d| d.code.starts_with('E'))
        .map(|d| d.message.clone())
        .unwrap_or_default()
}

// ---- lowering ----------------------------------------------------------------------------

#[test]
fn a_policy_a_principal_and_an_allow_reach_the_ir() {
    let ir = ir(&authed("    allow ManageBooking(user, booking)"));
    assert_eq!(ir.policies.len(), 1);
    let p = &ir.policies[0];
    assert_eq!(p.name, "ManageBooking");
    assert_eq!(p.qualified, "app:app.ManageBooking");
    assert!(p.id.starts_with("veld:app/ManageBooking@"), "{}", p.id);
    assert_eq!(p.params.len(), 2);
    let effects: Vec<PolicyEffect> = p.clauses.iter().map(|c| c.effect).collect();
    assert_eq!(
        effects,
        [
            PolicyEffect::Permit,
            PolicyEffect::Permit,
            PolicyEffect::Forbid
        ]
    );
    assert_eq!(p.clauses[0].message, None);
    assert_eq!(
        p.clauses[2].message.as_deref(),
        Some("A suspended account cannot manage bookings")
    );
    let principal = ir.services[0].principal.as_ref().expect("principal");
    assert_eq!(principal.name, "user");
    assert_eq!(ir.decl(principal.decl).name, "User");
    let allows = &ir.services[0].endpoints[0].rules.allows;
    assert_eq!(allows.len(), 1);
    assert_eq!(allows[0].policy.index(), 0);
    assert_eq!(allows[0].args.len(), 2);
    assert!(allows[0].reads.is_empty());
    let ExprKind::Path(arg0) = &allows[0].args[0].e else {
        panic!("not a path");
    };
    assert_eq!(arg0.root, Root::Principal);
    let ExprKind::Path(arg1) = &allows[0].args[1].e else {
        panic!("not a path");
    };
    assert_eq!(arg1.root, Root::Fact("booking".into()));
}

#[test]
fn the_parameters_an_argument_reads_are_its_reads() {
    let src = format!(
        "{MODELS}\npolicy Own(user: User, id: uuid) {{ permit when user.id == id }}\n@auth(bearer)\nservice S at \"/s\" {{\n  get \"/{{id}}\" get(id: uuid, @query other: int) -> Booking {{\n    allow Own(user, id)\n  }}\n}}\n"
    );
    let ir = ir(&src);
    let allow = &ir.services[0].endpoints[0].rules.allows[0];
    assert_eq!(allow.reads, ["id"]);
}

#[test]
fn several_allows_keep_their_source_order_and_policy_indexes_follow_names() {
    let src = format!(
        "{MODELS}\npolicy Zeta(user: User) {{ permit when true }}\npolicy Alpha(user: User) {{ permit when true }}\n@auth(bearer)\nservice S at \"/s\" {{\n  get \"/\" list() -> Booking {{\n    allow Zeta(user)\n    allow Alpha(user)\n    allow Zeta(user)\n  }}\n}}\n"
    );
    let ir = ir(&src);
    let names: Vec<&str> = ir.policies.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "ManageBooking", "Zeta"]);
    let order: Vec<usize> = ir.services[0].endpoints[0]
        .rules
        .allows
        .iter()
        .map(|a| a.policy.index())
        .collect();
    assert_eq!(order, [2, 0, 2]);
}

#[test]
fn an_allow_declares_forbidden_implicitly() {
    let ir = ir(&authed("    allow ManageBooking(user, booking)"));
    let ep = &ir.services[0].endpoints[0];
    let forbidden = ep
        .errors
        .iter()
        .find(|e| ir.decl(e.decl).name == "Forbidden")
        .expect("Forbidden");
    assert_eq!(forbidden.status, 403);
    let plain = self::ir(&authed("    require true"));
    assert!(
        !plain.services[0].endpoints[0]
            .errors
            .iter()
            .any(|e| plain.decl(e.decl).name == "Forbidden")
    );
}

#[test]
fn a_package_without_policies_has_none_in_the_ir() {
    let ir = ir("model A { x: int }\nservice S at \"/s\" { get \"/\" f() -> A }");
    assert!(ir.policies.is_empty());
    assert!(ir.services[0].principal.is_none());
    let json = serde_json::to_string(&ir).expect("json");
    assert!(!json.contains("policies") && !json.contains("principal"));
}

#[test]
fn every_service_of_the_package_gets_the_principal() {
    let src = format!(
        "{MODELS}\nservice A at \"/a\" {{ get \"/\" a() -> Booking }}\nservice B at \"/b\" {{ get \"/\" b() -> Booking }}\n"
    );
    let ir = ir(&src);
    assert!(ir.services.iter().all(|s| s.principal.is_some()));
}

#[test]
fn a_policy_over_facts_only_works_on_an_endpoint_without_auth() {
    let src = format!(
        "{MODELS}\npolicy Open(b: Booking) {{ permit when b.total >= 0 }}\nservice S at \"/s\" {{\n  get \"/{{id}}\" get(id: uuid) -> Booking {{\n    fact booking: Booking = id\n    allow Open(booking)\n  }}\n}}\n"
    );
    let out = one(&src);
    assert!(out.ir.is_some(), "{}", out.render());
}

// ---- the principal -------------------------------------------------------------------------

#[test]
fn the_principal_can_be_renamed() {
    let src = authed("    allow ManageBooking(caller, booking)")
        .replace("principal User", "principal User as caller");
    let ir = ir(&src);
    assert_eq!(
        ir.services[0].principal.as_ref().map(|p| p.name.as_str()),
        Some("caller")
    );
    let renamed_away = authed("    allow ManageBooking(user, booking)")
        .replace("principal User", "principal User as caller");
    assert_eq!(codes(&renamed_away), ["E0901"]);
}

#[test]
fn e0940_the_principal_must_be_a_model() {
    for ty in ["int", "Role", "Boom"] {
        let src = format!(
            "enum Role {{ admin }}\nerror Boom 400 {{ x: int }}\nprincipal {ty}\nmodel M {{ x: int }}"
        );
        assert_eq!(codes(&src), ["E0940"], "{ty}");
    }
}

#[test]
fn e0941_a_package_has_one_principal() {
    let src = "model A { x: int }\nmodel B { x: int }\nprincipal A\nprincipal B";
    assert_eq!(codes(src), ["E0941"]);
    let two_files = multi(&[
        ("app", "a.veld", "model A { x: int }\nprincipal A"),
        ("app", "b.veld", "model B { x: int }\nprincipal B"),
    ]);
    let found: Vec<&str> = two_files.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(found, ["E0941"]);
}

#[test]
fn two_packages_may_each_have_a_principal() {
    let out = multi(&[
        ("a", "a.veld", "model U { x: int }\nprincipal U"),
        ("b", "b.veld", "model V { x: int }\nprincipal V as caller"),
    ]);
    assert!(out.ir.is_some(), "{}", out.render());
}

#[test]
fn e0904_the_principal_is_not_there_without_auth() {
    let none = service("@auth(none)", "    require user.suspended");
    assert_eq!(codes(&none), ["E0904"]);
    let nothing = service("", "    require user.suspended");
    assert_eq!(codes(&nothing), ["E0904"]);
    let svc_level = service("@auth(bearer)", "    require not user.suspended");
    assert!(codes(&svc_level).is_empty());
    let endpoint_level = format!(
        "{MODELS}\nservice S at \"/s\" {{\n  @auth(basic)\n  get \"/\" f() -> Booking {{\n    require not user.suspended\n  }}\n}}\n"
    );
    assert!(codes(&endpoint_level).is_empty());
}

#[test]
fn e0920_a_parameter_or_fact_named_like_the_principal() {
    let param = format!(
        "{MODELS}\n@auth(bearer)\nservice S at \"/s\" {{\n  get \"/\" f(@query user: int) -> Booking\n  {{\n    require user > 0\n  }}\n}}\n"
    );
    assert_eq!(codes(&param), ["E0920"]);
    let fact = authed("    fact user: User = id");
    assert_eq!(codes(&fact), ["E0920"]);
    // Where the principal does not exist the name is free.
    let free = format!(
        "{MODELS}\nservice S at \"/s\" {{\n  get \"/\" f(@query user: int) -> Booking\n  {{\n    require user > 0\n  }}\n}}\n"
    );
    assert!(codes(&free).is_empty(), "{:?}", one(&free).render());
}

#[test]
fn the_principal_cannot_be_named_now() {
    let src = "model U { x: int }\nprincipal U as now";
    assert_eq!(codes(src), ["E0920"]);
}

#[test]
fn the_principal_is_readable_everywhere_in_the_block() {
    let block = "    fact other: Booking = id\n    require not user.suspended\n    ensure not user.suspended";
    assert!(codes(&authed(block)).is_empty());
    // The key of a fact may use it too.
    let key = authed("    fact mine: Booking = user.id");
    assert!(codes(&key).is_empty(), "{}", first_error(&key));
}

#[test]
fn the_principal_is_checked_like_any_value() {
    let optional = authed("    require user.nick == \"x\"");
    assert_eq!(codes(&optional), ["E0903"]);
    let guarded = authed("    require has(user.nick) && user.nick == \"x\"");
    assert!(codes(&guarded).is_empty());
    let unknown = authed("    require user.nope");
    assert_eq!(codes(&unknown), ["E0901"]);
}

// ---- policies -------------------------------------------------------------------------------

#[test]
fn w0940_a_policy_with_no_permit() {
    let src = "model U { x: int }\npolicy P(u: U) { forbid when u.x > 1 }";
    assert_eq!(codes(src), ["W0940"]);
    let empty = "model U { x: int }\npolicy P(u: U) {}";
    assert_eq!(codes(empty), ["W0940"]);
    let fine = "model U { x: int }\npolicy P(u: U) { permit when u.x > 1 }";
    assert!(codes(fine).is_empty());
}

#[test]
fn a_clause_may_read_now_and_nothing_else() {
    let now = "model U { until: datetime }\npolicy P(u: U) { permit when u.until > now }";
    assert!(codes(now).is_empty());
    // A fact, the principal and `result` are not in scope inside a policy.
    let fact = "model U { x: int }\nprincipal U\npolicy P(u: U) { permit when booking.x > 1 }";
    assert_eq!(codes(fact), ["E0901"]);
    let principal = "model U { x: int }\nprincipal U\npolicy P(u: U) { permit when user.x > 1 }";
    assert_eq!(codes(principal), ["E0901"]);
    let result = "model U { x: int }\npolicy P(u: U) { permit when result.x > 1 }";
    assert_eq!(codes(result), ["E0904"]);
}

#[test]
fn a_clause_follows_the_guard_rules_and_must_be_boolean() {
    let optional = "model U { n?: int }\npolicy P(u: U) { permit when u.n > 1 }";
    assert_eq!(codes(optional), ["E0903"]);
    let guarded = "model U { n?: int }\npolicy P(u: U) { permit when has(u.n) && u.n > 1 }";
    assert!(codes(guarded).is_empty());
    let not_bool = "model U { n: int }\npolicy P(u: U) { permit when u.n }";
    assert_eq!(codes(not_bool), ["E0902"]);
}

#[test]
fn a_clause_can_nest_only_so_deep() {
    let deep = (0..40).fold("u.x".to_string(), |acc, _| format!("({acc} + 1)"));
    let src = format!("model U {{ x: int }}\npolicy P(u: U) {{ permit when {deep} > 1 }}");
    assert_eq!(codes(&src), ["E0005"]);
}

#[test]
fn a_parameter_that_repeats_is_reported() {
    let src = "model U { x: int }\npolicy P(u: U, u: U) { permit when u.x > 1 }";
    assert_eq!(codes(src), ["E0907"]);
}

#[test]
fn a_policy_name_is_unique_in_the_package() {
    let src = "model P { x: int }\npolicy P(u: P) { permit when u.x > 1 }";
    assert_eq!(codes(src), ["E0201"]);
    let twice = "model U { x: int }\npolicy P(u: U) { permit when u.x > 1 }\npolicy P(u: U) { permit when u.x > 2 }";
    assert_eq!(codes(twice), ["E0201"]);
}

#[test]
fn a_policy_is_imported_like_a_declaration() {
    let policies = "model U { x: int }\npolicy Big(u: U) { permit when u.x > 100 }";
    let app = "use policies.{U, Big}\nprincipal U\n@auth(bearer)\nservice S at \"/s\" {\n  get \"/\" f() -> U {\n    allow Big(user)\n  }\n}";
    let out = multi(&[("app", "policies.veld", policies), ("app", "app.veld", app)]);
    assert!(out.ir.is_some(), "{}", out.render());
    let unused = "use policies.{U, Big}\nprincipal U\nservice S at \"/s\" { get \"/\" f() -> U }";
    let out = multi(&[
        ("app", "policies.veld", policies),
        ("app", "app.veld", unused),
    ]);
    let found: Vec<&str> = out.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(found, ["W0201"]);
    let missing = "use policies.{Nope}\nmodel M { x: int }";
    let out = multi(&[
        ("app", "policies.veld", policies),
        ("app", "app.veld", missing),
    ]);
    assert!(out.diagnostics.iter().any(|d| d.code == "E0205"));
}

#[test]
fn a_policy_that_is_not_imported_is_unknown() {
    let policies = "model U { x: int }\npolicy Big(u: U) { permit when u.x > 100 }";
    let app = "use policies.{U}\nprincipal U\n@auth(bearer)\nservice S at \"/s\" {\n  get \"/\" f() -> U {\n    allow Big(user)\n  }\n}";
    let out = multi(&[("app", "policies.veld", policies), ("app", "app.veld", app)]);
    let found: Vec<&str> = out.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(found, ["E0942"]);
}

// ---- allow ----------------------------------------------------------------------------------

#[test]
fn e0942_an_unknown_policy() {
    let src = authed("    allow Nope(user, booking)");
    assert_eq!(codes(&src), ["E0942"]);
    assert!(first_error(&src).contains("unknown policy"));
    let near = authed("    allow ManageBookin(user, booking)");
    let out = one(&near);
    let d = out
        .diagnostics
        .iter()
        .find(|d| d.code == "E0942")
        .expect("E0942");
    assert!(
        d.help
            .as_deref()
            .is_some_and(|h| h.contains("ManageBooking")),
        "{d:?}"
    );
}

#[test]
fn e0942_the_wrong_number_of_arguments() {
    for call in [
        "ManageBooking(user)",
        "ManageBooking(user, booking, booking)",
        "ManageBooking()",
    ] {
        let src = authed(&format!("    allow {call}"));
        assert_eq!(codes(&src), ["E0942"], "{call}");
        assert!(first_error(&src).contains("takes 2 arguments"), "{call}");
    }
}

#[test]
fn e0942_an_argument_of_the_wrong_type() {
    let src = authed("    allow ManageBooking(booking, user)");
    let out = one(&src);
    let found: Vec<&str> = out.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(found, ["E0942", "E0942"]);
    let scalar = authed("    allow ManageBooking(user, id)");
    assert_eq!(codes(&scalar), ["E0942"]);
}

#[test]
fn an_argument_may_widen_exactly() {
    let src = "model U { n: int }\nprincipal U\npolicy Big(n: long) { permit when n > 100 }\n@auth(bearer)\nservice S at \"/s\" {\n  get \"/\" f() -> U {\n    allow Big(user.n)\n  }\n}";
    let out = one(src);
    assert!(out.ir.is_some(), "{}", out.render());
}

#[test]
fn an_argument_is_checked_in_the_scope_of_the_endpoint() {
    let unknown = authed("    allow ManageBooking(user, nothing)");
    assert_eq!(codes(&unknown), ["E0901"]);
    let result = authed("    allow ManageBooking(user, result)");
    assert_eq!(codes(&result), ["E0904"]);
}

#[test]
fn an_allow_without_a_principal_on_an_endpoint_with_auth_none_is_fine_for_facts() {
    let src = format!(
        "{MODELS}\npolicy Plain(b: Booking, since: datetime) {{ permit when b.total >= 0 && since <= now }}\n@auth(none)\nservice S at \"/s\" {{\n  get \"/{{id}}\" get(id: uuid) -> Booking {{\n    fact booking: Booking = id\n    allow Plain(booking, now)\n  }}\n}}\n"
    );
    let out = one(&src);
    assert!(out.ir.is_some(), "{}", out.render());
}

// ---- identity -------------------------------------------------------------------------------

fn id_of(src: &str, name: &str) -> String {
    let ir: Ir = ir(src);
    ir.policies
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.id.clone())
        .expect("policy")
}

#[test]
fn the_id_ignores_messages_and_docs_and_follows_the_clauses() {
    let base =
        "model U { x: int }\npolicy P(u: U) {\n  permit when u.x > 1\n  forbid when u.x > 9\n}";
    let worded = "model U { x: int }\n/// Who may.\npolicy P(u: U) {\n  permit when u.x > 1 message \"a\"\n  forbid when u.x > 9 message \"b\"\n}";
    assert_eq!(id_of(base, "P"), id_of(worded, "P"));
    let other =
        "model U { x: int }\npolicy P(u: U) {\n  permit when u.x > 2\n  forbid when u.x > 9\n}";
    assert_ne!(id_of(base, "P"), id_of(other, "P"));
    let swapped =
        "model U { x: int }\npolicy P(u: U) {\n  forbid when u.x > 9\n  permit when u.x > 1\n}";
    assert_ne!(id_of(base, "P"), id_of(swapped, "P"));
    let renamed =
        "model U { x: int }\npolicy Q(u: U) {\n  permit when u.x > 1\n  forbid when u.x > 9\n}";
    assert_ne!(id_of(base, "P"), id_of(renamed, "Q"));
}

#[test]
fn the_name_of_a_parameter_is_in_the_id() {
    let a = "model U { x: int }\npolicy P(u: U) { permit when u.x > 1 }";
    let b = "model U { x: int }\npolicy P(v: U) { permit when v.x > 1 }";
    assert_ne!(id_of(a, "P"), id_of(b, "P"));
}

//! The rule book (`spec/rules.md` section 19.2).

use veld_ir::ServiceId;

use super::{NO_RULES, rule_book};
use crate::format;
use crate::testutil::ir;

const SRC: &str = r#"enum Status {
  pending, paid, cancelled
  flow pending -> paid
  flow pending -> cancelled
  final paid, cancelled
}

model User {
  id: uuid
  roles: string[]
  suspended: bool
}

principal User

/// What a guest asks for.
model NewBooking {
  guests: int @min(1)

  /// Parties above six need a call.
  @rollout(audit)
  rule smallParty: guests <= 6

  rule atLeastOne: guests >= 1
    message "At least one guest"

  /// The old limit.
  @deprecated("use smallParty")
  rule limit: guests <= 20
}

model Booking {
  id: uuid
  guestId: uuid
  status: Status = .pending
  total: decimal
}

event Paid {
  amount: int
  rule positive: amount > 0
}

channel Money { carries Paid }

error RoomFull 409 { capacity: int }

/// Who may manage a booking.
policy ManageBooking(user: User, booking: Booking) {
  permit when booking.guestId == user.id
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
}

@auth(bearer)
service Bookings at "/bookings" {
  publishes Money

  /// Pays a booking.
  post "/{id}/pay" payBooking(id: uuid, body: NewBooking) -> Booking | RoomFull {
    fact booking: Booking = id
    allow ManageBooking(user, booking)
    transition booking.status: pending -> paid
    require fits: body.guests <= 4 else RoomFull { capacity: 4 }
    require body.guests >= 1 message "At least one guest"
    @rollout(audit)
    ensure result.total >= 0
    ensure result.status == .paid
  }

  get "/{id}" getBooking(id: uuid) -> Booking
}

example "a pending booking is paid" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", status: .pending, total: 10.00 }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [], suspended: false }
  payBooking(id: booking.id, body: { guests: 2 }) -> ok
}

example "a party of five is turned away" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", status: .pending, total: 10.00 }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [], suspended: false }
  payBooking(id: booking.id, body: { guests: 5 }) -> RoomFull
}
"#;

fn book() -> String {
    let ir = ir(SRC);
    let all: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    rule_book(&ir, &all)
}

#[test]
fn the_whole_book_of_a_contract_with_every_part() {
    insta::assert_snapshot!(book());
}

#[test]
fn the_five_parts_come_in_order() {
    let b = book();
    let at = |needle: &str| {
        b.find(needle)
            .unwrap_or_else(|| panic!("no `{needle}` in\n{b}"))
    };
    let order = [
        "# Rule book",
        "## Package `app`",
        "### Declarations",
        "### State machines",
        "### Policies",
        "### Endpoints",
        "### Examples",
    ];
    let positions: Vec<usize> = order.iter().map(|n| at(n)).collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "{positions:?}\n{b}"
    );
}

#[test]
fn a_rule_is_listed_by_its_message_or_its_name_with_its_doc_and_expression() {
    let b = book();
    assert!(b.contains("- At least one guest: `guests >= 1`\n"), "{b}");
    assert!(
        b.contains("- smallParty: `guests <= 6` (audited, not enforced)\n  Parties above six need a call.\n"),
        "{b}"
    );
    assert!(
        b.contains("- limit: `guests <= 20` (deprecated: use smallParty)\n  The old limit.\n"),
        "{b}"
    );
    assert!(
        b.contains("#### NewBooking (model)\n\nWhat a guest asks for.\n"),
        "{b}"
    );
}

#[test]
fn an_event_with_invariants_is_a_declaration_with_invariants() {
    let b = book();
    assert!(b.contains("#### Paid (event)"), "{b}");
    assert!(b.contains("- positive: `amount > 0`"), "{b}");
}

#[test]
fn a_state_machine_is_a_table_of_moves_and_its_final_states() {
    let b = book();
    assert!(
        b.contains("#### Status\n\n| From | To |\n|---|---|\n| pending | paid |\n| pending | cancelled |\n\nFinal states: paid, cancelled\n"),
        "{b}"
    );
}

#[test]
fn a_policy_lists_its_clauses_in_order_and_a_forbid_with_its_message() {
    let b = book();
    let permit = b
        .find("- permit when `booking.guestId == user.id`\n")
        .unwrap();
    let forbid = b
        .find("- forbid when `user.suspended`: A suspended account cannot manage bookings\n")
        .unwrap();
    assert!(permit < forbid);
    assert!(
        b.contains("#### ManageBooking(user, booking)\n\nWho may manage a booking.\n"),
        "{b}"
    );
}

#[test]
fn an_endpoint_lists_what_it_loads_who_may_call_it_its_move_and_its_promises() {
    let b = book();
    assert!(
        b.contains("#### POST /bookings/{id}/pay (Bookings.payBooking)\n\nPays a booking.\n"),
        "{b}"
    );
    let in_order = [
        "- Loads: `booking: Booking = id`",
        "- Who may call it: `ManageBooking(principal, booking)`",
        "- Move: `booking.status`: pending -> paid",
        "- Requires:\n  - fits: `body.guests <= 4`, answers `RoomFull`\n  - At least one guest: `body.guests >= 1`, answers `ValidationError`\n",
        "- Promises:\n  - ensure #1: `result.total >= 0` (audited, not enforced)\n  - ensure #2: `result.status ==",
    ];
    let mut from = 0;
    for needle in in_order {
        let at = b[from..]
            .find(needle)
            .unwrap_or_else(|| panic!("no `{needle}` after {from} in\n{b}"));
        from += at;
    }
}

#[test]
fn an_endpoint_without_rules_is_not_listed() {
    assert!(!book().contains("getBooking"));
}

#[test]
fn the_examples_are_listed_by_name_with_their_outcome() {
    let b = book();
    assert!(
        b.contains("- a pending booking is paid: ok\n- a party of five is turned away: RoomFull\n"),
        "{b}"
    );
}

#[test]
fn the_expressions_read_as_veld_fmt_prints_them() {
    let formatted = format(SRC).unwrap();
    for expr in [
        "guests <= 6",
        "guests >= 1",
        "amount > 0",
        "body.guests <= 4",
        "result.total >= 0",
        "booking.guestId == user.id",
    ] {
        assert!(
            formatted.contains(expr),
            "veld fmt does not print `{expr}`:\n{formatted}"
        );
        assert!(
            book().contains(&format!("`{expr}`")),
            "the book does not print `{expr}`"
        );
    }
}

#[test]
fn two_runs_give_the_same_bytes() {
    assert_eq!(book(), book());
}

#[test]
fn a_workspace_without_rules_says_so() {
    let ir = ir("model A { n: int }\nservice S at \"/s\" { get \"/\" list() -> A }\n");
    let all: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    assert_eq!(rule_book(&ir, &all), format!("# Rule book\n\n{NO_RULES}\n"));
}

#[test]
fn no_service_selected_means_no_rules() {
    let ir = ir(SRC);
    assert_eq!(rule_book(&ir, &[]), format!("# Rule book\n\n{NO_RULES}\n"));
}

//! `emits` on a flow and on a transition (`spec/events.md` section 11): checks, lowering, ids.

use crate::testing::{codes, ir, multi, one};
use veld_ir::{DeclKind, Feature, Ir};

const EVENTS: &str = r#"
event BookingPaid { bookingId: uuid, amount: decimal }
event BookingCancelled { bookingId: uuid }
event Refunded { bookingId: uuid }

@topic("shop.bookings.v1")
channel BookingEvents {
  carries BookingPaid, BookingCancelled
  key bookingId
}

@topic("shop.refunds.v1")
channel RefundEvents { carries Refunded }

model Booking { id: uuid, status: Status, total: int }
"#;

fn enum_with(flows: &str) -> String {
    format!("enum Status {{\n  pending, paid, cancelled\n{flows}\n  final cancelled\n}}\n")
}

const FLOWS: &str = "  flow pending -> paid emits BookingPaid\n  flow pending -> cancelled emits BookingCancelled\n  flow paid -> cancelled";

fn service(publishes: &str, endpoints: &str) -> String {
    format!("service Bookings at \"/b\" {{\n{publishes}\n{endpoints}\n}}\n")
}

const PAY: &str = r#"  post "/{id}/pay" pay(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending -> paid
  }"#;

fn source(flows: &str, publishes: &str, endpoints: &str) -> String {
    format!(
        "{EVENTS}\n{}\n{}",
        enum_with(flows),
        service(publishes, endpoints)
    )
}

fn errors(src: &str) -> Vec<&'static str> {
    codes(src)
        .into_iter()
        .filter(|c| c.starts_with('E'))
        .collect()
}

fn status(ir: &Ir) -> veld_ir::Enum {
    ir.decls
        .iter()
        .find_map(|d| match &d.kind {
            DeclKind::Enum(e) if d.name == "Status" => Some(e.clone()),
            _ => None,
        })
        .expect("Status")
}

#[test]
fn a_flow_with_emits_lowers_the_event_and_keeps_its_edges() {
    let ir = ir(&source(FLOWS, "  publishes BookingEvents", PAY));
    let s = status(&ir);
    assert_eq!(s.flows.len(), 3);
    let name = |f: &veld_ir::Flow| f.emits.map(|e| ir.decl(e).name.clone());
    assert_eq!(name(&s.flows[0]).as_deref(), Some("BookingPaid"));
    assert_eq!(name(&s.flows[1]).as_deref(), Some("BookingCancelled"));
    assert_eq!(name(&s.flows[2]), None);
}

#[test]
fn a_transition_over_an_emitting_edge_lowers_the_event_and_the_channel() {
    let ir = ir(&source(FLOWS, "  publishes BookingEvents", PAY));
    let t = ir.services[0].endpoints[0]
        .rules
        .transition
        .as_ref()
        .expect("transition");
    assert_eq!(t.emits.len(), 1);
    assert_eq!(ir.decl(t.emits[0].event).name, "BookingPaid");
    assert_eq!(ir.channel(t.emits[0].channel).name, "BookingEvents");
}

#[test]
fn several_sources_emit_the_events_of_their_edges_once_each() {
    let flows = "  flow pending -> cancelled emits BookingCancelled\n  flow paid -> cancelled emits BookingCancelled\n  flow pending -> paid";
    let cancel = r#"  post "/{id}/cancel" cancel(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending, paid -> cancelled
  }"#;
    let ir = ir(&source(flows, "  publishes BookingEvents", cancel));
    let t = ir.services[0].endpoints[0]
        .rules
        .transition
        .as_ref()
        .expect("transition");
    assert_eq!(t.emits.len(), 1);
    assert_eq!(ir.decl(t.emits[0].event).name, "BookingCancelled");
}

#[test]
fn a_move_over_edges_with_different_events_emits_both_in_edge_order() {
    let flows = "  flow pending -> cancelled emits BookingCancelled\n  flow paid -> cancelled emits BookingPaid\n  flow pending -> paid";
    let cancel = r#"  post "/{id}/cancel" cancel(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending, paid -> cancelled
  }"#;
    let ir = ir(&source(flows, "  publishes BookingEvents", cancel));
    let t = ir.services[0].endpoints[0]
        .rules
        .transition
        .as_ref()
        .unwrap();
    let names: Vec<_> = t
        .emits
        .iter()
        .map(|e| ir.decl(e.event).name.clone())
        .collect();
    assert_eq!(names, ["BookingCancelled", "BookingPaid"]);
}

#[test]
fn the_first_published_channel_that_carries_the_event_is_the_one() {
    let src = format!(
        "{EVENTS}\nchannel Mirror {{ carries BookingPaid }}\n{}\n{}",
        enum_with(FLOWS),
        service("  publishes Mirror, BookingEvents", PAY)
    );
    let ir = ir(&src);
    let t = ir.services[0].endpoints[0]
        .rules
        .transition
        .as_ref()
        .unwrap();
    assert_eq!(ir.channel(t.emits[0].channel).name, "Mirror");
}

#[test]
fn emits_is_not_part_of_the_transition_id_nor_the_enum_signature() {
    let with = ir(&source(FLOWS, "  publishes BookingEvents", PAY));
    let plain_flows = FLOWS
        .replace(" emits BookingPaid", "")
        .replace(" emits BookingCancelled", "");
    let plain = ir(&source(&plain_flows, "", PAY));
    let id = |i: &Ir| {
        i.services[0].endpoints[0]
            .rules
            .transition
            .as_ref()
            .unwrap()
            .id
            .clone()
    };
    assert_eq!(id(&with), id(&plain));
    let sig = |i: &Ir| {
        i.decls
            .iter()
            .find(|d| d.name == "Status")
            .unwrap()
            .sig
            .clone()
    };
    assert_eq!(sig(&with), sig(&plain));
}

#[test]
fn a_contract_without_emits_keeps_its_ir_keys() {
    let plain_flows = FLOWS
        .replace(" emits BookingPaid", "")
        .replace(" emits BookingCancelled", "");
    let json = serde_json::to_string(&ir(&source(&plain_flows, "", PAY))).expect("json");
    assert!(!json.contains("emits"), "{json}");
}

#[test]
fn the_ir_json_has_the_keys_of_the_spec() {
    let ir = ir(&source(FLOWS, "  publishes BookingEvents", PAY));
    let v = serde_json::to_value(&ir).expect("json");
    let flow = &v["decls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "Status")
        .unwrap()["flows"][0];
    assert!(flow["emits"].is_number(), "{flow}");
    let t = &v["services"][0]["endpoints"][0]["rules"]["transition"]["emits"][0];
    assert!(t["event"].is_number() && t["channel"].is_number(), "{t}");
}

#[test]
fn events_outbox_is_used_by_a_service_with_an_emitting_move_only() {
    let with = ir(&source(FLOWS, "  publishes BookingEvents", PAY));
    let id = veld_ir::ServiceId(0);
    assert!(with.features_used(&[id]).contains(&Feature::EventsOutbox));
    let other = r#"  post "/{id}/cancel" cancel(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: paid -> cancelled
  }"#;
    let without = ir(&source(FLOWS, "  publishes BookingEvents", other));
    let id = veld_ir::ServiceId(0);
    assert!(
        !without
            .features_used(&[id])
            .contains(&Feature::EventsOutbox)
    );
}

#[test]
fn emits_on_a_flow_with_several_edges_is_e1011() {
    let flows = "  flow pending -> paid -> cancelled emits BookingPaid";
    assert_eq!(
        errors(&source(flows, "  publishes BookingEvents", PAY)),
        ["E1011"]
    );
}

#[test]
fn emits_naming_no_event_is_e1011() {
    for name in ["Nothing", "Booking", "BookingEvents", "Status"] {
        let flows = format!("  flow pending -> paid emits {name}\n  flow pending -> cancelled");
        let e = errors(&source(&flows, "  publishes BookingEvents", PAY));
        assert_eq!(e, ["E1011"], "{name}");
    }
}

#[test]
fn a_transition_whose_service_publishes_no_such_channel_is_e1011_at_the_transition() {
    let out = one(&source(FLOWS, "  publishes RefundEvents", PAY));
    let d: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| d.code == "E1011")
        .collect();
    assert_eq!(d.len(), 1, "{}", out.render());
    let text = out.render();
    assert!(text.contains("BookingPaid"), "{text}");
    let none = errors(&source(FLOWS, "", PAY));
    assert_eq!(none, ["E1011"]);
}

#[test]
fn a_flow_that_emits_and_no_endpoint_takes_needs_no_publisher() {
    let e = errors(&source(FLOWS, "", r#"  get "/ping" ping() -> 204"#));
    assert!(e.is_empty(), "{e:?}");
}

#[test]
fn a_repeated_edge_with_another_emits_is_e1011() {
    let flows = "  flow pending -> paid emits BookingPaid\n  flow pending -> paid emits BookingCancelled\n  flow pending -> cancelled";
    let e = errors(&source(flows, "  publishes BookingEvents", PAY));
    assert_eq!(e, ["E1011"]);
}

#[test]
fn emits_is_an_ordinary_name_elsewhere() {
    let src = "enum S { emits, b\n  flow emits -> b\n  final b }\nmodel emits { emits: int }";
    assert!(errors(src).is_empty(), "{:?}", codes(src));
}

#[test]
fn emits_on_the_next_line_is_a_variant_not_a_clause() {
    let src = "enum S { a, b\n  flow a -> b\n  emits\n  final b }";
    let out = one(src);
    assert!(
        out.diagnostics.iter().all(|d| d.code != "E0003"),
        "{}",
        out.render()
    );
}

#[test]
fn an_event_from_another_package_can_be_emitted_when_it_is_visible() {
    let events = "event Paid { id: uuid }\nchannel Pays { carries Paid }";
    let app = "use lib:events.{Paid}\nenum S { a, b\n  flow a -> b emits Paid\n  final b }";
    let out = multi(&[("lib", "events.veld", events), ("app", "app.veld", app)]);
    assert!(
        out.diagnostics.iter().all(|d| d.code != "E1011"),
        "{}",
        out.render()
    );
}

// ---- `expect emitted` -------------------------------------------------------------------------

const PAY_AND_CANCEL: &str = r#"  post "/{id}/pay" pay(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending -> paid
  }
  post "/{id}/hold" hold(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: paid -> cancelled
  }"#;

fn with_example(example: &str) -> String {
    format!(
        "{}\n{example}",
        source(FLOWS, "  publishes BookingEvents", PAY_AND_CANCEL)
    )
}

const PAID: &str = "example \"pays\" {\n  given booking = { status: .pending }\n  pay() returns { status: .paid } -> ok\n";

#[test]
fn expect_emitted_lowers_the_events_of_the_call_example() {
    let ir = ir(&with_example(&format!(
        "{PAID}  expect emitted BookingPaid\n}}"
    )));
    let ex = &ir.examples[0];
    let emitted = ex.emitted.as_ref().expect("emitted");
    assert_eq!(emitted.len(), 1);
    assert_eq!(ir.decl(emitted[0]).name, "BookingPaid");
}

#[test]
fn expect_not_emitted_lowers_to_an_empty_list_and_is_allowed_after_an_error() {
    let src = with_example(
        "example \"a paid booking cannot be paid\" {\n  given booking = { status: .paid }\n  pay() -> InvalidTransition\n  expect not emitted\n}",
    );
    let ir = ir(&src);
    assert_eq!(ir.examples[0].emitted.as_deref(), Some(&[][..]));
}

#[test]
fn an_example_without_the_lines_has_no_emitted_key() {
    let ir = ir(&with_example(&format!("{PAID}}}")));
    assert!(ir.examples[0].emitted.is_none());
    let json = serde_json::to_string(&ir.examples[0]).expect("json");
    assert!(!json.contains("emitted"), "{json}");
}

#[test]
fn expect_emitted_on_an_endpoint_whose_move_emits_nothing_is_e1010() {
    let src = with_example(
        "example \"x\" {\n  given booking = { status: .paid }\n  hold() returns { status: .cancelled } -> ok\n  expect emitted BookingPaid\n}",
    );
    assert_eq!(errors(&src), ["E1010"]);
}

#[test]
fn expect_emitted_of_an_event_the_move_does_not_emit_is_e1010() {
    let src = with_example(&format!("{PAID}  expect emitted BookingCancelled\n}}"));
    assert_eq!(errors(&src), ["E1010"]);
}

#[test]
fn expect_emitted_naming_no_event_is_e1010_or_e0205() {
    let src = with_example(&format!("{PAID}  expect emitted Booking\n}}"));
    assert_eq!(errors(&src), ["E1010"]);
    let src = with_example(&format!("{PAID}  expect emitted Nothing\n}}"));
    assert_eq!(errors(&src), ["E0205"]);
}

#[test]
fn expect_emitted_needs_ok_and_returns() {
    let error = with_example(
        "example \"x\" {\n  given booking = { status: .paid }\n  pay() -> InvalidTransition\n  expect emitted BookingPaid\n}",
    );
    assert_eq!(errors(&error), ["E1010"]);
    let no_returns = with_example(
        "example \"x\" {\n  given booking = { status: .pending }\n  pay() -> ok\n  expect emitted BookingPaid\n}",
    );
    assert_eq!(errors(&no_returns), ["E1010"]);
}

#[test]
fn not_emitted_cannot_be_mixed_and_an_event_is_listed_once() {
    let mixed = with_example(&format!(
        "{PAID}  expect emitted BookingPaid\n  expect not emitted\n}}"
    ));
    assert_eq!(errors(&mixed), ["E1010"]);
    let twice = with_example(&format!(
        "{PAID}  expect emitted BookingPaid\n  expect emitted BookingPaid\n}}"
    ));
    assert_eq!(errors(&twice), ["E1010"]);
}

#[test]
fn expect_not_emitted_on_an_endpoint_without_an_emitting_move_is_e1010() {
    let src = format!(
        "{}\nexample \"x\" {{\n  ping() -> ok\n  expect not emitted\n}}",
        source(
            FLOWS,
            "  publishes BookingEvents",
            "  get \"/ping\" ping() -> 204"
        )
    );
    assert_eq!(errors(&src), ["E1010"]);
}

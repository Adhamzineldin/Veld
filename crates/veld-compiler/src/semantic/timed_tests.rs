//! Timed moves (`spec/platform.md` section 10): `after`, `since`, `E0935`, `E0936`, `W0933`, the
//! IR keys and the id of a timed transition.

use crate::testing::{codes, ir};
use veld_ir::{DeclKind, Feature, Ir};

fn contract(flows: &str, model: &str, block: &str) -> String {
    format!(
        "enum BookingStatus {{\n  pending, paid, cancelled\n{flows}\n  final paid, cancelled\n}}\n\
         model Booking {{ id: uuid, status: BookingStatus, {model} }}\n\
         service Bookings at \"/bookings\" {{\n\
         \x20 post \"/{{id}}/expire\" expireBooking(id: uuid) -> Booking {{\n\
         \x20   fact booking: Booking = id\n{block}\n  }}\n}}\n"
    )
}

const FLOWS: &str = "  flow pending -> paid\n  flow pending -> cancelled after hours(24)";
const SINCE: &str =
    "    transition booking.status: pending -> cancelled since booking.pendingSince";
const MODEL: &str = "pendingSince: datetime";

fn ok() -> String {
    contract(FLOWS, MODEL, SINCE)
}

fn transition_id(ir: &Ir) -> String {
    ir.services[0].endpoints[0]
        .rules
        .transition
        .as_ref()
        .map(|t| t.id.clone())
        .unwrap_or_default()
}

#[test]
fn a_timed_edge_and_its_transition_compile() {
    assert!(codes(&ok()).is_empty(), "{:?}", codes(&ok()));
    let ir = ir(&ok());
    let machine = ir.decls.iter().find_map(|d| match &d.kind {
        DeclKind::Enum(e) if d.name == "BookingStatus" => Some(e.clone()),
        _ => None,
    });
    let machine = machine.expect("BookingStatus");
    let timed: Vec<(&str, Option<&str>)> = machine
        .flows
        .iter()
        .map(|f| (f.to.as_str(), f.after_ms.as_deref()))
        .collect();
    assert_eq!(timed, [("paid", None), ("cancelled", Some("86400000"))]);
    let t = ir.services[0].endpoints[0].rules.transition.as_ref();
    let t = t.expect("transition");
    assert_eq!(t.since.len(), 1);
    assert_eq!(t.since[0].wire, "pendingSince");
    assert!(t.is_timed());
}

#[test]
fn the_feature_is_used_only_by_a_contract_with_a_timed_edge() {
    let timed = ir(&ok());
    let used = timed.features_used(&[timed.services[0].id]);
    assert!(used.contains(&Feature::RulesTimed), "{used:?}");
    let plain = contract(
        "  flow pending -> paid\n  flow pending -> cancelled",
        MODEL,
        "    transition booking.status: pending -> cancelled",
    );
    let plain = ir(&plain);
    assert!(
        !plain
            .features_used(&[plain.services[0].id])
            .contains(&Feature::RulesTimed)
    );
    let json = serde_json::to_string(&plain).unwrap_or_default();
    assert!(
        !json.contains("after_ms") && !json.contains("since"),
        "{json}"
    );
}

#[test]
fn the_id_of_a_timed_transition_differs_and_an_ordinary_one_is_unchanged() {
    let timed = transition_id(&ir(&ok()));
    let plain = contract(
        "  flow pending -> paid\n  flow pending -> cancelled",
        MODEL,
        "    transition booking.status: pending -> cancelled",
    );
    let plain_id = transition_id(&ir(&plain));
    assert_ne!(timed, plain_id);
    assert!(timed.starts_with("veld:") && timed.contains(".transition@"));
    // A longer duration is another edge, so another id.
    let longer = contract(&FLOWS.replace("hours(24)", "hours(48)"), MODEL, SINCE);
    assert_ne!(transition_id(&ir(&longer)), timed);
}

#[test]
fn durations_are_folded_with_checked_arithmetic() {
    for (text, ms) in [
        ("hours(24)", "86400000"),
        ("days(1) + hours(1)", "90000000"),
        ("minutes(5) * 2", "600000"),
        ("seconds(90) - seconds(30)", "60000"),
        ("ms(1)", "1"),
    ] {
        let flows = format!("  flow pending -> paid\n  flow pending -> cancelled after {text}");
        let ir = ir(&contract(&flows, MODEL, SINCE));
        let after = ir.decls.iter().find_map(|d| match &d.kind {
            DeclKind::Enum(e) => e.flows.iter().find_map(|f| f.after_ms.clone()),
            _ => None,
        });
        assert_eq!(after.as_deref(), Some(ms), "{text}");
    }
}

#[test]
fn after_with_several_edges_or_a_bad_duration_is_e0935() {
    let cases = [
        "  flow pending -> paid -> cancelled after hours(1)",
        "  flow pending -> paid\n  flow pending -> cancelled after hours(0)",
        "  flow pending -> paid\n  flow pending -> cancelled after hours(-1)",
        "  flow pending -> paid\n  flow pending -> cancelled after 5",
        "  flow pending -> paid\n  flow pending -> cancelled after now",
        "  flow pending -> paid\n  flow pending -> cancelled after hours(9223372036854775807)",
    ];
    for flows in cases {
        let got = codes(&contract(
            flows,
            MODEL,
            "    transition booking.status: pending -> paid",
        ));
        assert!(got.contains(&"E0935"), "{flows}: {got:?}");
    }
}

#[test]
fn a_second_after_for_the_same_edge_is_e0935() {
    let flows = "  flow pending -> paid\n  flow pending -> cancelled after hours(1)\n  flow pending -> cancelled after hours(2)";
    let got = codes(&contract(flows, MODEL, SINCE));
    assert_eq!(got, ["E0935"]);
}

#[test]
fn a_timed_edge_needs_since() {
    let got = codes(&contract(
        FLOWS,
        MODEL,
        "    transition booking.status: pending -> cancelled",
    ));
    assert_eq!(got, ["E0936"]);
}

#[test]
fn since_on_an_edge_that_is_not_timed_is_e0936() {
    let block = "    transition booking.status: pending -> paid since booking.pendingSince";
    assert_eq!(codes(&contract(FLOWS, MODEL, block)), ["E0936"]);
}

#[test]
fn since_with_several_sources_is_e0936() {
    let flows = "  flow pending -> cancelled after hours(1)\n  flow paid -> cancelled after hours(1)\n  flow pending -> paid";
    let block =
        "    transition booking.status: pending, paid -> cancelled since booking.pendingSince";
    let got = codes(&contract(flows, MODEL, block));
    assert!(got.contains(&"E0936"), "{got:?}");
}

#[test]
fn since_must_be_a_required_datetime_field_of_the_fact() {
    let cases = [
        ("pendingSince: datetime, other: string", "booking.other"),
        ("pendingSince?: datetime", "booking.pendingSince"),
        ("pendingSince: date", "booking.pendingSince"),
        (MODEL, "booking"),
        (MODEL, "booking.missing"),
        (MODEL, "other.pendingSince"),
    ];
    for (model, path) in cases {
        let block = format!("    transition booking.status: pending -> cancelled since {path}");
        let got = codes(&contract(FLOWS, model, &block));
        assert!(
            !got.is_empty() && got[0].starts_with('E'),
            "{path}: {got:?}"
        );
    }
}

#[test]
fn a_timed_edge_that_no_transition_takes_is_w0933() {
    let got = codes(&contract(
        FLOWS,
        MODEL,
        "    transition booking.status: pending -> paid",
    ));
    assert_eq!(got, ["W0933"]);
}

#[test]
fn a_nested_since_path_is_resolved_step_by_step() {
    let src = "enum S { a, b\n flow a -> b after minutes(5)\n final b }\n\
               model Stamps { entered: datetime }\n\
               model Doc { id: uuid, state: S, stamps: Stamps }\n\
               service D at \"/d\" {\n post \"/{id}\" go(id: uuid) -> Doc {\n  fact d: Doc = id\n  \
               transition d.state: a -> b since d.stamps.entered\n }\n}\n";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
    let ir = ir(src);
    let t = ir.services[0].endpoints[0]
        .rules
        .transition
        .as_ref()
        .expect("transition");
    let names: Vec<&str> = t.since.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["stamps", "entered"]);
}

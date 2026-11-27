//! State machines and `transition` (`spec/rules.md` section 10): checks, lowering and ids.

use crate::testing::{codes, ir, one};
use veld_ir::{DeclKind, Enum, Ir};

const MACHINE: &str = r"
enum Status {
  pending, paid, checkedIn, cancelled
  flow pending -> paid -> checkedIn
  flow pending -> cancelled
  flow paid -> cancelled
  final checkedIn, cancelled
}
model Booking { id: uuid, status: Status, total: int }
";

fn service(body: &str) -> String {
    format!("{MACHINE}\nservice Bookings at \"/b\" {{\n{body}\n}}\n")
}

fn endpoint(block: &str) -> String {
    service(&format!(
        "  post \"/{{id}}/x\" act(id: uuid) -> Booking {{\n    fact booking: Booking = id\n{block}\n  }}"
    ))
}

fn status(ir: &Ir) -> Enum {
    ir.decls
        .iter()
        .filter(|d| d.name == "Status")
        .find_map(|d| match &d.kind {
            DeclKind::Enum(e) => Some(e.clone()),
            DeclKind::Model(_)
            | DeclKind::Error(_)
            | DeclKind::Union(_)
            | DeclKind::Const(_)
            | DeclKind::Event(_) => None,
        })
        .expect("Status")
}

fn pairs(e: &Enum) -> Vec<(&str, &str)> {
    e.flows
        .iter()
        .map(|f| (f.from.as_str(), f.to.as_str()))
        .collect()
}

#[test]
fn chains_expand_to_edges_in_first_declaration_order() {
    let e = status(&ir(&service("")));
    assert_eq!(
        pairs(&e),
        [
            ("pending", "paid"),
            ("paid", "checkedIn"),
            ("pending", "cancelled"),
            ("paid", "cancelled")
        ]
    );
    assert_eq!(e.finals, ["checkedIn", "cancelled"]);
}

#[test]
fn a_duplicate_edge_is_kept_once_and_warned_about() {
    let src = "enum S { a, b\n flow a -> b\n flow a -> b\n final b }";
    assert_eq!(codes(src), ["W0930"]);
    assert_eq!(pairs(&status_of(src)), [("a", "b")]);
}

fn status_of(src: &str) -> Enum {
    let ir = ir(src);
    let decl = ir.decls.iter().find(|d| d.name == "S").expect("S");
    let DeclKind::Enum(e) = &decl.kind else {
        panic!("S is not an enum");
    };
    e.clone()
}

#[test]
fn a_self_edge_is_legal_and_leaves_the_state_unstuck() {
    let src = "enum S { a, b\n flow a -> a\n flow a -> b\n final b }";
    assert!(codes(src).is_empty());
    assert_eq!(pairs(&status_of(src)), [("a", "a"), ("a", "b")]);
}

#[test]
fn repeated_finals_are_listed_once_in_declaration_order() {
    let src = "enum S { a, b, c\n flow a -> b\n flow a -> c\n final c, b\n final b }";
    assert_eq!(status_of(src).finals, ["c", "b"]);
}

#[test]
fn unknown_names_are_e0930_in_flows_and_finals() {
    assert_eq!(codes("enum S { a, b\n flow a -> z\n final b }"), ["E0930"]);
    assert_eq!(codes("enum S { a, b\n flow a -> b\n final q }"), ["E0930"]);
}

#[test]
fn a_bad_flow_does_not_make_the_rest_look_stuck() {
    // `b` is entered only by the flow that was dropped: no W0931 on top of the error.
    assert_eq!(
        codes("enum S { a, b\n flow a -> b -> zz\n final b }"),
        ["E0930"]
    );
}

#[test]
fn final_with_a_way_out_is_e0931() {
    assert_eq!(
        codes("enum S { a, b\n flow a -> b\n flow b -> a\n final b }"),
        ["E0931"]
    );
}

#[test]
fn final_without_flow_is_e0931() {
    assert_eq!(codes("enum S { a, b\n final b }"), ["E0931"]);
}

#[test]
fn a_stuck_state_and_a_forgotten_variant_are_different_warnings() {
    assert_eq!(codes("enum S { a, b\n flow a -> b }"), ["W0931"]);
    assert_eq!(
        codes("enum S { a, b, c\n flow a -> b\n final b }"),
        ["W0932"]
    );
}

#[test]
fn flows_do_not_change_the_signature() {
    let plain = ir("enum Status { a, b, c }\nmodel M { s: Status }");
    let machine = ir("enum Status { a, b, c\n flow a -> b -> c\n final c }\nmodel M { s: Status }");
    let sig = |ir: &Ir, n: &str| ir.decls.iter().find(|d| d.name == n).expect(n).sig.clone();
    assert_eq!(sig(&plain, "Status"), sig(&machine, "Status"));
    assert_eq!(sig(&plain, "M"), sig(&machine, "M"));
}

#[test]
fn a_machine_does_not_change_an_ir_without_flows() {
    let json = serde_json::to_string(&ir("enum Status { a, b }")).expect("json");
    assert!(
        !json.contains("flows") && !json.contains("finals"),
        "{json}"
    );
}

// ---- transition -----------------------------------------------------------------------------------

fn transition_of(block: &str) -> veld_ir::EndpointTransition {
    let ir = ir(&endpoint(block));
    ir.services[0].endpoints[0]
        .rules
        .transition
        .clone()
        .expect("a transition")
}

#[test]
fn a_transition_lowers_with_its_machine_sources_and_target() {
    let ir = ir(&endpoint(
        "    transition booking.status: pending, paid -> cancelled",
    ));
    let ep = &ir.services[0].endpoints[0];
    let t = ep.rules.transition.as_ref().expect("transition");
    assert_eq!((t.fact.as_str(), t.to.as_str()), ("booking", "cancelled"));
    assert_eq!(t.from, ["pending", "paid"]);
    assert_eq!(t.steps.len(), 1);
    assert_eq!(
        (t.steps[0].name.as_str(), t.steps[0].wire.as_str()),
        ("status", "status")
    );
    assert!(!t.steps[0].optional);
    assert_eq!(ir.decl(t.decl).name, "Status");
    assert_eq!(t.origin.line, 14);
    assert!(
        t.id.starts_with("veld:app/Bookings.act.transition@"),
        "{}",
        t.id
    );
    assert_eq!(t.id.len(), "veld:app/Bookings.act.transition@".len() + 16);
}

#[test]
fn the_implicit_invalid_transition_is_among_the_errors() {
    let with = ir(&endpoint("    transition booking.status: pending -> paid"));
    let names = |ir: &Ir| -> Vec<String> {
        ir.services[0].endpoints[0]
            .errors
            .iter()
            .map(|e| ir.decl(e.decl).name.clone())
            .collect()
    };
    let mut listed = names(&with);
    listed.sort();
    assert_eq!(
        listed,
        [
            "Internal",
            "InvalidTransition",
            "NotFound",
            "ValidationError"
        ]
    );
    let without = ir(&endpoint(""));
    assert!(!names(&without).contains(&"InvalidTransition".to_owned()));
}

#[test]
fn the_id_ignores_source_order_and_repeats_but_not_the_target_or_sources() {
    let a = transition_of("    transition booking.status: pending, paid -> cancelled").id;
    let b = transition_of("    transition booking.status: paid, pending, paid -> cancelled").id;
    assert_eq!(a, b);
    let c = transition_of("    transition booking.status: paid -> cancelled").id;
    let d = transition_of("    transition booking.status: pending -> paid").id;
    assert_ne!(a, c);
    assert_ne!(c, d);
}

#[test]
fn the_id_uses_wire_values() {
    let wired = "enum Status { pending = \"P\", paid = \"PD\"\n flow pending -> paid\n final paid }\nmodel Booking { id: uuid, status: Status }\nservice B at \"/b\" { post \"/{id}\" act(id: uuid) -> Booking {\n fact booking: Booking = id\n transition booking.status: pending -> paid } }";
    let renamed = wired.replace("pending", "waiting");
    let id = |src: &str| {
        ir(src).services[0].endpoints[0]
            .rules
            .transition
            .clone()
            .expect("transition")
            .id
    };
    assert_eq!(id(wired), id(&renamed));
}

#[test]
fn a_nested_path_lowers_every_step() {
    let src = "enum S { a, b\n flow a -> b\n final b }\nmodel Inner { st: S }\nmodel Outer { id: uuid, inner: Inner }\nservice X at \"/x\" { post \"/{id}\" f(id: uuid) -> 204 {\n fact o: Outer = id\n transition o.inner.st: a -> b } }";
    let t = ir(src).services[0].endpoints[0]
        .rules
        .transition
        .clone()
        .expect("transition");
    let names: Vec<&str> = t.steps.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["inner", "st"]);
}

#[test]
fn transition_errors_have_their_codes() {
    let cases = [
        ("    transition id.status: pending -> paid", "E0932"),
        ("    transition booking.total: pending -> paid", "E0932"),
        ("    transition booking: pending -> paid", "E0932"),
        ("    transition booking.status: paid -> pending", "E0933"),
        ("    transition booking.status: pending -> nowhere", "E0933"),
        ("    transition booking.nope: pending -> paid", "E0901"),
        (
            "    transition booking.status: pending -> paid\n    transition booking.status: pending -> cancelled",
            "E0934",
        ),
    ];
    for (block, code) in cases {
        assert_eq!(codes(&endpoint(block)), [code], "{block}");
    }
}

#[test]
fn a_parameter_or_fact_named_transition_is_e0920_only_with_a_transition() {
    let param = service(
        "  post \"/{id}\" f(id: uuid, @query transition: int) -> Booking {\n    fact booking: Booking = id\n    transition booking.status: pending -> paid\n  }",
    );
    assert_eq!(codes(&param), ["E0920"]);
    let free = service(
        "  post \"/{id}\" f(id: uuid, @query transition: int) -> Booking {\n    fact booking: Booking = id\n  }",
    );
    assert!(codes(&free).is_empty());
}

#[test]
fn a_failed_fact_does_not_cascade_into_the_transition() {
    let out = codes(&endpoint(
        "    fact other: Missing = id\n    transition other.status: pending -> paid",
    ));
    assert_eq!(out, ["E0205"]);
}

#[test]
fn allow_is_a_statement_now_and_an_unknown_policy_is_e0942() {
    let out = one(&endpoint("    allow Whatever(booking)"));
    let found: Vec<&str> = out.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(found, ["E0942"]);
}

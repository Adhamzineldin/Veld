//! Platform phase P2 in the reference evaluator (`spec/platform.md` sections 10 to 13): the
//! pending vector contracts and what the pipeline answers, written down from the spec.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::vectors::{load_cases, run_case};
use veld_rules::{
    FlagState, Outcome, Pipeline, Request, flag_names, run_example, timed_endpoints, timed_moves,
};

fn compile_text(text: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    match out.ir {
        Some(ir) => ir,
        None => panic!("does not compile:\n{}", out.render()),
    }
}

fn vectors(file: &str) -> Ir {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rules/endpoints")
        .join(file);
    compile_text(&std::fs::read_to_string(path).unwrap())
}

fn every_example_passes(file: &str, count: usize) {
    let ir = vectors(file);
    assert_eq!(ir.examples.len(), count, "{file}");
    for example in &ir.examples {
        let report = run_example(&ir, example).unwrap();
        assert!(report.passed(), "{file}: {report}");
    }
}

// --- timed moves ---------------------------------------------------------------------------------

#[test]
fn the_pending_timed_contract_holds_its_eight_examples() {
    every_example_passes("timed.veld.pending", 8);
}

fn pipeline<'a>(ir: &'a Ir, service: &str, endpoint: &str) -> Pipeline<'a> {
    let s = ir.services.iter().find(|s| s.name == service).unwrap();
    Pipeline::new(ir, s, endpoint).unwrap()
}

const BOOKING: &str = "00000000-0000-4000-8000-000000000001";
const CLOCK: i64 = 1_781_179_200_000; // 2026-06-11T12:00:00Z

fn expire(ir: &Ir, since: &str, status: &str, now: i64) -> Outcome {
    let request = Request {
        params: BTreeMap::from([("id".to_owned(), json!(BOOKING))]),
        facts: BTreeMap::from([(
            "booking".to_owned(),
            Some(json!({ "id": BOOKING, "status": status, "pendingSince": since })),
        )]),
        now,
        ..Request::default()
    };
    pipeline(ir, "Bookings", "expireBooking")
        .run(&request)
        .unwrap()
}

fn refusal_message(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Error(e) => e
            .violations
            .first()
            .map(|v| v.message.clone())
            .unwrap_or_default(),
        Outcome::Passes => panic!("expected a refusal"),
    }
}

#[test]
fn a_move_that_is_not_due_says_so_in_its_violation() {
    let ir = vectors("timed.veld.pending");
    let outcome = expire(&ir, "2026-06-10T12:00:00.001Z", "pending", CLOCK);
    assert_eq!(
        refusal_message(&outcome),
        "violates rule transition: not due"
    );
    // A wrong state keeps the message of an ordinary transition.
    let wrong = expire(&ir, "2026-06-10T12:00:00Z", "paid", CLOCK);
    assert_eq!(refusal_message(&wrong), "violates rule transition");
}

#[test]
fn the_boundary_is_exact_to_the_millisecond() {
    let ir = vectors("timed.veld.pending");
    let due = expire(&ir, "2026-06-10T12:00:00Z", "pending", CLOCK);
    assert!(matches!(due, Outcome::Passes), "{due:?}");
    let early = expire(&ir, "2026-06-10T12:00:00Z", "pending", CLOCK - 1);
    assert!(matches!(early, Outcome::Error(_)), "{early:?}");
}

#[test]
fn timed_endpoints_lists_what_a_due_runner_asks_its_loader() {
    let ir = vectors("timed.veld.pending");
    let listed = timed_endpoints(&ir);
    let names: Vec<(&str, &str, i64)> = listed
        .iter()
        .map(|t| (t.endpoint.as_str(), t.to.as_str(), t.after_ms))
        .collect();
    assert_eq!(
        names,
        [
            ("expireBooking", "cancelled", 86_400_000),
            ("archiveBooking", "expired", 172_800_000)
        ]
    );
    assert_eq!(listed[0].from, "pending");
    assert_eq!(listed[0].since, ["pendingSince"]);
    assert_eq!(listed[0].fact, "booking");
}

#[test]
fn timed_moves_lists_the_timed_edges_of_a_machine_in_edge_order() {
    let ir = vectors("timed.veld.pending");
    let machine = ir
        .decls
        .iter()
        .find_map(|d| {
            if let veld_ir::DeclKind::Enum(e) = &d.kind {
                Some(e)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        timed_moves(machine),
        [
            ("pending", "cancelled", 86_400_000),
            ("pending", "expired", 172_800_000)
        ]
    );
}

#[test]
fn a_contract_without_timed_edges_lists_nothing() {
    let ir = compile_text(
        "enum S { a, b\n flow a -> b\n}\nmodel M { id: uuid, s: S }\n\
         service X at \"/x\" { post \"/{id}\" go(id: uuid) -> M { fact m: M = id\n transition m.s: a -> b } }",
    );
    assert!(timed_endpoints(&ir).is_empty());
}

// --- flags ---------------------------------------------------------------------------------------

fn core_vectors(name: &str) -> (Ir, String) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/rules/core");
    let ir =
        compile_text(&std::fs::read_to_string(dir.join(format!("{name}.veld.pending"))).unwrap());
    let cases = std::fs::read_to_string(dir.join(format!("{name}.json.pending"))).unwrap();
    (ir, cases)
}

#[test]
fn the_pending_core_flag_vectors_hold_their_eleven_cases() {
    let (ir, text) = core_vectors("flags");
    let cases = load_cases(&text).unwrap();
    assert_eq!(cases.len(), 11);
    for case in &cases {
        if let Err(m) = run_case(&ir, case) {
            panic!("{m}");
        }
    }
}

#[test]
fn the_pending_endpoint_flag_contract_holds_its_fourteen_examples() {
    every_example_passes("flags.veld.pending", 14);
}

#[test]
fn the_flags_of_a_contract_are_listed_with_what_they_gate() {
    let ir = vectors("flags.veld.pending");
    assert_eq!(
        flag_names(&ir),
        ["archiving", "betaDesk", "lockSuspended", "openOnly"]
    );
    let usage = ir.flag_usage();
    assert_eq!(usage["openOnly"].requires, 1);
    assert_eq!(usage["betaDesk"].permits, 1);
    assert_eq!(usage["lockSuspended"].forbids, 1);
    assert_eq!(usage["archiving"].flows, 1);
    assert_eq!(usage["archiving"].total(), 1);
}

#[test]
fn a_contract_without_flags_has_none() {
    let ir = vectors("timed.veld.pending");
    assert!(flag_names(&ir).is_empty());
    assert!(ir.flag_usage().is_empty());
}

const USER: &str = "00000000-0000-4000-8000-0000000000aa";

fn book(ir: &Ir, roles: &[&str], open: bool) -> Outcome {
    let request = Request {
        params: BTreeMap::from([("id".to_owned(), json!(BOOKING))]),
        facts: BTreeMap::from([(
            "room".to_owned(),
            Some(json!({ "id": BOOKING, "open": open, "capacity": 3, "phase": "draft" })),
        )]),
        principal: Some(json!({ "id": USER, "roles": roles })),
        now: CLOCK,
        ..Request::default()
    };
    pipeline(ir, "Rooms", "bookRoom").run(&request).unwrap()
}

fn denied(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Error(e) if e.name == "Forbidden")
}

#[test]
fn a_failing_provider_restricts_and_does_not_grant() {
    let ir = vectors("flags.veld.pending");
    let failed = FlagState::ProviderFailed.resolve(&ir);
    // The flagged forbid counts as on: a staff member who is suspended is refused.
    assert!(denied(&book(&failed, &["staff", "suspended"], true)));
    // The flagged permit counts as off: a beta user is not permitted.
    assert!(denied(&book(&failed, &["beta"], true)));
    // The flagged require counts as on: a closed room is refused.
    let closed = book(&failed, &["staff"], false);
    assert!(
        matches!(closed, Outcome::Error(ref e) if e.name == "ValidationError"),
        "{closed:?}"
    );
    // The flagged flow counts as off: the machine has only draft -> live.
    let machine = failed.decls.iter().find_map(|d| {
        if let veld_ir::DeclKind::Enum(e) = &d.kind {
            Some(e)
        } else {
            None
        }
    });
    assert_eq!(machine.unwrap().flows.len(), 1);
}

#[test]
fn with_every_flag_off_the_flagged_statements_are_gone() {
    let ir = vectors("flags.veld.pending");
    let off = FlagState::default().resolve(&ir);
    assert!(off.flag_usage().is_empty());
    assert_eq!(book(&off, &["staff", "suspended"], false), Outcome::Passes);
    let on = FlagState::on(["lockSuspended"]).resolve(&ir);
    assert!(denied(&book(&on, &["staff", "suspended"], true)));
}

#[test]
fn resolving_twice_changes_nothing() {
    let ir = vectors("flags.veld.pending");
    let once = FlagState::on(["openOnly", "archiving"]).resolve(&ir);
    let twice = FlagState::default().resolve(&once);
    assert_eq!(once, twice);
}

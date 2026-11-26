//! Platform phase P2 in the reference evaluator (`spec/platform.md` sections 10 to 13): the
//! pending vector contracts and what the pipeline answers, written down from the spec.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::{Outcome, Pipeline, Request, run_example, timed_endpoints, timed_moves};

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
        .find_map(|d| match &d.kind {
            veld_ir::DeclKind::Enum(e) => Some(e),
            _ => None,
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

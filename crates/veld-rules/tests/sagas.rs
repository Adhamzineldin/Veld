//! The reference orchestrator of sagas (`spec/platform.md` sections 17 and 18): the shared script
//! vectors, the pending example contract, the stops and restarts, and the pieces a generated
//! orchestrator copies (the id of a saga, the key of a call, the members of the saved state).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::run_example;
use veld_rules::saga::vectors::{SagaCase, load_cases, run_case};
use veld_rules::saga::{
    Answer, SagaInput, Script, Status, StepStatus, Stop, run_saga, saga_id, step_key,
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

fn rules_file(dir: &str, file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rules")
        .join(dir)
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn checkout() -> Ir {
    compile_text(&rules_file("sagas", "checkout.veld"))
}

fn cases() -> Vec<SagaCase> {
    load_cases(&rules_file("sagas", "orchestrator.json")).unwrap()
}

// ---- the shared vectors ---------------------------------------------------------------------------

#[test]
fn the_script_vectors_hold_twenty_four_cases_and_the_orchestrator_meets_every_one() {
    let ir = checkout();
    let cases = cases();
    assert_eq!(cases.len(), 24);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|c| run_case(&ir, c).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_case_is_deterministic() {
    let ir = checkout();
    for c in cases() {
        let a = run_case(&ir, &c).unwrap();
        let b = run_case(&ir, &c).unwrap();
        assert_eq!(a, b, "{}", c.name);
    }
}

fn run_with(ir: &Ir, c: &SagaCase, script: Script) -> veld_rules::saga::SagaRun {
    let service = ir
        .services
        .iter()
        .find(|s| s.name == "Checkout")
        .unwrap_or_else(|| panic!("no Checkout"));
    let name = c.endpoint.split_once('.').map(|x| x.1).unwrap_or_default();
    let endpoint = service.endpoints.iter().find(|e| e.name == name).unwrap();
    let input = SagaInput {
        ir,
        service,
        endpoint,
        params: c.params.clone(),
        id: c.id.clone(),
        scope: c.scope.clone(),
        start_ms: c.start_ms,
    };
    run_saga(&input, &script).unwrap()
}

/// The gate of section 21 in the small: stop after every save of a run and start again, and the
/// run ends where it ended without the stop, with the same calls and the same saves.
#[test]
fn stopping_after_every_save_changes_nothing() {
    let ir = checkout();
    for c in cases().iter().filter(|c| c.script.stops.is_empty()) {
        let baseline = run_with(&ir, c, c.script.clone());
        for n in 1..=baseline.saves.len() {
            let mut script = c.script.clone();
            script.stops = vec![Stop {
                after_save: Some(n),
                after_call: None,
                down_ms: 0,
            }];
            let run = run_with(&ir, c, script);
            assert_eq!(
                run.state, baseline.state,
                "{} stopped after save {n}",
                c.name
            );
            assert_eq!(run.calls, baseline.calls, "{} after save {n}", c.name);
            assert_eq!(run.saves, baseline.saves, "{} after save {n}", c.name);
            assert_eq!(
                run.compensated, baseline.compensated,
                "{} after {n}",
                c.name
            );
            assert_eq!(run.restarts, 1, "{} after save {n}", c.name);
        }
    }
}

/// A call that was made and whose answer was lost is made again with the same key: the run
/// ends in the same state, with one more call.
#[test]
fn stopping_after_every_call_repeats_that_call_and_nothing_else() {
    let ir = checkout();
    for c in cases().iter().filter(|c| c.script.stops.is_empty()) {
        let baseline = run_with(&ir, c, c.script.clone());
        // A long run (a call that never answers) is stopped at a few of its calls only.
        for n in (1..=baseline.calls.len()).take(6) {
            let mut script = c.script.clone();
            script.stops = vec![Stop {
                after_save: None,
                after_call: Some(n),
                down_ms: 0,
            }];
            let run = run_with(&ir, c, script);
            assert_eq!(
                run.state.status, baseline.state.status,
                "{} call {n}",
                c.name
            );
            assert_eq!(
                run.state.answer, baseline.state.answer,
                "{} call {n}",
                c.name
            );
            assert_eq!(run.compensated, baseline.compensated, "{} call {n}", c.name);
            assert_eq!(run.restarts, 1, "{} call {n}", c.name);
            // The call after the stop is the stopped call again, with the next attempt.
            let (before, again) = (&run.calls[n - 1], &run.calls[n]);
            assert_eq!(before.key, again.key, "{} call {n}", c.name);
            assert_eq!(before.args, again.args, "{} call {n}", c.name);
            assert_eq!(again.attempt, before.attempt + 1, "{} call {n}", c.name);
        }
    }
}

// ---- the pending example contract -------------------------------------------------------------------

#[test]
fn the_pending_saga_contract_holds_its_eleven_examples() {
    let ir = compile_text(&rules_file("endpoints", "sagas.veld.pending"));
    assert_eq!(ir.examples.len(), 11);
    for example in &ir.examples {
        let report = run_example(&ir, example).unwrap();
        assert!(report.passed(), "{report}");
    }
}

#[test]
fn a_saga_example_that_expects_the_wrong_steps_fails() {
    let text = rules_file("endpoints", "sagas.veld.pending").replace(
        "expect compensated charge, hold",
        "expect compensated hold, charge",
    );
    let ir = compile_text(&text);
    let failed: Vec<&str> = ir
        .examples
        .iter()
        .filter(|e| !run_example(&ir, e).unwrap().passed())
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(
        failed,
        ["the last step is refused: the others are undone in reverse order"]
    );
}

#[test]
fn a_saga_example_that_expects_stuck_but_ends_compensated_fails() {
    let text =
        rules_file("endpoints", "sagas.veld.pending").replace("  given undo hold = NotFound\n", "");
    let ir = compile_text(&text);
    let report = ir
        .examples
        .iter()
        .find(|e| e.name == "a compensation that answers an error ends the saga stuck")
        .map(|e| run_example(&ir, e).unwrap())
        .unwrap();
    assert!(!report.passed(), "{report}");
}

// ---- what a generated orchestrator copies --------------------------------------------------------------

#[test]
fn the_saga_id_is_the_key_and_the_caller_is_part_of_it_when_the_keys_are_scoped() {
    assert_eq!(saga_id(None, "k-1"), "k-1");
    let a = saga_id(Some("alice"), "k-1");
    let b = saga_id(Some("bob"), "k-1");
    assert_ne!(a, b);
    assert!(a.ends_with(".k-1"));
    assert_eq!(a.len(), 16 + 1 + 3);
    assert!(a[..16].bytes().all(|c| c.is_ascii_hexdigit()));
    // The digest of `alice`, written down.
    assert_eq!(&a[..16], "2bd806c97f0e00af");
}

#[test]
fn the_key_of_a_call_is_the_id_a_dot_and_the_step_with_undo_for_a_compensation() {
    assert_eq!(step_key("k-1", "hold", false), "k-1.hold");
    assert_eq!(step_key("k-1", "hold", true), "k-1.hold.undo");
}

#[test]
fn a_key_longer_than_255_characters_is_its_hash() {
    let id = "x".repeat(250);
    let key = step_key(&id, "hold", true);
    assert_eq!(key.len(), 64);
    assert!(key.bytes().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(key, step_key(&id, "hold", true));
    assert_ne!(key, step_key(&id, "hold", false));
    // 255 is still the key itself.
    let id = "y".repeat(255 - ".hold".len());
    assert_eq!(step_key(&id, "hold", false).len(), 255);
}

#[test]
fn the_script_answers_by_attempt_and_repeats_its_last_answer() {
    let script: Script = serde_json::from_value(json!({
        "steps": { "hold": [ { "kind": "none" }, { "kind": "ok", "value": 1 } ] }
    }))
    .unwrap();
    assert_eq!(script.answer("hold", false, 1), Answer::none());
    assert_eq!(
        script.answer("hold", false, 2),
        Answer::Ok {
            value: Some(json!(1)),
            after_ms: 0
        }
    );
    assert_eq!(
        script.answer("hold", false, 9),
        script.answer("hold", false, 2)
    );
    // What the script does not name answers a success; an undo has its own list.
    assert_eq!(script.answer("other", false, 1), Answer::ok());
    assert_eq!(script.answer("hold", true, 1), Answer::ok());
}

#[test]
fn a_script_that_names_a_step_the_saga_does_not_have_is_refused() {
    let ir = checkout();
    let mut c = cases().remove(0);
    c.script.steps.insert("nope".into(), vec![Answer::ok()]);
    let err = run_case(&ir, &c).unwrap_err();
    assert!(err.contains("nope"), "{err}");
}

#[test]
fn the_state_has_the_members_of_the_spec() {
    let ir = checkout();
    let c = &cases()[2];
    let run = run_case(&ir, c).unwrap();
    let v = serde_json::to_value(&run.state).unwrap();
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "answer",
            "deadline",
            "failedStep",
            "id",
            "operation",
            "parameters",
            "scope",
            "status",
            "steps"
        ]
    );
    assert_eq!(v["status"], "compensated");
    assert_eq!(v["failedStep"], "charge");
    assert_eq!(v["deadline"], 30_000);
    assert_eq!(v["scope"], serde_json::Value::Null);
    assert_eq!(v["steps"][0]["name"], "hold");
    assert_eq!(v["steps"][0]["status"], "undone");
    assert_eq!(
        v["steps"][0]["value"],
        json!({"id": "h-1", "bookingId": "b-1"})
    );
    assert_eq!(v["steps"][1]["status"], "failed");
    assert_eq!(v["steps"][1]["error"]["error"], "PaymentDeclined");
    assert_eq!(v["steps"][1]["error"]["status"], 402);
    assert!(v["steps"][1].get("value").is_none());
    assert_eq!(v["steps"][2]["status"], "pending");
    assert_eq!(
        v["answer"],
        json!({"status": 402, "error": "PaymentDeclined", "body": {"reason": "funds"}})
    );
    // It reads back.
    let back: veld_rules::saga::SagaState = serde_json::from_value(v).unwrap();
    assert_eq!(back, run.state);
}

#[test]
fn the_first_save_creates_the_state_with_the_parameters_and_the_deadline() {
    let ir = checkout();
    let c = &cases()[0];
    let run = run_case(&ir, c).unwrap();
    let first = &run.saves[0];
    assert_eq!(first.status, Status::Running);
    assert_eq!(first.parameters, c.params);
    assert_eq!(first.deadline, 30_000);
    assert!(first.steps.iter().all(|s| s.status == StepStatus::Pending));
    assert!(
        first.operation.starts_with("veld:t/Checkout.checkout@"),
        "{}",
        first.operation
    );
    assert_eq!(
        first.operation.len(),
        "veld:t/Checkout.checkout@".len() + 16
    );
}

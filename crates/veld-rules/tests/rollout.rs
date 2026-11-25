//! `@rollout(audit)` in the reference evaluator (`spec/rules.md` section 17.1): the shared
//! vectors `tests/rules/core/rollout.*.pending` and `tests/rules/endpoints/rollout.veld.pending`,
//! and what the pipeline reports besides its outcome.
//!
//! The vector files carry the suffix `.pending` so that the walkers of the generators, which look
//! for `.veld` and `.json`, do not pick them up before a generator supports the feature. This
//! file reads them by their explicit names.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::vectors::{Case, Expected, Mismatch, load_cases, run_case};
use veld_rules::{
    Options, Outcome, OutputMode, Pipeline, Request, check_value, check_value_audited, run_example,
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

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    std::fs::read_to_string(path).unwrap()
}

// --- the shared vectors ----------------------------------------------------------------------

#[test]
fn the_reference_evaluator_passes_the_core_rollout_vectors() {
    let ir = compile_text(&read("tests/rules/core/rollout.veld.pending"));
    let cases = load_cases(&read("tests/rules/core/rollout.json.pending")).unwrap();
    assert_eq!(cases.len(), 8);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|c| run_case(&ir, c).err().map(|m| m.to_string()))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // The vectors state both lists: some case has audited violations and some has none.
    assert!(cases.iter().any(|c| !c.audited.is_empty()));
    assert!(cases.iter().any(|c| c.audited.is_empty()));
}

#[test]
fn a_vector_that_states_the_audited_violations_wrongly_fails() {
    let ir = compile_text(&read("tests/rules/core/rollout.veld.pending"));
    let mut cases = load_cases(&read("tests/rules/core/rollout.json.pending")).unwrap();
    let mut case: Case = cases.remove(1);
    case.audited.clear();
    assert!(matches!(
        run_case(&ir, &case),
        Err(Mismatch::AuditedDiffers { .. })
    ));
    let mut case: Case = load_cases(&read("tests/rules/core/rollout.json.pending"))
        .unwrap()
        .remove(1);
    case.expect.push(Expected {
        rule: "smallParty".into(),
        path: "$".into(),
        fields: vec!["guests".into()],
        error: None,
    });
    assert!(matches!(
        run_case(&ir, &case),
        Err(Mismatch::Differs { .. })
    ));
}

// --- check_value ------------------------------------------------------------------------------

const PARTY: &str = "model Party {\n guests: int\n @rollout(audit)\n rule small: guests <= 6\n rule positive: guests > 0\n}\n";

fn party() -> Ir {
    compile_text(PARTY)
}

#[test]
fn check_value_returns_only_the_violations_that_reject() {
    let ir = party();
    let decl = ir.decls.iter().find(|d| d.name == "Party").unwrap();
    let value = json!({ "guests": 7 });
    assert!(
        check_value(&ir, decl, &value, Options::default())
            .unwrap()
            .is_empty()
    );
    let found = check_value_audited(&ir, decl, &value, Options::default()).unwrap();
    assert!(found.violations.is_empty());
    let names: Vec<&str> = found.audited.iter().map(|v| v.rule_name.as_str()).collect();
    assert_eq!(names, ["small"]);
    let found =
        check_value_audited(&ir, decl, &json!({ "guests": 0 }), Options::default()).unwrap();
    let names: Vec<&str> = found
        .violations
        .iter()
        .map(|v| v.rule_name.as_str())
        .collect();
    assert_eq!(names, ["positive"]);
    assert!(found.audited.is_empty());
}

#[test]
fn the_partial_check_skips_audited_rules() {
    let ir = party();
    let decl = ir.decls.iter().find(|d| d.name == "Party").unwrap();
    let options = Options {
        request: false,
        partial: true,
    };
    let found = check_value_audited(&ir, decl, &json!({ "guests": 7 }), options).unwrap();
    assert!(found.violations.is_empty() && found.audited.is_empty());
}

// --- the pipeline -----------------------------------------------------------------------------

const SERVICE: &str = r#"
model Room { id: uuid, capacity: int }
model Party {
  roomId: uuid
  guests: int
  @rollout(audit)
  rule small: guests <= 6
}
model Booking {
  id: uuid
  guests: int
  @rollout(audit)
  rule sane: guests <= 100
}
error RoomFull 409 { capacity: int }

service Parties at "/p" {
  post "/" book(body: Party) -> 201 Booking | RoomFull {
    fact room: Room = body.roomId
    @rollout(audit)
    require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
    require body.guests > 0
    @rollout(audit)
    ensure answered: result.guests == body.guests
    ensure result.guests >= 0
  }
}
"#;

const ROOM: &str = "00000000-0000-4000-8000-000000000001";
const ID: &str = "00000000-0000-4000-8000-000000000002";

fn request(guests: i64, capacity: i64) -> Request {
    let mut req = Request::default();
    req.params
        .insert("body".into(), json!({ "roomId": ROOM, "guests": guests }));
    req.facts.insert(
        "room".into(),
        Some(json!({ "id": ROOM, "capacity": capacity })),
    );
    req.now = 0;
    req
}

fn names(vs: &[veld_rules::Violation]) -> Vec<&str> {
    vs.iter().map(|v| v.rule_name.as_str()).collect()
}

#[test]
fn an_audited_invariant_of_the_body_is_reported_and_the_request_goes_on() {
    let ir = compile_text(SERVICE);
    let p = Pipeline::new(&ir, &ir.services[0], "book").unwrap();
    let r = p
        .run_reported(&request(7, 100), OutputMode::Strict)
        .unwrap();
    assert_eq!(r.outcome, Outcome::Passes);
    assert_eq!(names(&r.audited), ["small"]);
    assert_eq!(r.audited[0].path, "body");
    assert!(r.hook.is_empty());
    assert_eq!(p.run(&request(7, 100)).unwrap(), Outcome::Passes);
}

#[test]
fn an_audited_require_with_else_never_answers_its_error() {
    let ir = compile_text(SERVICE);
    let p = Pipeline::new(&ir, &ir.services[0], "book").unwrap();
    let r = p.run_reported(&request(3, 2), OutputMode::Strict).unwrap();
    assert_eq!(r.outcome, Outcome::Passes);
    assert_eq!(names(&r.audited), ["require"]);
    assert_eq!(r.audited[0].path, "request");
    assert_eq!(r.audited[0].fields, ["body"]);
}

#[test]
fn an_enforced_require_after_a_violated_audited_one_still_rejects_without_naming_it() {
    let ir = compile_text(SERVICE);
    let p = Pipeline::new(&ir, &ir.services[0], "book").unwrap();
    // guests 0 breaks `guests > 0` and nothing else audited except `guests <= capacity` holds.
    let r = p
        .run_reported(&request(0, 100), OutputMode::Strict)
        .unwrap();
    let Outcome::Error(a) = r.outcome else {
        panic!("expected a rejection");
    };
    assert_eq!((a.name.as_str(), a.status), ("ValidationError", 422));
    assert_eq!(names(&a.violations), ["require"]);
    assert!(r.audited.is_empty());
    // Both fail: the audited one is reported, the enforced one answers, only it is in violations.
    let r = p
        .run_reported(&request(-1, -5), OutputMode::Strict)
        .unwrap();
    let Outcome::Error(a) = r.outcome else {
        panic!("expected a rejection");
    };
    assert_eq!(a.violations.len(), 1);
    assert_eq!(names(&r.audited), ["require"]);
}

fn with_result(mut req: Request, guests: i64) -> Request {
    req.result = Some(json!({ "id": ID, "guests": guests }));
    req
}

#[test]
fn an_audited_ensure_and_an_audited_result_invariant_never_turn_into_500() {
    let ir = compile_text(SERVICE);
    let p = Pipeline::new(&ir, &ir.services[0], "book").unwrap();
    for mode in [OutputMode::Strict, OutputMode::Audit] {
        let r = p
            .run_reported(&with_result(request(2, 100), 101), mode)
            .unwrap();
        assert_eq!(r.outcome, Outcome::Passes, "{mode:?}");
        // `sane` of the result, then the audited `ensure`, in the order they are evaluated.
        assert_eq!(names(&r.audited), ["sane", "answered"], "{mode:?}");
        assert_eq!(r.audited[0].path, "result");
        assert!(r.hook.is_empty());
    }
}

#[test]
fn output_mode_off_skips_audited_output_rules_and_audit_keeps_reporting_them() {
    let ir = compile_text(SERVICE);
    let p = Pipeline::new(&ir, &ir.services[0], "book").unwrap();
    let req = with_result(request(2, 100), 3);
    let off = p.run_reported(&req, OutputMode::Off).unwrap();
    assert_eq!(off.outcome, Outcome::Passes);
    assert!(off.audited.is_empty() && off.hook.is_empty());
    let strict = p.run_reported(&req, OutputMode::Strict).unwrap();
    assert_eq!(names(&strict.audited), ["answered"]);
    // Off skips output rules, not input rules: an audited input rule is still reported.
    let off = p
        .run_reported(&with_result(request(7, 100), 7), OutputMode::Off)
        .unwrap();
    assert_eq!(names(&off.audited), ["small"]);
}

#[test]
fn an_enforced_output_rule_is_500_in_strict_and_only_reported_in_audit() {
    let ir = compile_text(SERVICE);
    let p = Pipeline::new(&ir, &ir.services[0], "book").unwrap();
    let req = with_result(request(2, 100), -1);
    let strict = p.run_reported(&req, OutputMode::Strict).unwrap();
    let Outcome::Error(a) = strict.outcome else {
        panic!("strict must reject");
    };
    assert_eq!((a.name.as_str(), a.status), ("Internal", 500));
    assert_eq!(names(&a.hook), ["ensure"]);
    let audit = p.run_reported(&req, OutputMode::Audit).unwrap();
    assert_eq!(audit.outcome, Outcome::Passes);
    assert_eq!(names(&audit.hook), ["ensure"]);
    assert_eq!(names(&audit.audited), ["answered"]);
}

#[test]
fn run_and_before_after_keep_their_meaning_without_audited_rules() {
    let service = "service S at \"/s\" { post \"/\" f(body: Party) -> 204 }";
    let ir = compile_text(&format!("{PARTY}{service}"));
    let p = Pipeline::new(&ir, &ir.services[0], "f").unwrap();
    let mut req = Request::default();
    req.params.insert("body".into(), json!({ "guests": 8 }));
    assert_eq!(p.run(&req).unwrap(), Outcome::Passes);
    let mut audited = Vec::new();
    let before = p.before_audited(&req, &mut audited).unwrap();
    assert!(matches!(before, veld_rules::Before::Ready(_)));
    assert_eq!(names(&audited), ["small"]);
}

// --- the shared endpoint examples --------------------------------------------------------------

fn endpoint_ir() -> Ir {
    compile_text(&read("tests/rules/endpoints/rollout.veld.pending"))
}

#[test]
fn the_reference_evaluator_passes_the_rollout_examples() {
    let ir = endpoint_ir();
    assert_eq!(ir.examples.len(), 11);
    for ex in &ir.examples {
        let report = run_example(&ir, ex).unwrap();
        assert!(report.passed(), "{report}");
    }
}

fn audited_of(ir: &Ir, example: &str) -> Vec<String> {
    let ex = ir.examples.iter().find(|e| e.name == example).unwrap();
    let report = run_example(ir, ex).unwrap();
    report.audited.iter().map(|v| v.rule_name.clone()).collect()
}

#[test]
fn an_example_lists_the_audited_rules_it_violates_and_still_passes() {
    let ir = endpoint_ir();
    let cases: [(&str, &[&str]); 7] = [
        (
            "an audited invariant that is violated does not reject the request",
            &["smallParty"],
        ),
        ("the same invariant enforced rejects it", &[]),
        ("an audited rule that holds changes nothing", &[]),
        (
            "an audited require with else that is violated is not answered",
            &["require"],
        ),
        (
            "an enforced require after an audited one that is violated still answers",
            &["require"],
        ),
        (
            "an audited ensure that the result breaks leaves the outcome ok",
            &["ensure"],
        ),
        (
            "an audited invariant of the result that it breaks leaves the outcome ok",
            &["sane"],
        ),
    ];
    for (name, want) in cases {
        let got = audited_of(&ir, name);
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn an_audited_violation_is_never_among_the_violations_of_the_answer() {
    let ir = endpoint_ir();
    let ex = ir
        .examples
        .iter()
        .find(|e| {
            e.name
                .starts_with("an audited invariant next to an enforced one")
        })
        .unwrap();
    let report = run_example(&ir, ex).unwrap();
    assert!(report.passed(), "{report}");
    assert_eq!(report.audited.len(), 1);
    assert_eq!(report.audited[0].rule_name, "smallParty");
    let Some(Outcome::Error(a)) = report.outcome else {
        panic!("expected a rejection");
    };
    assert_eq!(names(&a.violations), ["atMostTen"]);
}

/// `veld check` fills in the example and refuses a filled-in value that breaks an invariant
/// (`E0953`). An audited invariant is not enforced, so it cannot make an example wrong.
#[test]
fn a_filled_in_value_that_breaks_only_an_audited_rule_is_not_an_e0953() {
    let src = |annotation: &str| {
        format!(
            "model P {{\n guests: int\n {annotation}\n rule atLeastOne: guests >= 1\n}}\n\
             service S at \"/s\" {{ post \"/\" f(body: P) -> 204 }}\n\
             example \"left out\" {{ f(body: {{}}) -> ok }}\n"
        )
    };
    let audited = compile_text(&src("@rollout(audit)"));
    assert!(veld_rules::fill_example(&audited, &audited.examples[0]).is_ok());
    let report = run_example(&audited, &audited.examples[0]).unwrap();
    assert!(report.passed(), "{report}");
    assert_eq!(report.audited.len(), 1);
    let enforced = compile_text(&src(""));
    let err = veld_rules::fill_example(&enforced, &enforced.examples[0]).unwrap_err();
    assert_eq!(err.code(), "E0953");
}

#[test]
fn a_contract_without_rollout_reports_nothing_audited() {
    let ir = compile_text(&read("tests/rules/endpoints/bookings.veld"));
    for ex in &ir.examples {
        let report = run_example(&ir, ex).unwrap();
        assert!(report.audited.is_empty(), "{}", ex.name);
    }
}

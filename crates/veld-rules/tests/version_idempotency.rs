//! `@version` and `@idempotent` in the reference evaluator (`spec/rules.md` sections 11.7 to
//! 11.9): the pending vector contracts, and what the pipeline answers and reports, member by
//! member, written down from the spec. A live run holds every generated server to these answers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value as Json, json};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::{
    ErrorAnswer, IdempotencyKey, Outcome, OutputMode, Pipeline, Reported, Request, Scope,
    StoreEffect, StoreState, StoredAnswer, etag, run_example,
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

// --- the pending vector contracts ----------------------------------------------------------------

fn every_example_passes(file: &str, count: usize) {
    let ir = vectors(file);
    assert_eq!(ir.examples.len(), count, "{file}");
    for example in &ir.examples {
        let report = run_example(&ir, example).unwrap();
        assert!(report.passed(), "{file}: {report}");
    }
}

#[test]
fn the_pending_version_contract_holds_its_nineteen_examples() {
    every_example_passes("version.veld.pending", 19);
}

#[test]
fn the_pending_idempotency_contract_holds_its_twenty_one_examples() {
    every_example_passes("idempotency.veld.pending", 21);
}

// --- a contract of our own ---------------------------------------------------------------------------

const SOURCE: &str = r#"
model Room {
  id: uuid
  name: string
  @version
  revision: long
}
model Counter {
  id: uuid
  @version
  version: int
}
model Plain { id: uuid }
model Rename { name: string }
model Body {
  n: int
  rule positive: n > 0
}
error Gone 410 { why: string }

service Rooms at "/r" {
  put "/{id}" rename(id: uuid, body: Rename) -> Room | Gone {
    fact room: Room = id
  }
  patch "/{id}" touch(id: uuid) -> Room {
    fact maybe?: Room = id
  }
  post "/{id}/tick" tick(id: uuid) -> Counter {
    fact counter: Counter = id
  }
  get "/{id}" one(id: uuid) -> Room
  get "/p/{id}" plain(id: uuid) -> Plain {
    fact plain: Plain = id
  }
}

service Keys at "/k" {
  @idempotent
  post "/" make(body: Body) -> 201 Body | Gone {
    fact seed: Plain
    ensure result.n < 100
  }
  @idempotent(required: true, header: "X-Req")
  post "/must" must(body: Body) -> 201 Body
  @auth(bearer)
  @idempotent
  post "/mine" mine(body: Body) -> 201 Body
}
"#;

const ID: &str = "00000000-0000-4000-8000-000000000001";

fn pipeline<'a>(ir: &'a Ir, service: &str, endpoint: &str) -> Pipeline<'a> {
    let s = ir.services.iter().find(|s| s.name == service).unwrap();
    Pipeline::new(ir, s, endpoint).unwrap()
}

fn rename(revision: &str, if_match: Option<&str>) -> Request {
    let mut r = Request::default();
    r.params.insert("id".into(), json!(ID));
    r.params.insert("body".into(), json!({ "name": "Blue" }));
    r.facts.insert(
        "room".into(),
        Some(json!({ "id": ID, "name": "Red", "revision": revision })),
    );
    r.if_match = if_match.map(str::to_owned);
    r
}

fn answer(r: &Reported) -> &ErrorAnswer {
    match &r.outcome {
        Outcome::Error(a) => a,
        Outcome::Passes => panic!("expected an error"),
    }
}

// --- If-Match -----------------------------------------------------------------------------------------

#[test]
fn a_stale_tag_is_412_with_expected_actual_and_the_version_rule() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "Rooms", "rename");
    let r = p
        .run_reported(&rename("4", Some("\"3\"")), OutputMode::Strict)
        .unwrap();
    let a = answer(&r);
    assert_eq!((a.name.as_str(), a.status), ("PreconditionFailed", 412));
    assert_eq!(a.data, json!({ "expected": "3", "actual": "4" }));
    assert_eq!(a.violations.len(), 1);
    let v = &a.violations[0];
    assert_eq!(v.rule_name, "version");
    assert!(
        v.rule_id.starts_with("veld:t/Rooms.rename.version@"),
        "{}",
        v.rule_id
    );
    assert_eq!(v.rule_id.len(), "veld:t/Rooms.rename.version@".len() + 16);
    assert_eq!(
        (v.path.as_str(), v.message.as_str()),
        ("request", "violates rule version")
    );
    assert!(v.fields.is_empty() && v.error.is_none());
    assert!(a.hook.is_empty());
    assert!(!r.replayed && r.checked_version.is_none());
}

#[test]
fn the_tag_that_matches_passes_and_the_handler_gets_the_version_that_was_checked() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "Rooms", "rename");
    let r = p
        .run_reported(&rename("4", Some("\"4\"")), OutputMode::Strict)
        .unwrap();
    assert_eq!(r.outcome, Outcome::Passes);
    assert_eq!(r.checked_version, Some(4));
    // A star passes, and so does no header: in both the handler gets no version to write against.
    for header in [Some("*"), None] {
        let r = p
            .run_reported(&rename("4", header), OutputMode::Strict)
            .unwrap();
        assert_eq!(r.outcome, Outcome::Passes, "{header:?}");
        assert_eq!(r.checked_version, None, "{header:?}");
    }
}

#[test]
fn a_malformed_if_match_is_422_with_one_issue_and_no_violations() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "Rooms", "rename");
    for bad in [
        "3",
        "\"03\"",
        "W/\"3\"",
        "\"3\", \"4\"",
        "\"\"",
        "\"-1\"",
        " *",
        "\"9223372036854775808\"",
    ] {
        let r = p
            .run_reported(&rename("4", Some(bad)), OutputMode::Strict)
            .unwrap();
        let a = answer(&r);
        assert_eq!(
            (a.name.as_str(), a.status),
            ("ValidationError", 422),
            "{bad}"
        );
        assert_eq!(
            a.data,
            json!({ "issues": [{
                "path": "header.If-Match",
                "code": "type",
                "message": "must be * or one strong entity tag holding a version, such as \"3\""
            }] }),
            "{bad}"
        );
        assert!(a.violations.is_empty(), "{bad}");
    }
}

#[test]
fn an_endpoint_without_a_guard_ignores_if_match() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "Rooms", "plain");
    let mut r = Request::default();
    r.params.insert("id".into(), json!(ID));
    r.facts.insert("plain".into(), Some(json!({ "id": ID })));
    r.if_match = Some("not even a tag".into());
    let out = p.run_reported(&r, OutputMode::Strict).unwrap();
    assert_eq!(out.outcome, Outcome::Passes);
    assert_eq!(out.checked_version, None);
}

#[test]
fn an_int_version_and_an_absent_optional_fact() {
    let ir = compile_text(SOURCE);
    let tick = pipeline(&ir, "Rooms", "tick");
    let mut r = Request::default();
    r.params.insert("id".into(), json!(ID));
    r.facts
        .insert("counter".into(), Some(json!({ "id": ID, "version": 2 })));
    r.if_match = Some("\"1\"".into());
    let a = tick.run_reported(&r, OutputMode::Strict).unwrap();
    assert_eq!(answer(&a).data, json!({ "expected": "1", "actual": "2" }));
    let touch = pipeline(&ir, "Rooms", "touch");
    let mut r = Request::default();
    r.params.insert("id".into(), json!(ID));
    r.facts.insert("maybe".into(), None);
    r.if_match = Some("\"1\"".into());
    let out = touch.run_reported(&r, OutputMode::Strict).unwrap();
    assert_eq!(out.outcome, Outcome::Passes);
}

#[test]
fn the_etag_is_the_version_of_the_returned_value() {
    let ir = compile_text(SOURCE);
    let svc = |n: &str| ir.services.iter().find(|s| s.name == n).unwrap();
    let ep = |s: &str, e: &str| svc(s).endpoints.iter().find(|x| x.name == e).unwrap();
    let room = json!({ "id": ID, "name": "Red", "revision": "7" });
    assert_eq!(
        etag(&ir, ep("Rooms", "one"), &room),
        Some("\"7\"".to_owned())
    );
    let counter = json!({ "id": ID, "version": 12 });
    assert_eq!(
        etag(&ir, ep("Rooms", "tick"), &counter),
        Some("\"12\"".to_owned())
    );
    assert_eq!(etag(&ir, ep("Rooms", "plain"), &json!({ "id": ID })), None);
    // A success reports it: a run with a result carries the `ETag`.
    let p = pipeline(&ir, "Rooms", "one");
    let mut r = Request::default();
    r.params.insert("id".into(), json!(ID));
    r.result = Some(room);
    let out = p.run_reported(&r, OutputMode::Strict).unwrap();
    assert_eq!(out.etag.as_deref(), Some("\"7\""));
    assert!(out.body.is_some());
    // An error carries none.
    let p = pipeline(&ir, "Rooms", "rename");
    let out = p
        .run_reported(&rename("4", Some("\"3\"")), OutputMode::Strict)
        .unwrap();
    assert_eq!(out.etag, None);
}

// --- the key ----------------------------------------------------------------------------------------------

fn keyed(endpoint: &str, key: Option<(&str, StoreState)>, n: i64) -> Request {
    let mut r = Request::default();
    r.params.insert("body".into(), json!({ "n": n }));
    r.facts.insert("seed".into(), Some(json!({ "id": ID })));
    r.key = key.map(|(v, store)| IdempotencyKey {
        value: v.to_owned(),
        store,
    });
    let _ = endpoint;
    r
}

fn run(ir: &Ir, endpoint: &str, r: &Request) -> Reported {
    pipeline(ir, "Keys", endpoint)
        .run_reported(r, OutputMode::Strict)
        .unwrap()
}

#[test]
fn a_required_key_that_is_missing_is_422_at_the_header_the_annotation_names() {
    let ir = compile_text(SOURCE);
    let r = run(&ir, "must", &keyed("must", None, 1));
    let a = answer(&r);
    assert_eq!((a.name.as_str(), a.status), ("ValidationError", 422));
    assert_eq!(
        a.data,
        json!({ "issues": [{ "path": "header.X-Req", "code": "required", "message": "is required" }] })
    );
    assert!(a.violations.is_empty());
    assert_eq!(r.store, StoreEffect::Untouched);
}

#[test]
fn a_key_that_is_not_a_key_is_422_and_an_optional_missing_one_is_not() {
    let ir = compile_text(SOURCE);
    for bad in ["", "two words", "tab\t", "é", &"k".repeat(256)] {
        let r = run(
            &ir,
            "make",
            &keyed("make", Some((bad, StoreState::Empty)), 1),
        );
        let a = answer(&r);
        assert_eq!(a.status, 422, "{bad:?}");
        assert_eq!(
            a.data,
            json!({ "issues": [{
                "path": "header.Idempotency-Key",
                "code": "type",
                "message": "must be 1 to 255 visible ASCII characters"
            }] })
        );
    }
    let none = run(&ir, "make", &keyed("make", None, 1));
    assert_eq!(none.outcome, Outcome::Passes);
    assert_eq!(none.store, StoreEffect::Untouched);
}

#[test]
fn a_header_issue_and_a_violation_are_reported_together_and_only_the_violation_is_a_rule() {
    let ir = compile_text(SOURCE);
    let r = run(&ir, "must", &keyed("must", None, 0));
    let a = answer(&r);
    let issues = a.data["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0]["path"], "header.X-Req");
    assert_eq!(issues[1]["code"], "rule");
    assert_eq!(a.violations.len(), 1);
}

fn stored(status: u16, error: Option<&str>, body: Json) -> StoredAnswer {
    StoredAnswer {
        status,
        error: error.map(str::to_owned),
        body: Some(body),
    }
}

#[test]
fn a_stored_answer_is_sent_again_and_no_fact_is_loaded_and_nothing_runs() {
    let ir = compile_text(SOURCE);
    // The fact `seed` is absent and `make` would answer 404; the result would break `ensure`.
    let mut req = keyed(
        "make",
        Some((
            "k",
            StoreState::Stored(stored(410, Some("Gone"), json!({ "why": "x" }))),
        )),
        1,
    );
    req.facts.insert("seed".into(), None);
    req.result = Some(json!({ "n": 500 }));
    let r = run(&ir, "make", &req);
    assert!(r.replayed);
    let a = answer(&r);
    assert_eq!((a.name.as_str(), a.status), ("Gone", 410));
    assert_eq!(a.data, json!({ "why": "x" }));
    assert!(a.violations.is_empty() && a.hook.is_empty());
    assert_eq!(r.store, StoreEffect::Untouched);
    // A stored success: the body is the stored one, and the ETag follows from it.
    let ok = Some((
        "k",
        StoreState::Stored(stored(201, None, json!({ "n": 3 }))),
    ));
    let r = run(&ir, "make", &keyed("make", ok, 1));
    assert!(r.replayed);
    assert_eq!(r.outcome, Outcome::Passes);
    assert_eq!(r.body, Some(json!({ "n": 3 })));
}

#[test]
fn a_stored_answer_is_not_looked_up_when_the_parameters_are_invalid() {
    let ir = compile_text(SOURCE);
    let ok = Some((
        "k",
        StoreState::Stored(stored(201, None, json!({ "n": 3 }))),
    ));
    let r = run(&ir, "make", &keyed("make", ok, 0));
    assert!(!r.replayed);
    assert_eq!(answer(&r).status, 422);
    assert_eq!(r.store, StoreEffect::Untouched);
}

#[test]
fn another_fingerprint_and_a_running_request_are_409_conflict_with_the_message_that_says_which() {
    let ir = compile_text(SOURCE);
    for (state, message) in [
        (
            StoreState::Other,
            "this idempotency key was used with other parameters",
        ),
        (
            StoreState::Running,
            "a request with this idempotency key is still running",
        ),
    ] {
        let mut req = keyed("make", Some(("k", state)), 1);
        req.facts.insert("seed".into(), None);
        let r = run(&ir, "make", &req);
        let a = answer(&r);
        assert_eq!((a.name.as_str(), a.status), ("Conflict", 409));
        assert_eq!(a.data, json!({ "message": message }));
        assert!(a.violations.is_empty());
        assert!(!r.replayed);
        assert_eq!(r.store, StoreEffect::Untouched);
    }
}

// --- what the store holds afterwards -------------------------------------------------------------------

#[test]
fn a_new_key_stores_a_success_a_declared_error_and_a_rule_answer_but_never_a_5xx() {
    let ir = compile_text(SOURCE);
    let new = || Some(("k", StoreState::Empty));
    // A success below 500 is stored with its status and body.
    let mut req = keyed("make", new(), 1);
    req.result = Some(json!({ "n": 7 }));
    let r = run(&ir, "make", &req);
    assert_eq!(
        r.store,
        StoreEffect::Stored(stored(201, None, json!({ "n": 7 })))
    );
    // A fact the loader did not find is `404`, and it is stored.
    let mut req = keyed("make", new(), 1);
    req.facts.insert("seed".into(), None);
    let r = run(&ir, "make", &req);
    assert_eq!(answer(&r).status, 404);
    let StoreEffect::Stored(s) = &r.store else {
        panic!("not stored: {:?}", r.store);
    };
    assert_eq!((s.status, s.error.as_deref()), (404, Some("NotFound")));
    // An `ensure` that fails is `500 Internal`: the entry is removed.
    let mut req = keyed("make", new(), 1);
    req.result = Some(json!({ "n": 500 }));
    let r = run(&ir, "make", &req);
    assert_eq!(answer(&r).status, 500);
    assert_eq!(r.store, StoreEffect::Removed);
    // A run that stops before the handler leaves the entry "running".
    let r = run(&ir, "make", &keyed("make", new(), 1));
    assert_eq!(r.outcome, Outcome::Passes);
    assert_eq!(r.store, StoreEffect::Running);
    // A request without a key touches nothing.
    let mut req = keyed("make", None, 1);
    req.result = Some(json!({ "n": 7 }));
    assert_eq!(run(&ir, "make", &req).store, StoreEffect::Untouched);
}

// --- the scope of a key ----------------------------------------------------------------------------------

#[test]
fn the_caller_is_part_of_the_scope_only_when_the_endpoint_has_auth() {
    let ir = compile_text(SOURCE);
    let ep = |n: &str| {
        ir.services
            .iter()
            .flat_map(|s| s.endpoints.iter())
            .find(|e| e.name == n)
            .unwrap()
    };
    let open = Scope::of(ep("make"), Some("alice"));
    assert_eq!(open.caller, None);
    assert_eq!(open.operation, ep("make").sig);
    let authed = Scope::of(ep("mine"), Some("alice"));
    assert_eq!(authed.caller.as_deref(), Some("alice"));
    assert_ne!(authed, Scope::of(ep("mine"), Some("bob")));
    assert_eq!(Scope::of(ep("mine"), None).caller, None);
    assert_ne!(Scope::of(ep("make"), None).operation, authed.operation);
}

// --- the fingerprint ---------------------------------------------------------------------------------

const FINGERPRINTED: &str = r#"
model Body {
  n: int
  note: string = "none"
  tag?: string
}
service F at "/f" {
  @idempotent
  put "/{id}" make(id: uuid, body: Body, @query dry?: bool) -> Body
}
"#;

fn fingerprint_of(params: Json) -> Option<String> {
    let ir = compile_text(FINGERPRINTED);
    let mut req = Request::default();
    for (k, v) in params.as_object().unwrap() {
        req.params.insert(k.clone(), v.clone());
    }
    req.key = Some(IdempotencyKey {
        value: "k".into(),
        store: StoreState::Empty,
    });
    let p = pipeline(&ir, "F", "make");
    p.run_reported(&req, OutputMode::Strict)
        .unwrap()
        .fingerprint
}

#[test]
fn the_fingerprint_is_taken_after_the_defaults_and_leaves_out_an_optional_that_was_not_sent() {
    // `note` has a default and is added; `tag` and `dry` were not sent and are not there.
    let plain = fingerprint_of(json!({ "id": ID, "body": { "n": 1 } }));
    assert_eq!(plain.as_deref(), Some("bc508ee59e0a79b7"));
    // The same request with the default written out has the same fingerprint.
    let written = fingerprint_of(json!({ "id": ID, "body": { "n": 1, "note": "none" } }));
    assert_eq!(written, plain);
    // An optional parameter that was sent counts, and so does an optional member.
    let sent = fingerprint_of(json!({
        "id": ID,
        "dry": true,
        "body": { "n": 1, "tag": "a" }
    }));
    assert_eq!(sent.as_deref(), Some("62c08a8b20a7323e"));
}

#[test]
fn a_request_without_idempotency_has_no_fingerprint() {
    let ir = compile_text(SOURCE);
    let mut req = Request::default();
    req.params.insert("id".into(), json!(ID));
    req.facts.insert("plain".into(), Some(json!({ "id": ID })));
    let out = pipeline(&ir, "Rooms", "plain")
        .run_reported(&req, OutputMode::Strict)
        .unwrap();
    assert_eq!(out.fingerprint, None);
}

#[test]
fn the_handler_gets_a_version_only_when_a_tag_was_compared() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "Rooms", "touch");
    let mut req = Request::default();
    req.params.insert("id".into(), json!(ID));
    req.if_match = Some("\"3\"".into());
    // The optional fact is absent: nothing was compared, so nothing is handed over.
    req.facts.insert("maybe".into(), None);
    assert_eq!(
        p.run_reported(&req, OutputMode::Strict)
            .unwrap()
            .checked_version,
        None
    );
    // Present and equal: the version that was checked.
    req.facts.insert(
        "maybe".into(),
        Some(json!({ "id": ID, "name": "x", "revision": "3" })),
    );
    let r = p.run_reported(&req, OutputMode::Strict).unwrap();
    assert_eq!((r.outcome, r.checked_version), (Outcome::Passes, Some(3)));
}

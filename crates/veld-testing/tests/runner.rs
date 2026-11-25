//! The runner, offline and live. The live tests talk to a fake test handler on a local socket
//! that records every request and answers from a script.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde_json::{Value, json};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Example, Ir, PackageKind};
use veld_rules::Verdict;
use veld_testing::{HttpRequest, Live, LiveError, b64, build_request, run_offline, select};

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
    let Some(ir) = out.ir.clone() else {
        panic!("does not compile:\n{}", out.render());
    };
    ir
}

fn bookings() -> Ir {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/rules/endpoints/bookings.veld");
    compile_text(&std::fs::read_to_string(path).unwrap())
}

fn example<'a>(ir: &'a Ir, name: &str) -> &'a Example {
    ir.examples
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no example `{name}`"))
}

fn sig_of(ir: &Ir, decl: &str) -> String {
    ir.decls
        .iter()
        .find(|d| d.name == decl)
        .unwrap()
        .sig
        .clone()
}

fn problem(ir: &Ir, decl: &str, status: u16, data: &Value) -> String {
    json!({ "type": sig_of(ir, decl), "title": decl, "status": status, "data": data }).to_string()
}

// --- offline ---------------------------------------------------------------------------

#[test]
fn offline_every_example_of_bookings_passes() {
    let ir = bookings();
    let runs = run_offline(&ir, None);
    assert_eq!(runs.len(), ir.examples.len());
    assert!(runs.len() >= 20);
    for run in &runs {
        let report = run.result.as_ref().unwrap();
        assert!(report.passed(), "{report}");
    }
}

#[test]
fn offline_order_is_stable_and_filter_matches_name_or_endpoint() {
    let ir = bookings();
    let names: Vec<String> = run_offline(&ir, None).into_iter().map(|r| r.name).collect();
    let again: Vec<String> = run_offline(&ir, None).into_iter().map(|r| r.name).collect();
    assert_eq!(names, again);
    let lines: Vec<u32> = select(&ir, None).iter().map(|e| e.origin.line).collect();
    assert!(lines.windows(2).all(|w| w[0] <= w[1]), "{lines:?}");

    let by_name = run_offline(&ir, Some("FULL ROOM"));
    assert_eq!(by_name.len(), 1);
    let by_endpoint = run_offline(&ir, Some("Bookings.payBooking"));
    assert_eq!(by_endpoint.len(), 6);
    assert!(run_offline(&ir, Some("no such example")).is_empty());
}

#[test]
fn offline_reports_an_example_that_cannot_be_filled_in() {
    let ir = compile_text(
        "model Code { value: string @pattern(\"^[A-Z]+$\") }\n\
         service S at \"/s\" { post \"/\" make(body: Code) -> Code }\n\
         example \"needs a value\" { make(body: {}) -> ok }\n",
    );
    let runs = run_offline(&ir, None);
    let err = runs[0].result.as_ref().unwrap_err();
    assert_eq!(err.code(), "E0953");
}

// --- the fake test handler ---------------------------------------------------------------

#[derive(Debug, Clone)]
struct Captured {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
    body: String,
}

impl Captured {
    fn decoded(&self, header: &str) -> Value {
        let text = self.headers.get(&header.to_lowercase()).unwrap();
        serde_json::from_slice(&b64::decode(text).unwrap()).unwrap()
    }
}

struct Reply {
    status: u16,
    headers: Vec<(&'static str, &'static str)>,
    body: String,
}

fn reply(status: u16, body: impl Into<String>) -> Reply {
    Reply {
        status,
        headers: vec![("Content-Type", "application/json")],
        body: body.into(),
    }
}

struct Fake {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<Captured>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Fake {
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The requests other than the probe.
    fn requests(&self) -> Vec<Captured> {
        let seen = self.seen.lock().unwrap();
        seen.iter()
            .filter(|c| c.target != "/__veld/test")
            .cloned()
            .collect()
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Option<Captured> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.lines();
    let mut first = lines.next()?.split(' ');
    let (method, target) = (first.next()?.to_owned(), first.next()?.to_owned());
    let headers: BTreeMap<String, String> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_owned()))
        .collect();
    let len: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    while buf.len() < head_end + len {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[head_end..]).into_owned();
    Some(Captured {
        method,
        target,
        headers,
        body,
    })
}

fn write_reply(stream: &mut TcpStream, r: &Reply) {
    let mut out = format!("HTTP/1.1 {} X\r\n", r.status);
    for (k, v) in &r.headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        r.body.len()
    ));
    out.push_str(&r.body);
    let _ = stream.write_all(out.as_bytes());
    let _ = stream.flush();
}

/// A test handler: the probe answers `{}`, everything else comes from `script`.
fn serve(script: impl Fn(&Captured) -> Reply + Send + 'static) -> Fake {
    serve_raw(move |c| {
        if c.target == "/__veld/test" {
            reply(200, "{\"services\":{}}")
        } else {
            script(c)
        }
    })
}

fn serve_raw(script: impl Fn(&Captured) -> Reply + Send + 'static) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let (seen2, stop2) = (seen.clone(), stop.clone());
    let thread = std::thread::spawn(move || {
        for stream in listener.incoming() {
            if stop2.load(Ordering::SeqCst) {
                return;
            }
            let Ok(mut stream) = stream else { continue };
            if let Some(c) = read_request(&mut stream) {
                // Record the request before answering: the client may read the answer and the
                // test may look at the requests before this thread runs again.
                let answer = script(&c);
                seen2.lock().unwrap().push(c);
                write_reply(&mut stream, &answer);
            }
        }
    });
    Fake {
        addr,
        seen,
        stop,
        thread: Some(thread),
    }
}

fn run_live(fake: &Fake, ir: &Ir, name: &str, dry_run: bool) -> veld_rules::ExampleReport {
    let live = Live::connect(&fake.url(), dry_run).unwrap();
    live.run_example(ir, example(ir, name)).unwrap()
}

// --- the request ---------------------------------------------------------------------------

#[test]
fn the_request_of_create_booking() {
    let ir = bookings();
    let fake = serve(|_| reply(200, "{}"));
    run_live(
        &fake,
        &ir,
        "a full room is rejected with its capacity",
        false,
    );
    let sent = fake.requests();
    assert_eq!(sent.len(), 1);
    let c = &sent[0];
    assert_eq!(
        (c.method.as_str(), c.target.as_str()),
        ("POST", "/bookings")
    );
    assert_eq!(c.headers["content-type"], "application/json");
    let facts = c.decoded("Veld-Test-Facts");
    assert_eq!(facts["room"]["capacity"], 2);
    assert_eq!(facts["room"]["closed"], false);
    assert_eq!(
        facts["coupon"],
        Value::Null,
        "an absent optional fact is null"
    );
    let body: Value = serde_json::from_str(&c.body).unwrap();
    assert_eq!(body["guests"], 3);
    assert_eq!(body["roomId"], facts["room"]["id"]);
    assert!(body.get("couponCode").is_none());
    assert_eq!(c.headers["veld-test-now"], "2026-01-01T00:00:00Z");
    for absent in [
        "veld-test-principal",
        "veld-test-result",
        "veld-test-dry-run",
    ] {
        assert!(!c.headers.contains_key(absent), "{absent}");
    }
}

#[test]
fn the_request_of_pay_booking_binds_the_path_and_sends_the_clock() {
    let ir = bookings();
    let fake = serve(|_| reply(200, "{}"));
    run_live(
        &fake,
        &ir,
        "paying the wrong amount is refused with the expected one",
        false,
    );
    let c = &fake.requests()[0];
    let facts = c.decoded("veld-test-facts");
    let id = facts["booking"]["id"].as_str().unwrap();
    assert_eq!(c.method, "POST");
    assert_eq!(c.target, format!("/bookings/{id}/pay"));
    let body: Value = serde_json::from_str(&c.body).unwrap();
    assert_eq!(body, json!({ "amount": "100.00" }), "{body}");

    run_live(&fake, &ir, "a booking may start exactly now", false);
    let c = &fake.requests()[1];
    assert_eq!(c.headers["veld-test-now"], "2026-06-01T00:00:00Z");
}

#[test]
fn returns_sends_the_result_and_dry_run_is_for_examples_without_it() {
    let ir = bookings();
    let fake = serve(|_| reply(200, "{}"));
    run_live(&fake, &ir, "the handler must keep the room", true);
    run_live(&fake, &ir, "a valid booking passes every check", true);
    run_live(&fake, &ir, "a valid booking passes every check", false);
    let sent = fake.requests();
    let result = sent[0].decoded("Veld-Test-Result");
    assert_eq!(result["guests"], 2);
    assert_eq!(result["status"], "pending");
    assert!(!sent[0].headers.contains_key("veld-test-dry-run"));
    assert_eq!(sent[1].headers["veld-test-dry-run"], "1");
    assert!(!sent[1].headers.contains_key("veld-test-result"));
    assert!(!sent[2].headers.contains_key("veld-test-dry-run"));
}

#[test]
fn query_header_and_body_parameters_are_bound() {
    let ir = compile_text(
        "model Filter { tags: string[]\n limit?: int }\n\
         model Item { name: string }\n\
         service S at \"/s\" {\n\
           get \"/{id}/items\" list(id: string, query: Filter, @header(\"X-Trace\") trace: string, @query page: int) -> Item\n\
         }\n\
         example \"binds\" { list(id: \"a b\", query: { tags: [\"x\", \"y\"], limit: 5 }, trace: \"t1\", page: 2) -> ok }\n",
    );
    let filled = veld_rules::fill_example(&ir, &ir.examples[0]).unwrap();
    let req: HttpRequest = build_request(&ir, &filled, false).unwrap();
    assert_eq!(req.method, "GET");
    assert_eq!(req.target, "/s/a%20b/items?limit=5&page=2&tags=x&tags=y");
    assert_eq!(req.header("x-trace"), Some("t1"));
    assert!(req.body.is_none());
}

// --- comparing answers ---------------------------------------------------------------------

/// The id of the `require` of `endpoint` that answers with the error `error`.
fn require_id(ir: &Ir, endpoint: &str, error: &str) -> String {
    let decl = ir.decls.iter().find(|d| d.name == error).unwrap().id;
    ir.services
        .iter()
        .flat_map(|s| &s.endpoints)
        .find(|e| e.name == endpoint)
        .unwrap()
        .rules
        .requires
        .iter()
        .find(|r| r.else_error.as_ref().is_some_and(|e| e.decl == decl))
        .unwrap()
        .id
        .clone()
}

/// The whole `RoomFull` answer of section 9.3: the payload and the violation of the `require`.
fn room_full(ir: &Ir, violation: &Value) -> String {
    let mut doc: Value =
        serde_json::from_str(&problem(ir, "RoomFull", 409, &json!({ "capacity": 2 }))).unwrap();
    doc["violations"] = json!([violation]);
    doc.to_string()
}

fn room_full_violation(ir: &Ir) -> Value {
    json!({
        "rule": require_id(ir, "createBooking", "RoomFull"),
        "path": "request",
        "fields": ["body"],
        "message": "precondition failed",
    })
}

const FULL_ROOM: &str = "a full room is rejected with its capacity";

#[test]
fn a_matching_error_passes() {
    let ir = bookings();
    let body = room_full(&ir, &room_full_violation(&ir));
    let fake = serve(move |_| reply(409, body.clone()));
    let report = run_live(&fake, &ir, FULL_ROOM, false);
    assert!(report.passed(), "{report}");
    assert!(report.outcome.is_some());
}

// An example writes down the error and its payload. The violations are not in it, so the answer
// is also held to what the reference evaluator gives for the same request.

#[test]
fn an_error_without_its_violations_fails_against_the_reference() {
    let ir = bookings();
    let body = problem(&ir, "RoomFull", 409, &json!({ "capacity": 2 }));
    let fake = serve(move |_| reply(409, body.clone()));
    let report = run_live(&fake, &ir, FULL_ROOM, false);
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert_eq!(m.differences.len(), 1, "{m}");
    assert_eq!(m.differences[0].path, "violations");
    assert!(m.expected.contains("reference evaluator"), "{m}");
    assert!(m.expected.contains("RoomFull 409"), "{m}");
}

#[test]
fn a_violation_that_differs_in_any_member_fails_against_the_reference() {
    let ir = bookings();
    let changes: [(&str, Value); 5] = [
        (
            "rule",
            json!("veld:t/Bookings.createBooking.require@0000000000000000"),
        ),
        ("path", json!("body")),
        ("fields", json!(["body", "id"])),
        ("message", json!("the room is full")),
        ("error", json!("overflow")),
    ];
    for (member, value) in changes {
        let mut violation = room_full_violation(&ir);
        violation[member] = value;
        let body = room_full(&ir, &violation);
        let fake = serve(move |_| reply(409, body.clone()));
        let report = run_live(&fake, &ir, FULL_ROOM, false);
        let Verdict::Failed(m) = &report.verdict else {
            panic!("a changed `{member}` passed: {report}")
        };
        assert_eq!(m.differences[0].path, "violations", "{member}: {m}");
    }
}

#[test]
fn an_extra_violation_fails_against_the_reference() {
    let ir = bookings();
    let mut doc: Value = serde_json::from_str(&room_full(&ir, &room_full_violation(&ir))).unwrap();
    let again = doc["violations"][0].clone();
    doc["violations"].as_array_mut().unwrap().push(again);
    let body = doc.to_string();
    let fake = serve(move |_| reply(409, body.clone()));
    let report = run_live(&fake, &ir, FULL_ROOM, false);
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");
}

#[test]
fn a_wrong_error_type_fails_and_says_so() {
    let ir = bookings();
    let body = problem(&ir, "RoomClosed", 409, &json!({}));
    let fake = serve(move |_| reply(409, body.clone()));
    let report = run_live(
        &fake,
        &ir,
        "a full room is rejected with its capacity",
        false,
    );
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert!(m.differences.iter().any(|d| d.path == "$.type"), "{m}");
    assert!(m.expected.starts_with("RoomFull"), "{m}");
    assert!(m.actual.starts_with("RoomClosed"), "{m}");
}

#[test]
fn a_data_mismatch_fails_with_its_path() {
    let ir = bookings();
    let body = problem(&ir, "RoomFull", 409, &json!({ "capacity": 3 }));
    let fake = serve(move |_| reply(409, body.clone()));
    let report = run_live(
        &fake,
        &ir,
        "a full room is rejected with its capacity",
        false,
    );
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert_eq!(m.differences.len(), 1, "{m}");
    assert_eq!(m.differences[0].path, "$.capacity");
}

#[test]
fn a_wrong_status_fails() {
    let ir = bookings();
    let body = problem(&ir, "RoomFull", 500, &json!({ "capacity": 2 }));
    let fake = serve(move |_| reply(500, body.clone()));
    let report = run_live(
        &fake,
        &ir,
        "a full room is rejected with its capacity",
        false,
    );
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert!(m.differences.iter().any(|d| d.path == "status"), "{m}");
}

#[test]
fn an_answer_that_is_not_a_problem_document_fails() {
    let ir = bookings();
    let fake = serve(|_| reply(502, "<html>bad gateway</html>"));
    let report = run_live(
        &fake,
        &ir,
        "a full room is rejected with its capacity",
        false,
    );
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert!(m.actual.starts_with("HTTP 502"), "{m}");
    assert!(report.outcome.is_none());
}

#[test]
fn a_dry_run_passes_on_204_with_the_header() {
    let ir = bookings();
    let name = "a valid booking passes every check";
    let fake = serve(|_| Reply {
        status: 204,
        headers: vec![("Veld-Test-Passed", "1")],
        body: String::new(),
    });
    assert!(run_live(&fake, &ir, name, true).passed());

    let fake = serve(|_| reply(204, ""));
    let report = run_live(&fake, &ir, name, true);
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");

    let body = problem(&ir, "RoomFull", 409, &json!({ "capacity": 2 }));
    let fake = serve(move |_| reply(409, body.clone()));
    let report = run_live(&fake, &ir, name, true);
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");
}

#[test]
fn a_success_body_is_matched_against_the_pattern() {
    let ir = compile_text(
        "model Greeting { message: string\n n: int }\n\
         service S at \"/s\" { get \"/hi\" greet() -> Greeting }\n\
         example \"says hi\" { greet() -> ok { message: \"hi\" } }\n",
    );
    let fake = serve(|_| reply(200, "{\"message\":\"hi\",\"n\":1}"));
    assert!(run_live(&fake, &ir, "says hi", false).passed());

    let fake = serve(|_| reply(200, "{\"message\":\"yo\",\"n\":1}"));
    let report = run_live(&fake, &ir, "says hi", false);
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert_eq!(m.differences[0].path, "$.message");

    let fake = serve(|_| reply(200, "{}"));
    let report = run_live(&fake, &ir, "says hi", true);
    assert!(matches!(report.verdict, Verdict::Skipped(_)), "{report}");
}

// --- the probe and the connection ----------------------------------------------------------

#[test]
fn a_server_that_is_not_a_test_handler_is_refused_before_any_example() {
    let fake = serve_raw(|_| reply(404, "{\"error\":\"not found\"}"));
    let err = Live::connect(&fake.url(), false).unwrap_err();
    assert!(matches!(err, LiveError::NotTestHandler { .. }), "{err}");
    assert!(err.to_string().contains("404"), "{err}");
    assert!(fake.requests().is_empty());

    let fake = serve_raw(|_| reply(200, "<html></html>"));
    let err = Live::connect(&fake.url(), false).unwrap_err();
    assert!(matches!(err, LiveError::NotTestHandler { .. }), "{err}");
}

#[test]
fn a_failed_connection_names_the_url() {
    let addr = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    };
    let url = format!("http://{addr}");
    let err = Live::connect(&url, false).unwrap_err();
    assert!(matches!(err, LiveError::Connect { .. }), "{err}");
    assert!(err.to_string().contains(&addr.to_string()), "{err}");
}

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

/// The operation signature of `createBooking`, the endpoint the error examples call.
fn create_booking(ir: &Ir) -> String {
    ir.services
        .iter()
        .flat_map(|s| &s.endpoints)
        .find(|e| e.name == "createBooking")
        .unwrap()
        .sig
        .clone()
}

/// The problem document of an error of `createBooking` (`spec/ir.md` section 5.5).
fn problem(ir: &Ir, decl: &str, status: u16, data: &Value) -> String {
    json!({
        "type": sig_of(ir, decl),
        "title": decl,
        "status": status,
        "operation": create_booking(ir),
        "data": data,
    })
    .to_string()
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
    headers: Vec<(&'static str, String)>,
    body: String,
}

fn reply(status: u16, body: impl Into<String>) -> Reply {
    Reply {
        status,
        headers: vec![("Content-Type", "application/json".to_owned())],
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

/// What a test handler of the services of these tests answers to the probe (section 12.4).
const PROBE: &str = "{\"veldTestHandler\":1,\"digests\":{\"Bookings\":null,\"S\":null}}";

/// A test handler: the probe names its services, everything else comes from `script`.
fn serve(script: impl Fn(&Captured) -> Reply + Send + 'static) -> Fake {
    serve_raw(move |c| {
        if c.target == "/__veld/test" {
            reply(200, PROBE)
        } else {
            script(c)
        }
    })
}

/// The headers a generated server puts on an error: the problem media type and the rules digest
/// of the service.
fn problem_headers(ir: &Ir) -> Vec<(&'static str, String)> {
    let mut headers = vec![("Content-Type", "application/problem+json".to_owned())];
    if let Some(digest) = ir.services.iter().find_map(|s| s.rules_digest.clone()) {
        headers.push(("Veld-Rules", digest));
    }
    headers
}

/// A test handler that answers every request with this error, as a generated server would.
fn serve_problem(ir: &Ir, status: u16, body: String) -> Fake {
    let headers = problem_headers(ir);
    serve(move |_| Reply {
        status,
        headers: headers.clone(),
        body: body.clone(),
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
    let fake = serve_problem(&ir, 409, body.clone());
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
    let fake = serve_problem(&ir, 409, body.clone());
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
        let fake = serve_problem(&ir, 409, body.clone());
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
    let fake = serve_problem(&ir, 409, body.clone());
    let report = run_live(&fake, &ir, FULL_ROOM, false);
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");
}

#[test]
fn a_wrong_error_type_fails_and_says_so() {
    let ir = bookings();
    let body = problem(&ir, "RoomClosed", 409, &json!({}));
    let fake = serve_problem(&ir, 409, body.clone());
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
    let fake = serve_problem(&ir, 409, body.clone());
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
    let fake = serve_problem(&ir, 500, body.clone());
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
        headers: vec![("Veld-Test-Passed", "1".to_owned())],
        body: String::new(),
    });
    assert!(run_live(&fake, &ir, name, true).passed());

    let fake = serve(|_| reply(204, ""));
    let report = run_live(&fake, &ir, name, true);
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");

    let body = problem(&ir, "RoomFull", 409, &json!({ "capacity": 2 }));
    let fake = serve_problem(&ir, 409, body.clone());
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

/// Any JSON object used to pass the probe. A production server that answers `{}` on an unknown
/// path must not be sent test headers.
#[test]
fn a_probe_answer_must_say_it_is_a_test_handler_and_name_its_services() {
    let refused = [
        "{}",
        "{\"services\":{}}",
        "{\"veldTestHandler\":0,\"digests\":{}}",
        "{\"veldTestHandler\":1}",
        "{\"veldTestHandler\":1,\"digests\":[]}",
        "[1]",
    ];
    for body in refused {
        let fake = serve_raw(move |_| reply(200, body));
        let err = Live::connect(&fake.url(), false).unwrap_err();
        assert!(
            matches!(err, LiveError::NotTestHandler { .. }),
            "{body}: {err}"
        );
        assert!(fake.requests().is_empty(), "{body}");
    }
    let fake = serve_raw(|_| reply(200, PROBE));
    let live = Live::connect(&fake.url(), false).unwrap();
    assert!(live.serves("Bookings") && live.serves("S"));
    assert!(!live.serves("Payments"));
}

/// A workspace has one test handler per service: an example of another service is not sent.
#[test]
fn an_example_of_a_service_the_handler_does_not_serve_is_skipped() {
    let ir = bookings();
    let fake = serve_raw(|c| {
        if c.target == "/__veld/test" {
            reply(
                200,
                "{\"veldTestHandler\":1,\"digests\":{\"Payments\":null}}",
            )
        } else {
            reply(500, "must not be asked")
        }
    });
    let report = run_live(&fake, &ir, FULL_ROOM, false);
    let Verdict::Skipped(why) = &report.verdict else {
        panic!("{report}")
    };
    assert!(why.contains("does not serve `Bookings`"), "{why}");
    assert!(fake.requests().is_empty());
}

// The members of the problem document that an example never states are held to the contract too.

/// The full, correct `RoomFull` answer with one member of the document changed.
fn room_full_with(ir: &Ir, member: &str, value: Value) -> String {
    let mut doc: Value = serde_json::from_str(&room_full(ir, &room_full_violation(ir))).unwrap();
    doc[member] = value;
    doc.to_string()
}

#[test]
fn a_wrong_title_or_operation_fails_against_the_reference() {
    let ir = bookings();
    for (member, value) in [
        ("title", json!("WRONG")),
        ("operation", json!("veld:wrong/Op@0000000000000000")),
    ] {
        let fake = serve_problem(&ir, 409, room_full_with(&ir, member, value));
        let report = run_live(&fake, &ir, FULL_ROOM, false);
        let Verdict::Failed(m) = &report.verdict else {
            panic!("a wrong `{member}` passed: {report}")
        };
        let paths: Vec<&str> = m.differences.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, [format!("$.{member}")], "{m}");
    }
}

#[test]
fn an_error_that_is_not_problem_json_or_lacks_the_rules_digest_fails_against_the_reference() {
    let ir = bookings();
    let body = room_full(&ir, &room_full_violation(&ir));
    let good = problem_headers(&ir);
    assert_eq!(good.len(), 2, "the bookings service has a rules digest");
    let cases: [(&str, Vec<(&'static str, String)>); 3] = [
        (
            "header Content-Type",
            vec![
                ("Content-Type", "application/json".to_owned()),
                good[1].clone(),
            ],
        ),
        ("header Veld-Rules", vec![good[0].clone()]),
        (
            "header Veld-Rules",
            vec![
                good[0].clone(),
                ("Veld-Rules", "0000000000000000".to_owned()),
            ],
        ),
    ];
    for (path, headers) in cases {
        let body = body.clone();
        let fake = serve(move |_| Reply {
            status: 409,
            headers: headers.clone(),
            body: body.clone(),
        });
        let report = run_live(&fake, &ir, FULL_ROOM, false);
        let Verdict::Failed(m) = &report.verdict else {
            panic!("{path}: {report}")
        };
        let paths: Vec<&str> = m.differences.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, [path], "{m}");
    }
}

// --- audited rules (`@rollout(audit)`, spec/rules.md sections 12.4 and 17.1) ---------------------

const AUDITED_REQUEST: &str = "an audited invariant that is violated does not reject the request";
const AUDITED_ENFORCED: &str = "the same invariant enforced rejects it";
const AUDITED_HOLDS: &str = "an audited rule that holds changes nothing";

fn rollout() -> Ir {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/rules/endpoints/rollout.veld");
    compile_text(&std::fs::read_to_string(path).unwrap())
}

/// A test handler of the rollout contract that answers a dry run (`204` and `Veld-Test-Passed: 1`)
/// with these headers. The examples below are run as dry runs, so the reference reports the rules
/// of steps 5 and 6 only and the server's list must equal it.
fn serve_parties(mut headers: Vec<(&'static str, String)>) -> Fake {
    headers.push(("Veld-Test-Passed", "1".to_owned()));
    serve_raw(move |c| {
        if c.target == "/__veld/test" {
            reply(
                200,
                "{\"veldTestHandler\":1,\"digests\":{\"Parties\":null}}",
            )
        } else {
            Reply {
                status: 204,
                headers: headers.clone(),
                body: String::new(),
            }
        }
    })
}

/// The header a test handler sends for these reports (the unpadded base64url of a JSON array).
fn audited_header(reports: &[Value]) -> (&'static str, String) {
    let text = serde_json::to_string(reports).unwrap();
    ("Veld-Test-Audited", b64::encode(text.as_bytes()))
}

/// What the reference evaluator reports for the example, in the wire form of a header entry.
fn reference_reports(ir: &Ir, name: &str) -> Vec<Value> {
    let fake = serve_parties(vec![]);
    let report = run_live(&fake, ir, name, true);
    report
        .audited
        .iter()
        .map(|v| serde_json::to_value(v).unwrap())
        .collect()
}

#[test]
fn a_live_run_lists_the_audited_rules_the_reference_reports() {
    let ir = rollout();
    let reports = reference_reports(&ir, AUDITED_REQUEST);
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert_eq!(reports[0]["path"], "body");
}

#[test]
fn a_server_that_never_reports_an_audited_rule_fails_the_example() {
    let ir = rollout();
    let fake = serve_parties(vec![]);
    let report = run_live(&fake, &ir, AUDITED_REQUEST, true);
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert_eq!(m.differences.len(), 1, "{m}");
    assert_eq!(m.differences[0].path, "header Veld-Test-Audited");
    assert_eq!(m.differences[0].actual, "not reported");
    assert!(!report.audited.is_empty());
}

#[test]
fn a_server_that_reports_the_audited_rules_passes() {
    let ir = rollout();
    let reports = reference_reports(&ir, AUDITED_REQUEST);
    let fake = serve_parties(vec![audited_header(&reports)]);
    let report = run_live(&fake, &ir, AUDITED_REQUEST, true);
    assert!(report.passed(), "{report}");
    assert_eq!(report.audited.len(), 1);
}

#[test]
fn an_audited_report_the_reference_does_not_make_fails_the_example() {
    let ir = rollout();
    let reports = reference_reports(&ir, AUDITED_REQUEST);
    let invented = json!({ "rule": "veld:t/Party.invented@0000", "path": "body", "fields": [], "message": "m" });
    let fake = serve_parties(vec![audited_header(&[reports[0].clone(), invented])]);
    let report = run_live(&fake, &ir, AUDITED_REQUEST, true);
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert_eq!(m.differences.len(), 1, "{m}");
    assert_eq!(m.differences[0].expected, "not reported");
}

#[test]
fn a_report_at_another_path_or_with_another_error_fails_the_example() {
    let ir = rollout();
    let reports = reference_reports(&ir, AUDITED_REQUEST);
    for (member, value) in [("path", json!("body.guests")), ("error", json!("overflow"))] {
        let mut wrong = reports[0].clone();
        wrong[member] = value;
        let fake = serve_parties(vec![audited_header(&[wrong])]);
        let report = run_live(&fake, &ir, AUDITED_REQUEST, true);
        assert!(
            matches!(report.verdict, Verdict::Failed(_)),
            "{member}: {report}"
        );
    }
}

#[test]
fn an_unreadable_audited_header_fails_the_example() {
    let ir = rollout();
    let fake = serve_parties(vec![("Veld-Test-Audited", "not base64!".to_owned())]);
    let report = run_live(&fake, &ir, AUDITED_REQUEST, true);
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert!(m.actual.contains("cannot be read"), "{m}");
}

#[test]
fn an_example_that_audits_nothing_expects_no_header() {
    let ir = rollout();
    assert!(reference_reports(&ir, AUDITED_HOLDS).is_empty());
    let fake = serve_parties(vec![]);
    let report = run_live(&fake, &ir, AUDITED_HOLDS, true);
    assert!(report.passed(), "{report}");
    let reports = reference_reports(&ir, AUDITED_REQUEST);
    let fake = serve_parties(vec![audited_header(&reports)]);
    let report = run_live(&fake, &ir, AUDITED_HOLDS, true);
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");
}

/// A test handler of the rollout contract that answers `201` with `body` and these headers.
fn serve_parties_answering(body: &str, headers: Vec<(&'static str, String)>) -> Fake {
    let body = body.to_owned();
    serve_raw(move |c| {
        if c.target == "/__veld/test" {
            reply(
                200,
                "{\"veldTestHandler\":1,\"digests\":{\"Parties\":null}}",
            )
        } else {
            Reply {
                status: 201,
                headers: headers.clone(),
                body: body.clone(),
            }
        }
    })
}

/// An example without `returns` that is not a dry run runs the application's real handler: its
/// success answer is the result steps 8 and 9 saw, so the audited rules of that result are part of
/// what the server must report.
#[test]
fn the_answer_of_the_real_handler_is_the_result_the_reference_judges() {
    let ir = rollout();
    // The handler answers guests 3 to a request for 7: the audited `ensure` is violated.
    let answer = r#"{"id":"00000000-0000-4000-8000-000000000009","guests":3}"#;
    let bare = serve_parties_answering(answer, vec![]);
    let report = run_live(&bare, &ir, AUDITED_REQUEST, false);
    assert_eq!(report.audited.len(), 2, "{:?}", report.audited);
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");
    // Only the audited invariant of the body: the ensure of the answer is missing.
    let reports = reference_reports(&ir, AUDITED_REQUEST);
    let partial = serve_parties_answering(answer, vec![audited_header(&reports)]);
    let report = run_live(&partial, &ir, AUDITED_REQUEST, false);
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert!(m.differences[0].expected.contains(".ensure@"), "{m}");
    // Everything the reference reports for that answer.
    let full: Vec<Value> = report
        .audited
        .iter()
        .map(|v| serde_json::to_value(v).unwrap())
        .collect();
    let right = serve_parties_answering(answer, vec![audited_header(&full)]);
    let report = run_live(&right, &ir, AUDITED_REQUEST, false);
    assert!(report.passed(), "{report}");
}

#[test]
fn an_error_answer_of_the_real_handler_only_has_to_report_what_the_reference_knows() {
    let ir = rollout();
    let reports = reference_reports(&ir, AUDITED_REQUEST);
    let extra = json!({ "rule": "veld:t/Booking.sane@0000", "path": "result", "fields": [], "message": "m" });
    let mut more = reports.clone();
    more.push(extra);
    // A `500` answers no result: the server may have reported more than steps 5 and 6.
    let fake = serve_raw(move |c| {
        if c.target == "/__veld/test" {
            reply(
                200,
                "{\"veldTestHandler\":1,\"digests\":{\"Parties\":null}}",
            )
        } else {
            Reply {
                status: 500,
                headers: vec![audited_header(&more)],
                body: "{}".to_owned(),
            }
        }
    });
    let report = run_live(&fake, &ir, AUDITED_REQUEST, false);
    // The status is wrong for the example, but the audited list is not what fails it.
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert!(
        m.differences
            .iter()
            .all(|d| d.path != "header Veld-Test-Audited"),
        "{m}"
    );
}

#[test]
fn an_enforced_rule_is_not_an_audited_report() {
    let ir = rollout();
    assert!(reference_reports(&ir, AUDITED_ENFORCED).is_empty());
}

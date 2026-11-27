//! `expect emitted` and `expect not emitted` against a fake test handler (`spec/events.md`
//! section 11.1): the example is compared with the `Veld-Test-Emitted` header of the answer. The
//! fake is a socket that records every request and answers from a script.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Example, Ir, PackageKind};
use veld_rules::Verdict;
use veld_testing::Live;
use veld_testing::emitted::EMITTED;

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

fn outbox() -> Ir {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/events/outbox.veld.pending");
    compile_text(&std::fs::read_to_string(path).unwrap().replace("\r\n", "\n"))
}

fn example<'a>(ir: &'a Ir, name: &str) -> &'a Example {
    ir.examples
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no example `{name}`"))
}

// --- the fake test handler ---------------------------------------------------------------

#[derive(Debug, Clone)]
struct Captured {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
    body: String,
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

const PROBE: &str =
    "{\"veldTestHandler\":1,\"digests\":{\"Bookings\":null,\"Notifications\":null}}";

/// A test handler: the probe names its services, every other request is a success of `status`
/// that carries `emitted` as the header `Veld-Test-Emitted` when it is not empty.
fn serve(status: u16, emitted: &'static str) -> Fake {
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
                let mut answer = if c.target == "/__veld/test" {
                    reply(200, PROBE)
                } else {
                    reply(status, "{}")
                };
                if c.target != "/__veld/test" && !emitted.is_empty() {
                    answer.headers.push((EMITTED, emitted.to_owned()));
                }
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

/// The status a success of the endpoint `name` of `Bookings` has.
fn success_status(ir: &Ir, name: &str) -> u16 {
    let bookings = ir.services.iter().find(|s| s.name == "Bookings").unwrap();
    bookings
        .endpoints
        .iter()
        .find(|e| e.name == name)
        .unwrap()
        .success
        .status
}

fn run(fake: &Fake, ir: &Ir, example: &Example) -> veld_rules::ExampleReport {
    let live = Live::connect(&fake.url(), false).unwrap();
    live.run_example(ir, example).unwrap()
}

// --- the tests -----------------------------------------------------------------------------

#[test]
fn a_handler_that_wrote_the_event_of_the_example_passes() {
    let ir = outbox();
    let ex = example(&ir, "paying a pending booking writes BookingPaid");
    let fake = serve(success_status(&ir, "pay"), "BookingPaid");
    let report = run(&fake, &ir, ex);
    assert!(report.passed(), "{report}");
    let sent = fake.requests();
    assert_eq!(sent.len(), 1);
    // The request is the one of any call example: the test headers carry the facts and the result.
    assert_eq!(sent[0].method, "POST");
    assert!(sent[0].headers.contains_key("veld-test-facts"));
    assert!(sent[0].headers.contains_key("veld-test-result"));
    assert!(sent[0].body.is_empty() || sent[0].body.starts_with('{'));
}

#[test]
fn a_handler_that_wrote_nothing_fails_the_example() {
    let ir = outbox();
    let ex = example(&ir, "paying a pending booking writes BookingPaid");
    let fake = serve(success_status(&ir, "pay"), "");
    let report = run(&fake, &ir, ex);
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert!(
        m.differences.iter().any(|d| d.path == "event BookingPaid"),
        "{m}"
    );
}

#[test]
fn another_event_than_the_one_expected_fails_the_example() {
    let ir = outbox();
    let ex = example(
        &ir,
        "cancelling a paid booking writes RefundDue, and nothing else",
    );
    let fake = serve(success_status(&ir, "cancel"), "BookingCancelled");
    let report = run(&fake, &ir, ex);
    let Verdict::Failed(m) = &report.verdict else {
        panic!("{report}")
    };
    assert_eq!(m.differences.len(), 2, "{m}");
    // `nothing else`: the right event next to a stranger is also a failure.
    let both = serve(success_status(&ir, "cancel"), "RefundDue,BookingCancelled");
    assert!(!run(&both, &ir, ex).passed());
    let right = serve(success_status(&ir, "cancel"), "RefundDue");
    assert!(run(&right, &ir, ex).passed());
}

#[test]
fn not_emitted_wants_the_header_absent() {
    let mut ir = outbox();
    let at = ir
        .examples
        .iter()
        .position(|e| e.name == "paying works")
        .unwrap();
    ir.examples[at].emitted = Some(Vec::new());
    let ex = &ir.examples[at];
    let quiet = serve(success_status(&ir, "pay"), "");
    assert!(run(&quiet, &ir, ex).passed());
    let loud = serve(success_status(&ir, "pay"), "BookingPaid");
    let report = run(&loud, &ir, ex);
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");
}

#[test]
fn an_example_without_emitted_lines_ignores_the_header() {
    let ir = outbox();
    let ex = example(&ir, "paying works");
    let fake = serve(success_status(&ir, "pay"), "BookingPaid");
    assert!(run(&fake, &ir, ex).passed());
}

#[test]
fn a_wrong_status_is_still_reported_before_the_events() {
    let ir = outbox();
    let ex = example(&ir, "paying a pending booking writes BookingPaid");
    let fake = serve(500, "BookingPaid");
    let report = run(&fake, &ir, ex);
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");
}

//! Live event examples (`spec/events.md` section 10.9) against a fake test handler on a local
//! socket. The fake is a small reference handler: it decodes the message with the reference
//! consumer (`veld_rules::envelope`), gives every subscription a memory inbox and answers with
//! the JSON of section 10.9. Options make it behave as a faulty generated handler would.
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
use veld_rules::envelope::{ConsumeError, MemoryInbox, decode};
use veld_testing::{DELIVERIES, EVENT_PATH, KEY, Live, TOPIC};

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

fn delivery() -> Ir {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/events/delivery.veld");
    compile_text(&std::fs::read_to_string(path).unwrap().replace("\r\n", "\n"))
}

fn example<'a>(ir: &'a Ir, name: &str) -> &'a Example {
    ir.examples
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no example `{name}`"))
}

// --- the fake test handler -------------------------------------------------------------------

/// How the fake behaves.
#[derive(Clone)]
struct Opts {
    /// The services it serves, and so the ones in its probe.
    services: Vec<&'static str>,
    /// Whether the probe has `subscribes`.
    list_subscribes: bool,
    /// Without an inbox a second delivery runs the handler again.
    inbox: bool,
    /// A generated consumer checks constraints; this one rejects what breaks one as `shape`.
    constraints: bool,
    /// Answers this status instead of the JSON.
    status: u16,
    /// Rewrites the answer before it is sent.
    tamper: Option<fn(&mut Value)>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            services: vec!["Fulfilment", "Billing"],
            list_subscribes: true,
            inbox: true,
            constraints: true,
            status: 200,
            tamper: None,
        }
    }
}

#[derive(Clone, Debug)]
struct Captured {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
    body: String,
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

    fn events(&self) -> Vec<Captured> {
        let seen = self.seen.lock().unwrap();
        seen.iter()
            .filter(|c| c.target == EVENT_PATH)
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
    Some(Captured {
        method,
        target,
        headers,
        body: String::from_utf8_lossy(&buf[head_end..]).into_owned(),
    })
}

fn write_reply(stream: &mut TcpStream, status: u16, body: &str) {
    let out = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(out.as_bytes());
    let _ = stream.flush();
}

fn serve(ir: Ir, opts: Opts) -> Fake {
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
            let Some(c) = read_request(&mut stream) else {
                continue;
            };
            let (status, body) = if c.target == "/__veld/test" {
                (200, probe(&ir, &opts).to_string())
            } else {
                let mut answer = respond(&ir, &opts, &c);
                if let Some(t) = opts.tamper {
                    t(&mut answer);
                }
                (opts.status, answer.to_string())
            };
            seen2.lock().unwrap().push(c);
            write_reply(&mut stream, status, &body);
        }
    });
    Fake {
        addr,
        seen,
        stop,
        thread: Some(thread),
    }
}

fn probe(ir: &Ir, opts: &Opts) -> Value {
    let digests: serde_json::Map<String, Value> = opts
        .services
        .iter()
        .map(|s| ((*s).to_owned(), Value::Null))
        .collect();
    let mut body = json!({ "veldTestHandler": 1, "digests": digests });
    if opts.list_subscribes {
        let subscribes: serde_json::Map<String, Value> = ir
            .services
            .iter()
            .filter(|s| opts.services.contains(&s.name.as_str()) && !s.subscriptions.is_empty())
            .map(|s| {
                let topics: Vec<&str> = s
                    .subscriptions
                    .iter()
                    .map(|sub| ir.channel(sub.channel).topic.as_str())
                    .collect();
                (s.name.clone(), json!(topics))
            })
            .collect();
        body["subscribes"] = Value::Object(subscribes);
    }
    body
}

/// Section 10.9: deliver the message that many times to every subscription of the topic.
fn respond(ir: &Ir, opts: &Opts, c: &Captured) -> Value {
    let topic = &c.headers[&TOPIC.to_lowercase()];
    let deliveries: usize = c.headers[&DELIVERIES.to_lowercase()].parse().unwrap();
    let channel = ir
        .channels
        .iter()
        .position(|ch| &ch.topic == topic)
        .unwrap();
    let mut called: BTreeMap<String, u64> = BTreeMap::new();
    let (mut handled, mut rejected) = (false, None::<(&str, Vec<Value>)>);
    let mut dead_letters = 0;
    let mut inboxes: BTreeMap<String, MemoryInbox> = BTreeMap::new();
    for _ in 0..deliveries {
        for s in ir
            .services
            .iter()
            .filter(|s| opts.services.contains(&s.name.as_str()))
        {
            for sub in s
                .subscriptions
                .iter()
                .filter(|x| x.channel.index() == channel)
            {
                let events: Vec<_> = sub.handlers.iter().map(|h| h.event).collect();
                let got = decode(ir, &events, c.body.as_bytes());
                let got = got.and_then(|r| constraints(ir, opts, r));
                match got {
                    Ok(r) => {
                        handled = true;
                        let run = |called: &mut BTreeMap<String, u64>| {
                            for h in sub.handlers.iter().filter(|h| h.event == r.event) {
                                *called.entry(format!("{}.{}", s.name, h.name)).or_default() += 1;
                            }
                            Ok::<(), ()>(())
                        };
                        if opts.inbox {
                            let inbox = inboxes.entry(s.name.clone()).or_default();
                            inbox.handle(&r.source, &r.id, || run(&mut called)).unwrap();
                        } else {
                            run(&mut called).unwrap();
                        }
                    }
                    Err(ConsumeError::UnknownType(_)) => {}
                    Err(e) => {
                        dead_letters += u64::from(ir.channels[channel].dead_letter);
                        let violations = match &e {
                            ConsumeError::Rule(vs) => vs
                                .iter()
                                .map(|v| json!({ "rule": v.rule_id, "path": v.path, "fields": v.fields, "message": v.message }))
                                .collect(),
                            ConsumeError::Malformed(_)
                            | ConsumeError::UnknownType(_)
                            | ConsumeError::Shape(_) => Vec::new(),
                        };
                        rejected = Some((e.rejection().unwrap(), violations));
                    }
                }
            }
        }
    }
    answer(handled, rejected, called, dead_letters)
}

/// A generated consumer rejects what breaks a constraint as `shape`.
fn constraints(
    ir: &Ir,
    opts: &Opts,
    r: veld_rules::envelope::Received,
) -> Result<veld_rules::envelope::Received, ConsumeError> {
    if opts.constraints
        && !veld_rules::example::constraint_findings(ir, r.event, &r.data).is_empty()
    {
        return Err(ConsumeError::Shape("a constraint is broken".into()));
    }
    Ok(r)
}

fn answer(
    handled: bool,
    rejected: Option<(&str, Vec<Value>)>,
    called: BTreeMap<String, u64>,
    dead_letters: u64,
) -> Value {
    let (outcome, reason, violations) = match rejected {
        Some((reason, violations)) => ("rejected", json!(reason), violations),
        None if handled => ("handled", Value::Null, Vec::new()),
        None => ("ignored", Value::Null, Vec::new()),
    };
    json!({ "outcome": outcome, "reason": reason, "called": called, "violations": violations, "deadLetters": dead_letters })
}

fn run(fake: &Fake, ir: &Ir, name: &str) -> veld_rules::ExampleReport {
    let live = Live::connect(&fake.url(), false).unwrap();
    live.run_example(ir, example(ir, name)).unwrap()
}

fn failure(report: &veld_rules::ExampleReport) -> String {
    match &report.verdict {
        Verdict::Failed(m) => m.to_string(),
        Verdict::Passed | Verdict::Skipped(_) => panic!("expected a failure: {report}"),
    }
}

// --- the examples of tests/events/delivery.veld ----------------------------------------------

#[test]
fn every_example_of_the_delivery_contract_passes_against_a_reference_handler() {
    let ir = delivery();
    let fake = serve(ir.clone(), Opts::default());
    let live = Live::connect(&fake.url(), false).unwrap();
    assert!(live.serves_events());
    let events: Vec<&Example> = ir.examples.iter().filter(|e| e.event.is_some()).collect();
    assert!(events.len() >= 4, "{}", events.len());
    for ex in events {
        let report = live.run_example(&ir, ex).unwrap();
        assert!(report.passed(), "{report}");
    }
}

#[test]
fn the_request_is_the_message_with_the_three_headers() {
    let ir = delivery();
    let fake = serve(ir.clone(), Opts::default());
    let name = "a placed order is handled once even when it is delivered twice";
    assert!(run(&fake, &ir, name).passed());
    let sent = fake.events();
    assert_eq!(sent.len(), 1);
    let c = &sent[0];
    assert_eq!(c.method, "POST");
    assert_eq!(c.headers[&TOPIC.to_lowercase()], "shop.orders.v1");
    assert_eq!(c.headers[&DELIVERIES.to_lowercase()], "2");
    assert_eq!(
        c.headers["content-type"],
        "application/cloudevents+json; charset=UTF-8"
    );
    let m: Value = serde_json::from_str(&c.body).unwrap();
    assert_eq!(m["specversion"], "1.0");
    assert_eq!(m["source"], "veld:t/Orders");
    assert_eq!(m["subject"], c.headers[&KEY.to_lowercase()].as_str());
    assert_eq!(m["data"]["quantity"], 2);
    // The body is the canonical text of the reference encoder: no spaces, members sorted.
    let keys: Vec<&String> = m.as_object().unwrap().keys().collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
    assert!(!c.body.contains(": ") && !c.body.contains('\n'));
}

#[test]
fn one_delivery_for_called_and_two_for_called_once() {
    let ir = delivery();
    let fake = serve(ir.clone(), Opts::default());
    run(&fake, &ir, "called once and called are the same offline");
    run(
        &fake,
        &ir,
        "an order that breaks its rule is rejected, and with @deadLetter it is copied",
    );
    let sent = fake.events();
    // The first has a `called once` line on another handler: two deliveries.
    assert_eq!(sent[0].headers[&DELIVERIES.to_lowercase()], "2");
    assert_eq!(sent[1].headers[&DELIVERIES.to_lowercase()], "1");
}

#[test]
fn a_rejected_message_is_built_without_the_checks_of_a_publisher() {
    let ir = delivery();
    let fake = serve(ir.clone(), Opts::default());
    let name = "an order that breaks its rule is rejected, and with @deadLetter it is copied";
    assert!(run(&fake, &ir, name).passed());
    let m: Value = serde_json::from_str(&fake.events()[0].body).unwrap();
    assert_eq!(m["data"]["quantity"], 0);
    assert!(
        m["veldrules"].is_string(),
        "the consumer must check the rules"
    );
}

// --- what makes a live example fail ----------------------------------------------------------

#[test]
fn a_handler_without_an_inbox_fails_called_once() {
    let ir = delivery();
    let fake = serve(
        ir.clone(),
        Opts {
            inbox: false,
            ..Opts::default()
        },
    );
    let report = run(
        &fake,
        &ir,
        "a placed order is handled once even when it is delivered twice",
    );
    let text = failure(&report);
    assert!(text.contains("Fulfilment.reserve"), "{text}");
    assert!(text.contains("called once"), "{text}");
    assert!(text.contains("run 2 times"), "{text}");
}

#[test]
fn a_handler_that_runs_a_handler_the_example_says_not_to_is_a_failure() {
    let ir = delivery();
    let fake = serve(
        ir.clone(),
        Opts {
            tamper: Some(|a| a["called"]["Billing.charge"] = json!(1)),
            ..Opts::default()
        },
    );
    let report = run(
        &fake,
        &ir,
        "called once also holds for the handlers it does not name",
    );
    let text = failure(&report);
    assert!(text.contains("Billing.charge"), "{text}");
}

#[test]
fn a_handler_that_does_not_run_a_called_handler_is_a_failure() {
    let ir = delivery();
    let fake = serve(
        ir.clone(),
        Opts {
            tamper: Some(|a| a["called"]["Fulfilment.reserve"] = json!(0)),
            ..Opts::default()
        },
    );
    let text = failure(&run(
        &fake,
        &ir,
        "called once and called are the same offline",
    ));
    assert!(text.contains("Fulfilment.reserve"), "{text}");
}

#[test]
fn another_reason_or_other_violations_than_the_reference_are_failures() {
    let ir = delivery();
    let name = "an order that breaks its rule is rejected, and with @deadLetter it is copied";
    let wrong_reason = serve(
        ir.clone(),
        Opts {
            tamper: Some(|a| a["reason"] = json!("shape")),
            ..Opts::default()
        },
    );
    assert!(failure(&run(&wrong_reason, &ir, name)).contains("reason"));
    let wrong_rule = serve(
        ir.clone(),
        Opts {
            tamper: Some(|a| a["violations"][0]["rule"] = json!("veld:t/Other.rule@0")),
            ..Opts::default()
        },
    );
    assert!(failure(&run(&wrong_rule, &ir, name)).contains("violations"));
    let handled = serve(
        ir.clone(),
        Opts {
            tamper: Some(|a| {
                a["outcome"] = json!("handled");
                a["reason"] = Value::Null;
            }),
            ..Opts::default()
        },
    );
    assert!(failure(&run(&handled, &ir, name)).contains("rejected"));
}

#[test]
fn an_answer_that_is_not_section_10_9_is_a_failure_with_the_reason() {
    let ir = delivery();
    let name = "called once and called are the same offline";
    let status = serve(
        ir.clone(),
        Opts {
            status: 500,
            ..Opts::default()
        },
    );
    assert!(failure(&run(&status, &ir, name)).contains("HTTP 500"));
    let no_outcome = serve(
        ir.clone(),
        Opts {
            tamper: Some(|a| {
                a.as_object_mut().unwrap().remove("outcome");
            }),
            ..Opts::default()
        },
    );
    assert!(failure(&run(&no_outcome, &ir, name)).contains("outcome"));
}

// --- what makes it a skip --------------------------------------------------------------------

#[test]
fn a_handler_without_a_subscriber_of_the_topic_skips_the_example() {
    let ir = delivery();
    let fake = serve(
        ir.clone(),
        Opts {
            services: vec!["Orders"],
            ..Opts::default()
        },
    );
    let report = run(&fake, &ir, "called once and called are the same offline");
    let Verdict::Skipped(why) = &report.verdict else {
        panic!("{report}")
    };
    assert!(why.contains("no subscriber of `shop.orders.v1`"), "{why}");
    assert!(fake.events().is_empty(), "nothing is sent");
}

#[test]
fn a_probe_without_subscribes_skips_with_that_reason() {
    let ir = delivery();
    let fake = serve(
        ir.clone(),
        Opts {
            list_subscribes: false,
            ..Opts::default()
        },
    );
    let report = run(&fake, &ir, "called once and called are the same offline");
    let Verdict::Skipped(why) = &report.verdict else {
        panic!("{report}")
    };
    assert!(why.contains("does not serve events"), "{why}");
    assert!(fake.events().is_empty());
}

#[test]
fn a_line_about_a_service_the_handler_does_not_serve_is_not_judged() {
    let ir = delivery();
    let fake = serve(
        ir.clone(),
        Opts {
            services: vec!["Fulfilment"],
            ..Opts::default()
        },
    );
    let name = "a placed order is handled once even when it is delivered twice";
    assert!(run(&fake, &ir, name).passed());
}

// --- constraints: the live run is where they are checked -------------------------------------

const LIMITED: &str = r#"
event Limited {
  n: int @max(10)
}
channel C { carries Limited }
service P { publishes C }
service S { subscribes C as "g" { on Limited -> h } }
example "too big" { given event Limited { n: 11 }
  expect rejected }
example "fine" { given event Limited { n: 3 }
  expect S.h called }
"#;

fn limited_opts(constraints: bool) -> Opts {
    Opts {
        services: vec!["S"],
        constraints,
        ..Opts::default()
    }
}

#[test]
fn an_example_that_is_live_only_offline_passes_against_a_consumer_that_checks_constraints() {
    let ir = compile_text(LIMITED);
    let offline = veld_rules::run_example(&ir, example(&ir, "too big")).unwrap();
    assert!(matches!(offline.verdict, Verdict::Skipped(_)), "{offline}");
    let fake = serve(ir.clone(), limited_opts(true));
    assert!(run(&fake, &ir, "too big").passed());
    assert!(run(&fake, &ir, "fine").passed());
    let rejected: Value = serde_json::from_str(&fake.events()[0].body).unwrap();
    assert_eq!(rejected["data"]["n"], 11);
}

#[test]
fn a_consumer_that_does_not_check_constraints_fails_the_live_only_example() {
    let ir = compile_text(LIMITED);
    let fake = serve(ir.clone(), limited_opts(false));
    let text = failure(&run(&fake, &ir, "too big"));
    assert!(text.contains("rejected"), "{text}");
}

//! Invariants over HTTP (`spec/rules.md` sections 5.3, 5.4 and 7): the generated Rust server and
//! client of `tests/toolchain/contract/rules/bookings.veld`, over a real socket.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bookings_client::bookings::bookings::{Booking as ClientBooking, Status};
use bookings_client::bookings::bookings_client::{BookingsClient, CreateBookingRequest, ReceiptRequest};
use bookings_client::prelude::ValidationError;
use bookings_client::veld::DeclaredError;
use bookings_client::veld::client::ClientOptions;
use bookings_server::bookings::bookings::Receipt;
use bookings_server::bookings::bookings_server::{
    BookingsHandler, BookingsHooks, BookingsService, CreateBookingRequest as ServerCreate, ReceiptRequest as ServerReceipt,
};
use bookings_server::veld::Error;
use bookings_server::veld::rules::{Outcome, Position, RuleEvent, RuleOutput};
use bookings_server::veld::server::RequestContext;

type Events = Arc<Mutex<Vec<RuleEvent>>>;

struct Hooks {
    mode: RuleOutput,
    events: Events,
    validate: bool,
}

impl BookingsHooks for Hooks {
    fn rules_output(&self) -> RuleOutput {
        self.mode
    }

    fn validate_requests(&self) -> bool {
        self.validate
    }

    fn on_rule_violation(&self, event: &RuleEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

struct Impl;

impl BookingsService for Impl {
    async fn create_booking(&self, _request: ServerCreate, _ctx: &RequestContext) -> Result<Receipt, Error> {
        Ok(Receipt { total: 5 })
    }

    async fn receipt(&self, request: ServerReceipt, _ctx: &RequestContext) -> Result<Receipt, Error> {
        Ok(Receipt { total: request.total })
    }
}

struct Server {
    base: String,
    events: Events,
    hits: Arc<AtomicUsize>,
}

async fn serve(mode: RuleOutput) -> Server {
    serve_with(mode, true).await
}

async fn serve_with(mode: RuleOutput, validate: bool) -> Server {
    let events: Events = Arc::default();
    let hits = Arc::new(AtomicUsize::new(0));
    let handler = BookingsHandler::create(Impl, Hooks { mode, events: events.clone(), validate });
    let counter = hits.clone();
    let router = bookings_server::adapters::axum::router(handler).layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                next.run(req).await
            }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, events, hits }
}

fn booking(start: &str, end: &str) -> ClientBooking {
    ClientBooking {
        guest: "Ada".into(),
        start: start.into(),
        end: end.into(),
        status: Status::Pending,
        rooms: vec![1],
    }
}

const BAD: (&str, &str) = ("2026-01-02T00:00:00Z", "2026-01-01T00:00:00Z");
const GOOD: (&str, &str) = ("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z");

fn assert_end_after_start(err: &bookings_client::veld::Error, local: bool, what: &str) {
    assert!(ValidationError::matches(err), "{what}: not a ValidationError: {err:?}");
    assert_eq!(err.status(), Some(422), "{what}");
    assert_eq!(err.is_local(), local, "{what}: local flag");
    let violations = err.violations();
    assert_eq!(violations.len(), 1, "{what}: {violations:?}");
    let v = &violations[0];
    assert!(v.rule.starts_with("veld:bookings/Booking.endAfterStart@"), "{what}: {}", v.rule);
    assert_eq!(v.path, "body", "{what}");
    assert_eq!(v.fields, ["end", "start"], "{what}");
    assert_eq!(v.message, "A booking must end after it starts", "{what}");
    assert_eq!(v.error, None, "{what}");
    let typed = ValidationError::from_error(err).unwrap();
    assert_eq!(typed.issues.len(), 1, "{what}: one issue per violation");
    assert_eq!(typed.issues[0].code, "rule", "{what}");
}

async fn input_rules() {
    let server = serve(RuleOutput::Strict).await;
    // The raw problem document of section 7.
    let raw = reqwest::Client::new()
        .post(format!("{}/bookings", server.base))
        .header("content-type", "application/json")
        .body(r#"{"guest":"a","start":"2026-01-02T00:00:00Z","end":"2026-01-01T00:00:00Z","rooms":[1]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(raw.status().as_u16(), 422);
    let doc: serde_json::Value = serde_json::from_str(&raw.text().await.unwrap()).unwrap();
    assert_eq!(doc["data"]["issues"][0]["code"], "rule");
    assert_eq!(doc["data"]["issues"][0]["path"], "body");
    let violation = &doc["violations"][0];
    assert!(violation["rule"].as_str().unwrap().starts_with("veld:bookings/Booking.endAfterStart@"));
    assert_eq!(violation["path"], "body");
    assert_eq!(violation["fields"], serde_json::json!(["end", "start"]));
    assert_eq!(doc["violations"].as_array().unwrap().len(), 1);
    {
        let events = server.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!((events[0].position, events[0].outcome), (Position::Input, Outcome::Rejected));
        assert!(events[0].operation.starts_with("veld:bookings/Bookings.createBooking@"));
        assert_eq!(events[0].violation.fields, ["end", "start"]);
    }

    // Local check off: the server answers, the client decodes the same violations.
    let remote = BookingsClient::reqwest(ClientOptions::new().base_url(&server.base).check_rules(false)).unwrap();
    let hits = server.hits.load(Ordering::SeqCst);
    let err = remote.create_booking(CreateBookingRequest { body: booking(BAD.0, BAD.1) }).await.unwrap_err();
    assert_end_after_start(&err, false, "remote");
    assert_eq!(server.hits.load(Ordering::SeqCst), hits + 1, "the request was sent");

    // Local check on (the default): the same error, raised locally, nothing sent. Until a response
    // has shown the server's rules digest (`spec/rules.md` section 9.7) a client sends anyway and
    // the server decides: the first call is sent, and answers with the digest it was built from.
    let local = BookingsClient::reqwest(ClientOptions::new().base_url(&server.base)).unwrap();
    let first = local.create_booking(CreateBookingRequest { body: booking(BAD.0, BAD.1) }).await.unwrap_err();
    assert!(!first.is_local(), "before the first response the server decides: {first:?}");
    assert_end_after_start(&first, false, "first call");
    let hits = server.hits.load(Ordering::SeqCst);
    let err = local.create_booking(CreateBookingRequest { body: booking(BAD.0, BAD.1) }).await.unwrap_err();
    assert_end_after_start(&err, true, "local");
    assert_eq!(server.hits.load(Ordering::SeqCst), hits, "nothing was sent");

    // Several violations at once, and an evaluation error that fails closed.
    let mut two = booking(BAD.0, BAD.1);
    two.rooms = vec![0];
    let err = local.create_booking(CreateBookingRequest { body: two }).await.unwrap_err();
    assert_eq!(err.violations().len(), 2, "{:?}", err.violations());

    // A valid body is sent and answered.
    let ok = local.create_booking(CreateBookingRequest { body: booking(GOOD.0, GOOD.1) }).await.unwrap();
    assert_eq!(ok.total, 5);
}

async fn output_rules() {
    let strict = serve(RuleOutput::Strict).await;
    let client = BookingsClient::reqwest(ClientOptions::new().base_url(&strict.base)).unwrap();
    assert_eq!(client.receipt(ReceiptRequest { total: 3 }).await.unwrap().total, 3);
    assert!(strict.events.lock().unwrap().is_empty());
    let err = client.receipt(ReceiptRequest { total: 0 }).await.unwrap_err();
    assert_eq!(err.status(), Some(500), "strict: {err:?}");
    assert!(err.violations().is_empty(), "no rule detail leaks");
    {
        let events = strict.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!((events[0].position, events[0].outcome), (Position::Output, Outcome::Rejected));
        assert!(events[0].rule.starts_with("veld:bookings/Receipt.positive@"));
        assert_eq!(events[0].violation.path, "$");
    }

    let audit = serve(RuleOutput::Audit).await;
    let client = BookingsClient::reqwest(ClientOptions::new().base_url(&audit.base)).unwrap();
    assert_eq!(client.receipt(ReceiptRequest { total: 0 }).await.unwrap().total, 0, "audit sends it");
    let events = audit.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].position, events[0].outcome), (Position::Output, Outcome::Audited));
    drop(events);

    let off = serve(RuleOutput::Off).await;
    let client = BookingsClient::reqwest(ClientOptions::new().base_url(&off.base)).unwrap();
    assert_eq!(client.receipt(ReceiptRequest { total: 0 }).await.unwrap().total, 0);
    assert!(off.events.lock().unwrap().is_empty());
}

/// The child process of [`validation_off`]: a server whose request validation is off. It prints
/// its port and serves until it is killed.
async fn serve_off() {
    let server = serve_with(RuleOutput::Strict, false).await;
    println!("PORT={}", server.base.rsplit(':').next().unwrap());
    std::future::pending::<()>().await;
}

/// Request validation off (`spec/rules.md` section 5.3): the server accepts a rule-violating body,
/// says so once when it is created, and a client with validation off sends it too.
async fn validation_off() {
    use std::io::{BufRead, BufReader, Read};
    use std::process::{Command, Stdio};
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("serve-off")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let port = line.trim().strip_prefix("PORT=").unwrap_or_else(|| panic!("no port: {line:?}"));
    let base = format!("http://127.0.0.1:{port}");
    let bad = r#"{"guest":"a","start":"2026-01-02T00:00:00Z","end":"2026-01-01T00:00:00Z","rooms":[1]}"#;
    for _ in 0..2 {
        let raw = reqwest::Client::new()
            .post(format!("{base}/bookings"))
            .header("content-type", "application/json")
            .body(bad)
            .send()
            .await
            .unwrap();
        assert_eq!(raw.status().as_u16(), 201, "the rule-violating body is accepted");
    }
    // The client's local check is off with validation: the body is sent and accepted.
    let client = BookingsClient::reqwest(ClientOptions::new().base_url(&base).validate(false)).unwrap();
    let ok = client.create_booking(CreateBookingRequest { body: booking(BAD.0, BAD.1) }).await.unwrap();
    assert_eq!(ok.total, 5);
    child.kill().unwrap();
    child.wait().unwrap();
    let mut stderr = String::new();
    child.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
    let said = stderr.lines().filter(|l| l.contains("request validation is off")).count();
    assert_eq!(said, 1, "reported once, when the handler is created: {stderr:?}");
}

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("serve-off") {
        serve_off().await;
        return;
    }
    input_rules().await;
    output_rules().await;
    validation_off().await;
    println!("rules roundtrip ok");
}

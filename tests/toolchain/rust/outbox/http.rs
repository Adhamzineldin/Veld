//! The moves of `tests/events/outbox.veld.pending` over the production handler
//! (`spec/events.md` section 11, `spec/targets/rust.md` section 13, phase E3): a handler that
//! writes its event (the entry is in the outbox, nothing was published), one that forgets, one that
//! writes twice, one that writes the event of another edge, a declared error, an invalid event, a
//! move that emits nothing; then the relay with a transport and the subscriber of `Notifications`:
//! the event arrives, a second run sends nothing, an entry that fails to publish stays pending and
//! stops the run, and the limit. Prints `outbox http ok`.
#[path = "../common.rs"]
mod common;

use std::future::Future;
use std::sync::{Arc, Mutex};

use common::*;
use events_server::bookings::bookings_events::{CancelEmit, PayEmit, run_bookings_outbox_relay};
use events_server::bookings::bookings_server::{
    BookingsFactLoaders, BookingsHandler, BookingsHooks, BookingsService, CancelFacts, CancelRequest, PayFacts, PayRequest, RefundFacts,
    RefundRequest,
};
use events_server::bookings::notifications_events::{NotificationsEventHandlers, subscribe_notifications_events};
use events_server::bookings::t::{Booking, BookingCancelled, BookingPaid, RefundDue, Status};
use events_server::prelude::InvalidTransition;
use events_server::veld::events::{ConsumerOptions, EventMeta, MemoryOutbox, MemoryTransport, PublisherOptions, RelayOptions, Transport};
use events_server::veld::rules::Transition;
use events_server::veld::server::{Body, Request, RequestContext};
use events_server::veld::{Error, block_on};
use serde_json::{Value, json};

const ID: &str = "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21";
const MESSAGE_ID: &str = "0b5c9c3e-6a57-4d0e-9a55-1f0f6c2f7a11";
const TOPIC: &str = "shop.bookings.v1";

/// What the handler of a move does with its `emit`.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    /// Writes the events of the edge, once each.
    Write,
    /// Writes nothing.
    Forget,
    /// Writes the event of the edge twice.
    Twice,
    /// Writes an event the edge does not emit (a `cancel` from `paid` writes `BookingCancelled`).
    Wrong,
    /// Answers a declared error without writing.
    Refuse,
    /// Writes an event that breaks its invariant: `emit` fails and the handler returns the error.
    Invalid,
}

struct App {
    mode: Mutex<Mode>,
    /// The state the loader gives the booking.
    status: Mutex<Status>,
    outbox: MemoryOutbox,
    /// What `on_error` heard.
    errors: Mutex<Vec<String>>,
}

struct Impl(Arc<App>);

fn booking(request_id: &str, status: Status) -> Booking {
    Booking { id: request_id.to_string(), status, total: "120.00".to_string() }
}

impl BookingsService for Impl {
    async fn pay(&self, request: PayRequest, facts: PayFacts, transition: Transition<Status>, emit: PayEmit<'_>, _ctx: &RequestContext) -> Result<Booking, Error> {
        let mode = *self.0.mode.lock().unwrap();
        let paid = |amount: &str| BookingPaid { booking_id: request.id.clone(), amount: amount.to_string() };
        match mode {
            Mode::Write | Mode::Wrong => {
                emit.booking_paid(&paid("120.00")).await?;
            }
            Mode::Forget => {}
            Mode::Twice => {
                emit.booking_paid(&paid("120.00")).await?;
                emit.booking_paid(&paid("120.00")).await?;
            }
            Mode::Refuse => {
                return Err(InvalidTransition { field: "status".into(), from: "pending".into(), to: "paid".into() }.into());
            }
            Mode::Invalid => {
                emit.booking_paid(&paid("-1.00")).await?;
            }
        }
        Ok(Booking { status: transition.to, ..facts.booking })
    }

    async fn cancel(&self, request: CancelRequest, facts: CancelFacts, transition: Transition<Status>, emit: CancelEmit<'_>, _ctx: &RequestContext) -> Result<Booking, Error> {
        let mode = *self.0.mode.lock().unwrap();
        let from = transition.from;
        match mode {
            Mode::Write => {
                if from == Status::Pending {
                    emit.booking_cancelled(&BookingCancelled { booking_id: request.id.clone(), reason: None }).await?;
                } else {
                    emit.refund_due(&RefundDue { booking_id: request.id.clone() }).await?;
                }
            }
            // `paid -> cancelled` emits `RefundDue`; the handler writes it and `BookingCancelled` too.
            Mode::Wrong => {
                emit.refund_due(&RefundDue { booking_id: request.id.clone() }).await?;
                emit.booking_cancelled(&BookingCancelled { booking_id: request.id.clone(), reason: None }).await?;
            }
            Mode::Forget | Mode::Twice | Mode::Refuse | Mode::Invalid => {}
        }
        Ok(Booking { status: transition.to, ..facts.booking })
    }

    async fn refund(&self, _request: RefundRequest, facts: RefundFacts, transition: Transition<Status>, _ctx: &RequestContext) -> Result<Booking, Error> {
        Ok(Booking { status: transition.to, ..facts.booking })
    }
}

struct Loaders(Arc<App>);

impl BookingsFactLoaders for Loaders {
    fn booking(&self, key: String, _ctx: &RequestContext) -> impl Future<Output = Result<Option<Booking>, Error>> + Send {
        std::future::ready(Ok(Some(booking(&key, *self.0.status.lock().unwrap()))))
    }
}

struct Hooks(Arc<App>);

impl BookingsHooks for Hooks {
    type Outbox = MemoryOutbox;

    fn outbox(&self) -> &MemoryOutbox {
        &self.0.outbox
    }

    fn events(&self) -> PublisherOptions {
        PublisherOptions::new().new_id(|| MESSAGE_ID.to_string()).clock(|| time_from("2026-06-10T12:00:00Z"))
    }

    fn on_error(&self, err: &Error, _ctx: &RequestContext) {
        self.0.errors.lock().unwrap().push(err.to_string());
    }
}

struct Server {
    app: Arc<App>,
    handler: events_server::veld::server::Handler<events_server::bookings::bookings_server::BookingsDispatch<Impl, Hooks, Loaders>>,
}

fn serve() -> Server {
    let app = Arc::new(App {
        mode: Mutex::new(Mode::Write),
        status: Mutex::new(Status::Pending),
        outbox: MemoryOutbox::new(),
        errors: Mutex::new(Vec::new()),
    });
    let handler = BookingsHandler::create(Impl(app.clone()), Hooks(app.clone()), Loaders(app.clone()), None).unwrap();
    Server { app, handler }
}

struct Answer {
    status: u16,
    doc: Value,
}

fn call(s: &Server, route: &str, mode: Mode, status: Status) -> Answer {
    *s.app.mode.lock().unwrap() = mode;
    *s.app.status.lock().unwrap() = status;
    let request = Request { method: "POST".into(), path: format!("/bookings/{ID}/{route}"), ..Request::default() };
    let response = block_on(s.handler.handle(request)).expect("a route");
    let doc = match response.body {
        Body::Bytes(bytes) => serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        Body::Empty => Value::Null,
    };
    assert!(response.headers.iter().all(|(k, _)| k != "veld-test-emitted"), "only a test handler answers with Veld-Test-Emitted");
    Answer { status: response.status, doc }
}

fn ok(a: &Answer) -> bool {
    (200..300).contains(&a.status)
}

fn heard(s: &Server) -> Vec<String> {
    std::mem::take(&mut *s.app.errors.lock().unwrap())
}

fn entries(s: &Server) -> Vec<Value> {
    s.app.outbox.entries().iter().map(|e| json!({ "topic": e.topic, "key": e.key, "message": serde_json::from_slice::<Value>(&e.bytes).unwrap() })).collect()
}

fn clear(s: &Server) {
    for e in s.app.outbox.entries() {
        block_on(events_server::veld::events::Outbox::done(&s.app.outbox, &e)).unwrap();
    }
}

/// The moves over the production handler.
fn moves() {
    let s = serve();
    // A handler that writes: the entry is in the outbox, with the key and the bytes of a publisher.
    let a = call(&s, "pay", Mode::Write, Status::Pending);
    assert!(ok(&a), "{}", a.doc);
    let found = entries(&s);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["topic"], json!(TOPIC));
    assert_eq!(found[0]["key"], json!(ID));
    let message = &found[0]["message"];
    assert_eq!(message["id"], json!(MESSAGE_ID));
    assert_eq!(message["time"], json!("2026-06-10T12:00:00Z"));
    assert_eq!(message["source"], json!("veld:bookings/Bookings"));
    assert_eq!(message["data"], json!({ "bookingId": ID, "amount": "120.00" }));
    assert!(message["type"].as_str().unwrap().starts_with("veld:bookings/BookingPaid@"));
    assert!(heard(&s).is_empty());
    clear(&s);

    // A handler that forgets: the state changed without its event, and the server says so.
    let a = call(&s, "pay", Mode::Forget, Status::Pending);
    assert_eq!(a.status, 500, "{}", a.doc);
    assert_eq!(a.doc["title"], json!("Internal"));
    assert_eq!(heard(&s), vec!["pay: the move wrote the wrong events to the outbox: missing BookingPaid".to_string()]);
    assert!(entries(&s).is_empty());

    // Twice: both entries stay in the outbox (the application's transaction ends with the failure).
    let a = call(&s, "pay", Mode::Twice, Status::Pending);
    assert_eq!(a.status, 500, "{}", a.doc);
    assert_eq!(heard(&s), vec!["pay: the move wrote the wrong events to the outbox: twice BookingPaid".to_string()]);
    assert_eq!(entries(&s).len(), 2);
    clear(&s);

    // The event of another edge: `paid -> cancelled` emits `RefundDue`, the handler writes `BookingCancelled`.
    let a = call(&s, "cancel", Mode::Wrong, Status::Paid);
    assert_eq!(a.status, 500, "{}", a.doc);
    assert_eq!(heard(&s), vec!["cancel: the move wrote the wrong events to the outbox: unexpected BookingCancelled".to_string()]);
    clear(&s);
    // What is written depends on the edge taken: pending -> cancelled writes `BookingCancelled`, paid -> cancelled `RefundDue`.
    let a = call(&s, "cancel", Mode::Write, Status::Pending);
    assert!(ok(&a), "{}", a.doc);
    let found = entries(&s);
    assert_eq!(found.len(), 1);
    assert!(found[0]["message"]["type"].as_str().unwrap().starts_with("veld:bookings/BookingCancelled@"));
    clear(&s);
    let a = call(&s, "cancel", Mode::Write, Status::Paid);
    assert!(ok(&a), "{}", a.doc);
    let found = entries(&s);
    assert_eq!(found.len(), 1);
    assert!(found[0]["message"]["type"].as_str().unwrap().starts_with("veld:bookings/RefundDue@"));
    assert_eq!(found[0]["topic"], json!("shop.refunds.v1"), "RefundDue goes to the channel the service sends it on");
    clear(&s);
    assert!(heard(&s).is_empty());

    // A handler that answers a declared error need not have written.
    let a = call(&s, "pay", Mode::Refuse, Status::Pending);
    assert_eq!(a.status, 409, "{}", a.doc);
    assert_eq!(a.doc["title"], json!("InvalidTransition"));
    assert!(heard(&s).is_empty() && entries(&s).is_empty());
    // A move the guard refuses never reaches the handler.
    let a = call(&s, "pay", Mode::Write, Status::Paid);
    assert_eq!(a.status, 409);
    assert!(entries(&s).is_empty());

    // An invalid event: `emit` returns the typed error before anything is written.
    let a = call(&s, "pay", Mode::Invalid, Status::Pending);
    assert_eq!(a.status, 422, "{}", a.doc);
    assert_eq!(a.doc["title"], json!("ValidationError"));
    assert!(entries(&s).is_empty(), "nothing is written for a value that does not hold");

    // A move that emits nothing writes nothing and is not checked.
    let a = call(&s, "refund", Mode::Write, Status::Paid);
    assert!(ok(&a), "{}", a.doc);
    assert!(entries(&s).is_empty() && heard(&s).is_empty());
}

struct Notifications(Arc<Mutex<Vec<(String, String)>>>);

impl NotificationsEventHandlers for Notifications {
    fn send_receipt(&self, event: BookingPaid, meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        self.0.lock().unwrap().push((event.booking_id, meta.id));
        std::future::ready(Ok(()))
    }

    fn send_apology(&self, _event: BookingCancelled, _meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        std::future::ready(Ok(()))
    }
}

/// The relay with the memory transport and the subscriber of `Notifications`.
fn relay() {
    block_on(async {
        let s = serve();
        let a = call(&s, "pay", Mode::Write, Status::Pending);
        assert!(ok(&a), "{}", a.doc);
        let memory = Arc::new(MemoryTransport::new());
        let received: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
        let transport: Arc<dyn Transport> = memory.clone();
        subscribe_notifications_events(transport.clone(), Arc::new(Notifications(received.clone())), ConsumerOptions::new()).await.unwrap();
        assert!(received.lock().unwrap().is_empty(), "nothing was published when the handler wrote");

        let sent = run_bookings_outbox_relay(&s.app.outbox, &*transport, &RelayOptions::new()).await;
        memory.idle().await;
        assert_eq!(sent, 1);
        assert_eq!(*received.lock().unwrap(), vec![(ID.to_string(), MESSAGE_ID.to_string())], "the subscriber's handler got the event");
        assert_eq!(s.app.outbox.size(), 0);
        // A second run has nothing to send: the entry was marked done.
        let sent = run_bookings_outbox_relay(&s.app.outbox, &*transport, &RelayOptions::new()).await;
        memory.idle().await;
        assert_eq!(sent, 0);
        assert_eq!(received.lock().unwrap().len(), 1, "an entry is not published twice");

        // An entry whose publish fails stays pending and stops the run; the failure goes to on_error.
        for _ in 0..3 {
            assert!(ok(&call(&s, "pay", Mode::Write, Status::Pending)));
        }
        let recorder = Recorder::new();
        recorder.fail(Some(TOPIC));
        let failures: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
        let f = failures.clone();
        let options = RelayOptions::new().on_error(move |error, entry| f.lock().unwrap().push(entry.map(|en| format!("{}/{error}", en.id))));
        let first = s.app.outbox.entries()[0].id.clone();
        let sent = run_bookings_outbox_relay(&s.app.outbox, &*recorder, &options).await;
        assert_eq!(sent, 0);
        assert_eq!(s.app.outbox.size(), 3, "all three are still pending, in order");
        assert_eq!(failures.lock().unwrap().len(), 1);
        assert!(failures.lock().unwrap()[0].as_deref().unwrap().starts_with(&format!("{first}/")));
        // The limit: two of the three, the third stays.
        recorder.fail(None);
        let sent = run_bookings_outbox_relay(&s.app.outbox, &*recorder, &RelayOptions::new().limit(2)).await;
        assert_eq!(sent, 2);
        assert_eq!(s.app.outbox.size(), 1);
        assert_eq!(recorder.messages().len(), 2);
        // The rest on the next run.
        let sent = run_bookings_outbox_relay(&s.app.outbox, &*recorder, &RelayOptions::new()).await;
        assert_eq!((sent, s.app.outbox.size(), recorder.messages().len()), (1, 0, 3));
    });
}

fn main() {
    moves();
    relay();
    println!("outbox http ok");
}

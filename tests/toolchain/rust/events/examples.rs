//! The event examples of tests/events/bookings.veld against the generated consumers and the
//! generated publisher. Reads `cases.json` (written by the test) and prints one JSON array: for
//! each example the handlers called when its message is delivered as it is (`raw`), and what the
//! publisher does with its value (`viaPublisher`: `refused` is the title of the error, `called`
//! the handlers that ran).
#[path = "../common.rs"]
mod common;

use std::future::Future;
use std::sync::{Arc, Mutex};

use events_server::bookings::bookings_events::create_bookings_publisher;
use events_server::bookings::ledger_events::{LedgerEventHandlers, subscribe_ledger_events};
use events_server::bookings::notifications_events::{
    NotificationsEventHandlers, subscribe_notifications_events,
};
use events_server::bookings::t::{BookingArchived, BookingCancelled, BookingPaid};
use events_server::veld::events::{ChannelOptions, ConsumerOptions, EventMeta, MemoryTransport, PublisherOptions, Transport};
use events_server::veld::{Error, VeldDecl, block_on};
use serde_json::{Value, json};

type Log = Arc<Mutex<Vec<String>>>;

struct Notifications(Log);
struct Ledger(Log);

fn note(log: &Log, name: &str) -> std::future::Ready<Result<(), Error>> {
    log.lock().unwrap().push(name.to_string());
    std::future::ready(Ok(()))
}

impl NotificationsEventHandlers for Notifications {
    fn send_receipt(&self, _: BookingPaid, _: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        note(&self.0, "Notifications.sendReceipt")
    }

    fn send_apology(&self, _: BookingCancelled, _: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        note(&self.0, "Notifications.sendApology")
    }
}

impl LedgerEventHandlers for Ledger {
    fn record_payment(&self, _: BookingPaid, _: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        note(&self.0, "Ledger.recordPayment")
    }

    fn audit_payment(&self, _: BookingPaid, _: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        note(&self.0, "Ledger.auditPayment")
    }
}

async fn world() -> (Arc<MemoryTransport>, Arc<dyn Transport>, Log) {
    let transport = Arc::new(MemoryTransport::new());
    let t: Arc<dyn Transport> = transport.clone();
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    subscribe_notifications_events(t.clone(), Arc::new(Notifications(log.clone())), ConsumerOptions::new()).await.unwrap();
    subscribe_ledger_events(t.clone(), Arc::new(Ledger(log.clone())), ConsumerOptions::new()).await.unwrap();
    (transport, t, log)
}

fn sorted(log: &Log) -> Vec<String> {
    let mut v = log.lock().unwrap().clone();
    v.sort();
    v
}

async fn raw(case: &Value) -> Vec<String> {
    let (transport, t, log) = world().await;
    let key = case["key"].as_str();
    t.publish(case["topic"].as_str().unwrap(), key, case["message"].as_str().unwrap().as_bytes(), ChannelOptions::NONE).await.unwrap();
    transport.idle().await;
    sorted(&log)
}

async fn publish(case: &Value, t: Arc<dyn Transport>) -> Result<(), Error> {
    let p = create_bookings_publisher(t, PublisherOptions::new());
    let value = &case["value"];
    let publisher = &case["publisher"];
    let (channel, event) = (publisher["channel"].as_str().unwrap(), publisher["companion"].as_str().unwrap());
    match (channel, event) {
        ("bookingEvents", "BookingPaid") => p.booking_events().booking_paid(&BookingPaid::from_wire(value)?).await.map(drop),
        ("bookingEvents", "BookingCancelled") => {
            p.booking_events().booking_cancelled(&BookingCancelled::from_wire(value)?).await.map(drop)
        }
        ("bookingEvents", "BookingArchived") => {
            p.booking_events().booking_archived(&BookingArchived::from_wire(value)?).await.map(drop)
        }
        ("audit", "BookingPaid") => p.audit().booking_paid(&BookingPaid::from_wire(value)?).await.map(drop),
        other => panic!("the driver does not know {other:?}"),
    }
}

async fn via_publisher(case: &Value) -> Value {
    let (transport, t, log) = world().await;
    let refused = publish(case, t).await.err().map(|e| e.problem().map_or_else(|| e.to_string(), |p| p.title.clone()));
    transport.idle().await;
    json!({ "refused": refused, "called": sorted(&log) })
}

fn main() {
    let text = std::fs::read_to_string("cases.json").unwrap();
    let cases: Value = serde_json::from_str(&text).unwrap();
    let out: Vec<Value> = cases["examples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| block_on(async { json!({ "raw": raw(case).await, "viaPublisher": via_publisher(case).await }) }))
        .collect();
    println!("{}", serde_json::to_string(&out).unwrap());
}

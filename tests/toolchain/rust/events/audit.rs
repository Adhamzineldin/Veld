//! An audited invariant of an event (`@rollout(audit)`, tests/wire/events/edges.veld, event
//! `InvoiceNudged`): it is evaluated and reported to `on_rule_violation` and never rejects, on the
//! way out and on the way in. Prints one JSON document: what the hooks heard and what was
//! delivered, for a value that breaks the rule and one that does not.
#[path = "../common.rs"]
mod common;

use std::future::Future;
use std::sync::{Arc, Mutex};

use events_server::bookings::billing_events::create_billing_publisher;
use events_server::bookings::audit_probe_events::{AuditProbeEventHandlers, subscribe_audit_probe_events};
use events_server::bookings::t::InvoiceNudged;
use events_server::veld::events::{
    BoxFuture, ConsumerOptions, Deliver, EventMeta, MemoryTransport, PublisherOptions, Transport, TransportSubscription,
};
use events_server::veld::{Error, block_on};
use serde_json::{Value, json};

const INVOICE: &str = "7e6f0a52-3d84-4a4b-9c27-0f8e2b1d6a90";

type Log<T> = Arc<Mutex<Vec<T>>>;

struct Probe {
    delivered: Log<i32>,
}

impl AuditProbeEventHandlers for Probe {
    fn on_invoice_nudged(&self, event: InvoiceNudged, _: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        self.delivered.lock().unwrap().push(event.days);
        std::future::ready(Ok(()))
    }
}

/// The memory transport that also keeps the text of what is published.
struct Spy {
    inner: Arc<MemoryTransport>,
    sent: Log<String>,
}

impl Transport for Spy {
    fn publish<'a>(&'a self, topic: &'a str, key: Option<&'a str>, bytes: &'a [u8]) -> BoxFuture<'a, Result<(), Error>> {
        self.sent.lock().unwrap().push(String::from_utf8_lossy(bytes).into_owned());
        self.inner.publish(topic, key, bytes)
    }

    fn subscribe<'a>(
        &'a self,
        topic: &'a str,
        group: &'a str,
        deliver: Deliver,
    ) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>> {
        self.inner.subscribe(topic, group, deliver)
    }
}

fn heard(log: &Log<Value>) -> Value {
    Value::Array(log.lock().unwrap().clone())
}

async fn run(days: i32) -> Value {
    let memory = Arc::new(MemoryTransport::new());
    let (published, consumed, rejected): (Log<Value>, Log<Value>, Log<String>) = Default::default();
    let delivered: Log<i32> = Default::default();
    let (c, r) = (consumed.clone(), rejected.clone());
    let consumer_transport: Arc<dyn Transport> = memory.clone();
    subscribe_audit_probe_events(
        consumer_transport,
        Arc::new(Probe { delivered: delivered.clone() }),
        ConsumerOptions::new()
            .on_rule_violation(move |e| c.lock().unwrap().push(event_json(e)))
            .on_event_rejected(move |e| r.lock().unwrap().push(e.reason.as_str().to_string())),
    )
    .await
    .unwrap();
    let sent: Log<String> = Default::default();
    let spy: Arc<dyn Transport> = Arc::new(Spy { inner: memory.clone(), sent: sent.clone() });
    let p = published.clone();
    let billing = create_billing_publisher(spy, PublisherOptions::new().on_rule_violation(move |e| p.lock().unwrap().push(event_json(e))));
    let event = InvoiceNudged { invoice_id: INVOICE.to_string(), days };
    billing.invoice_events().invoice_nudged(&event).await.unwrap();
    memory.idle().await;
    json!({
        "days": days, "published": heard(&published), "consumed": heard(&consumed),
        "delivered": *delivered.lock().unwrap(), "rejected": *rejected.lock().unwrap(), "sent": *sent.lock().unwrap(),
    })
}

fn event_json(e: &events_server::veld::rules::RuleEvent) -> Value {
    json!({
        "rule": e.rule, "operation": e.operation,
        "position": format!("{:?}", e.position).to_lowercase(),
        "outcome": format!("{:?}", e.outcome).to_lowercase(),
    })
}

fn main() {
    let out = block_on(async { json!({ "breaking": run(45).await, "fine": run(10).await }) });
    println!("{out}");
}

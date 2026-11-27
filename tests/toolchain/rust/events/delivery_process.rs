//! Events phase E2 in one process over the memory transport (`tests/events/delivery.veld`):
//! `Orders` publishes `OrderEvents` (`@retry(max: 3, backoff: exponential)`, `@deadLetter`,
//! `@ordered`), `Fulfilment` and `Billing` subscribe. Retries with the waits recorded by a fake
//! `sleep`, the dead-letter copy read by a subscriber of `<topic>.dlq` and published again, a copy
//! that cannot be published, rejected messages, the inbox, and the options the transport gets.
//! Prints `delivery process ok`.
#[path = "../common.rs"]
mod common;

use std::future::Future;
use std::sync::{Arc, Mutex};

use common::*;
use events_server::bookings::billing_events::{BillingEventHandlers, subscribe_billing_events};
use events_server::bookings::fulfilment_events::{FulfilmentEventHandlers, subscribe_fulfilment_events};
use events_server::bookings::orders_events::create_orders_publisher;
use events_server::bookings::t::{OrderPlaced, OrderShipped};
use events_server::veld::events::{
    BoxFuture, ChannelOptions, ConsumerOptions, Deliver, EventMeta, MemoryInbox, MemoryTransport, Outcome,
    PublisherOptions, Transport, TransportMessage, TransportSubscription,
};
use events_server::veld::{Error, block_on};
use serde_json::Value;

const TOPIC: &str = "shop.orders.v1";
const ORDER: &str = "7d1d6c8e-84c8-4b5b-9d87-0a1a2b3c4d5e";

type Log = Arc<Mutex<Vec<String>>>;
type Reserve = Box<dyn Fn(u32) -> Result<(), Error> + Send + Sync>;

/// `Fulfilment`: `reserve` runs `reserve(run)`, the number of the run counting from 1.
struct Fulfilment {
    runs: Arc<Mutex<u32>>,
    reserve: Reserve,
    log: Log,
}

impl FulfilmentEventHandlers for Fulfilment {
    fn reserve(&self, event: OrderPlaced, _meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        let run = {
            let mut n = self.runs.lock().unwrap();
            *n += 1;
            *n
        };
        self.log.lock().unwrap().push(format!("reserve {} run {run}", event.order_id));
        std::future::ready((self.reserve)(run))
    }

    fn notify_customer(&self, _event: OrderShipped, _meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        std::future::ready(Ok(()))
    }
}

struct Billing(Log);

impl BillingEventHandlers for Billing {
    fn charge(&self, event: OrderPlaced, _meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        self.0.lock().unwrap().push(format!("charge {}", event.order_id));
        std::future::ready(Ok(()))
    }
}

/// A transport that delegates to the memory transport, remembers the options of every call and can
/// refuse to publish to one topic.
struct Spy {
    inner: MemoryTransport,
    published: Mutex<Vec<(String, ChannelOptions)>>,
    subscribed: Mutex<Vec<(String, String, ChannelOptions)>>,
    refuse: Mutex<Option<String>>,
}

impl Spy {
    fn new(inner: MemoryTransport) -> Arc<Spy> {
        Arc::new(Spy { inner, published: Mutex::default(), subscribed: Mutex::default(), refuse: Mutex::default() })
    }
}

impl Transport for Spy {
    fn publish<'a>(&'a self, topic: &'a str, key: Option<&'a str>, bytes: &'a [u8], options: ChannelOptions) -> BoxFuture<'a, Result<(), Error>> {
        if self.refuse.lock().unwrap().as_deref() == Some(topic) {
            return Box::pin(std::future::ready(Err(Error::other("the transport cannot publish"))));
        }
        self.published.lock().unwrap().push((topic.to_string(), options));
        self.inner.publish(topic, key, bytes, options)
    }

    fn subscribe<'a>(&'a self, topic: &'a str, group: &'a str, deliver: Deliver, options: ChannelOptions) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>> {
        self.subscribed.lock().unwrap().push((topic.to_string(), group.to_string(), options));
        self.inner.subscribe(topic, group, deliver, options)
    }
}

fn log() -> Log {
    Arc::new(Mutex::new(Vec::new()))
}

fn placed(quantity: i32) -> OrderPlaced {
    OrderPlaced { order_id: ORDER.to_string(), quantity }
}

fn publisher_options() -> PublisherOptions {
    PublisherOptions::new().new_id(counter()).clock(|| time_from("2026-06-10T12:00:00Z"))
}

/// The messages that arrive on `topic` for the group `listener`.
fn listen(transport: &Arc<Spy>, topic: &str) -> Arc<Mutex<Vec<(Option<String>, Vec<u8>)>>> {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let into = Arc::clone(&heard);
    let deliver: Deliver = Arc::new(move |m: TransportMessage| {
        into.lock().unwrap().push((m.key, m.bytes));
        Box::pin(std::future::ready(Outcome::Done))
    });
    block_on(transport.subscribe(topic, "listener", deliver, ChannelOptions::NONE)).unwrap();
    heard
}

struct World {
    transport: Arc<Spy>,
    runs: Arc<Mutex<u32>>,
    log: Log,
    waits: Arc<Mutex<Vec<u64>>>,
    failures: Arc<Mutex<Vec<(u32, bool)>>>,
    inbox: Arc<MemoryInbox>,
}

fn world(max_attempts: u32, reserve: Reserve) -> World {
    let transport = Spy::new(MemoryTransport::with_max_attempts(max_attempts));
    let (runs, log_, waits, failures) = (Arc::new(Mutex::new(0)), log(), Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())));
    let inbox = Arc::new(MemoryInbox::new());
    let (w, f) = (Arc::clone(&waits), Arc::clone(&failures));
    let options = ConsumerOptions::new()
        .inbox(inbox.clone())
        .sleep(move |ms| {
            w.lock().unwrap().push(ms);
            Box::pin(std::future::ready(()))
        })
        .on_handler_error(move |e| f.lock().unwrap().push((e.attempt, e.is_final)));
    let t: Arc<dyn Transport> = transport.clone();
    block_on(subscribe_fulfilment_events(
        t.clone(),
        Arc::new(Fulfilment { runs: runs.clone(), reserve, log: log_.clone() }),
        options,
    ))
    .unwrap();
    block_on(subscribe_billing_events(t, Arc::new(Billing(log_.clone())), ConsumerOptions::new())).unwrap();
    World { transport, runs, log: log_, waits, failures, inbox }
}

fn publish(w: &World, event: &OrderPlaced) -> events_server::veld::events::PublishedEvent {
    let t: Arc<dyn Transport> = w.transport.clone();
    let publisher = create_orders_publisher(t, publisher_options());
    block_on(publisher.order_events().order_placed(event)).unwrap()
}

fn recovers_after_two_failures() {
    let w = world(3, Box::new(|run| if run < 3 { Err(Error::other("not yet")) } else { Ok(()) }));
    publish(&w, &placed(2));
    block_on(w.transport.inner.idle());
    assert_eq!(*w.runs.lock().unwrap(), 3);
    assert_eq!(*w.waits.lock().unwrap(), vec![1000, 2000], "the waits go through `sleep`");
    assert_eq!(*w.failures.lock().unwrap(), vec![(1, false), (2, false)]);
    assert!(w.transport.inner.dead_letters().is_empty());
    assert!(w.inbox.has("veld:bookings/Orders", "id-1"), "the run that returned is recorded");
    assert_eq!(w.inbox.len(), 1);
}

fn copies_a_used_up_message_and_a_subscriber_reads_it() {
    let w = world(3, Box::new(|_| Err(Error::other("always"))));
    let heard = listen(&w.transport, &format!("{TOPIC}.dlq"));
    let sent = publish(&w, &placed(2));
    block_on(w.transport.inner.idle());
    assert_eq!(*w.runs.lock().unwrap(), 4, "max + 1 runs");
    assert_eq!(*w.waits.lock().unwrap(), vec![1000, 2000, 4000]);
    assert_eq!(*w.failures.lock().unwrap(), vec![(1, false), (2, false), (3, false), (4, true)]);
    assert!(w.transport.inner.dead_letters().is_empty(), "the copy was published, so the answer was done");
    let copies = heard.lock().unwrap().clone();
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].0.as_deref(), Some(ORDER), "the key of the original");
    let copy: Value = serde_json::from_slice(&copies[0].1).unwrap();
    assert_eq!((copy["veldreason"].as_str(), copy["veldattempts"].as_u64(), copy["veldgroup"].as_str()), (Some("handler"), Some(4), Some("fulfilment")));
    assert_eq!(copy["id"], sent.id.as_str());
    assert!(copy.get("veldissues").is_none() && copy.get("veldviolations").is_none());
    // A copy published again to the topic is a valid message: the consumer ignores the members it does not know.
    let before = *w.runs.lock().unwrap();
    let t: Arc<dyn Transport> = w.transport.clone();
    block_on(t.publish(TOPIC, Some(ORDER), &copies[0].1, ChannelOptions::NONE)).unwrap();
    block_on(w.transport.inner.idle());
    assert_eq!(*w.runs.lock().unwrap(), before + 4, "it is a valid message: the handler runs again, and fails again");
    let again = heard.lock().unwrap().clone();
    assert_eq!(again.len(), 2, "and the second copy describes its own failure");
    let second: Value = serde_json::from_slice(&again[1].1).unwrap();
    assert_eq!((second["veldreason"].as_str(), second["veldattempts"].as_u64()), (Some("handler"), Some(4)));
}

fn a_copy_that_cannot_be_published_leaves_the_message_unacknowledged() {
    let w = world(3, Box::new(|_| Err(Error::other("always"))));
    *w.transport.refuse.lock().unwrap() = Some(format!("{TOPIC}.dlq"));
    publish(&w, &placed(2));
    block_on(w.transport.inner.idle());
    // Every delivery of the memory transport ends in `again`: three deliveries, four runs each.
    assert_eq!(*w.runs.lock().unwrap(), 12);
    let dead = w.transport.inner.dead_letters();
    assert!(dead.iter().any(|d| d.group == "fulfilment" && d.attempts == 3), "{dead:?}");
}

fn a_rejected_message_is_acknowledged_and_copied_with_no_attempts() {
    let w = world(3, Box::new(|_| Ok(())));
    let heard = listen(&w.transport, &format!("{TOPIC}.dlq"));
    // The publisher refuses an order that breaks the rule and sends nothing.
    let t: Arc<dyn Transport> = w.transport.clone();
    let publisher = create_orders_publisher(t.clone(), publisher_options());
    assert!(block_on(publisher.order_events().order_placed(&placed(0))).is_err());
    block_on(w.transport.inner.idle());
    assert!(w.log.lock().unwrap().is_empty() && heard.lock().unwrap().is_empty(), "nothing was sent");
    // A message the consumer rejects as `shape` (the publisher would not have sent it).
    let message = br#"{"specversion":"1.0","id":"x-1","source":"veld:bookings/Orders","type":"veld:bookings/OrderPlaced@0fe34399503b2a2d","data":{"orderId":"7d1d6c8e-84c8-4b5b-9d87-0a1a2b3c4d5e","quantity":"many"}}"#;
    block_on(t.publish(TOPIC, Some(ORDER), message, ChannelOptions::NONE)).unwrap();
    block_on(w.transport.inner.idle());
    // `Billing` is on the same channel and rejects the message too: one copy for each group.
    let copies: Vec<Value> = heard.lock().unwrap().iter().map(|c| serde_json::from_slice(&c.1).unwrap()).collect();
    let mut groups: Vec<&str> = copies.iter().map(|c| c["veldgroup"].as_str().unwrap()).collect();
    groups.sort_unstable();
    assert_eq!(groups, ["billing", "fulfilment"], "the shape failure is copied once for each group");
    let copy = copies.iter().find(|c| c["veldgroup"] == "fulfilment").unwrap().clone();
    assert_eq!((copy["veldreason"].as_str(), copy["veldattempts"].as_u64()), (Some("shape"), Some(0)));
    assert!(copy["veldissues"].as_str().unwrap().contains("quantity"));
    assert_eq!(*w.runs.lock().unwrap(), 0, "no handler ran");
    assert!(w.transport.inner.dead_letters().is_empty(), "a rejected message is acknowledged");
}

fn the_inbox_skips_a_second_delivery() {
    let w = world(3, Box::new(|_| Ok(())));
    let t: Arc<dyn Transport> = w.transport.clone();
    let message = br#"{"specversion":"1.0","id":"x-2","source":"veld:bookings/Orders","type":"veld:bookings/OrderPlaced@0fe34399503b2a2d","data":{"orderId":"7d1d6c8e-84c8-4b5b-9d87-0a1a2b3c4d5e","quantity":2}}"#;
    for _ in 0..2 {
        block_on(t.publish(TOPIC, Some(ORDER), message, ChannelOptions::NONE)).unwrap();
    }
    block_on(w.transport.inner.idle());
    assert_eq!(*w.runs.lock().unwrap(), 1, "called once");
    assert_eq!(w.inbox.len(), 1);
    // `Billing` has no inbox: it ran twice, and the consumer with the inbox acknowledged the second one.
    let charges = w.log.lock().unwrap().iter().filter(|l| l.starts_with("charge")).count();
    assert_eq!(charges, 2);
    assert!(w.failures.lock().unwrap().is_empty(), "a skipped message reaches no hook");
}

fn the_transport_gets_the_options_of_the_channel() {
    let w = world(3, Box::new(|_| Ok(())));
    publish(&w, &placed(2));
    block_on(w.transport.inner.idle());
    let on = ChannelOptions { ordered: true, dead_letter: true };
    assert_eq!(w.transport.published.lock().unwrap()[0], (TOPIC.to_string(), on));
    let subscribed = w.transport.subscribed.lock().unwrap().clone();
    assert!(subscribed.len() == 2 && subscribed.iter().all(|(t, _, o)| t == TOPIC && *o == on), "{subscribed:?}");
}

fn main() {
    recovers_after_two_failures();
    copies_a_used_up_message_and_a_subscriber_reads_it();
    a_copy_that_cannot_be_published_leaves_the_message_unacknowledged();
    a_rejected_message_is_acknowledged_and_copied_with_no_attempts();
    the_inbox_skips_a_second_delivery();
    the_transport_gets_the_options_of_the_channel();
    println!("delivery process ok");
}

//! The RabbitMQ adapter against a real broker (`VELD_RABBITMQ`, `tests/toolchain/brokers/README.md`):
//! the rows of `spec/events.md` section 10.1 for RabbitMQ. Names are fresh; the program skips
//! without `VELD_RABBITMQ` (a failure with `VELD_REQUIRE_BROKERS=1`). Prints `rabbitmq ok`.
#[path = "../common.rs"]
mod common;

use std::sync::Arc;

use common::*;
use events_server::adapters::rabbitmq::{RabbitMqTransport, Redelivery};
use events_server::bookings::fulfilment_events::{FulfilmentEventHandlers, subscribe_fulfilment_events};
use events_server::bookings::orders_events::create_orders_publisher;
use events_server::bookings::t::{OrderPlaced, OrderShipped};
use events_server::veld::Error;
use events_server::veld::events::{ChannelOptions, ConsumerOptions, EventMeta, Outcome, PublisherOptions, Transport};
use lapin::options::{BasicGetOptions, QueueDeclareOptions};
use lapin::types::FieldTable;
use lapin::{Connection, ConnectionProperties};
use serde_json::Value;

async fn connect(url: &str) -> Arc<Connection> {
    Arc::new(Connection::connect(url, ConnectionProperties::default()).await.unwrap())
}

fn transport(connection: &Arc<Connection>) -> RabbitMqTransport {
    RabbitMqTransport::with_redelivery(connection.clone(), Redelivery { initial_ms: 100, max_ms: 400 })
}

async fn publish_consume_and_the_key(url: &str) {
    let connection = connect(url).await;
    let t = transport(&connection);
    let (topic, group) = (fresh("veld-rec"), fresh("g"));
    let (deliver, mut rx) = collector(always_done());
    let sub = t.subscribe(&topic, &group, deliver, ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, Some("k1"), b"{\"a\":1}", ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, None, b"{\"a\":2}", ChannelOptions::NONE).await.unwrap();
    let first = next(&mut rx, 30_000).await.expect("the first message");
    assert_eq!((first.key.as_deref(), text(&first), first.attempt), (Some("k1"), "{\"a\":1}".to_string(), 1), "the key is the routing key");
    let second = next(&mut rx, 30_000).await.expect("the second message");
    assert_eq!(second.key, None, "an empty routing key is no key");
    // Order: one consumer, publish order.
    for i in 0..10 {
        t.publish(&topic, Some("same"), i.to_string().as_bytes(), ChannelOptions::NONE).await.unwrap();
    }
    let got: Vec<String> = drain(&mut rx, 2_000).await.iter().map(text).collect();
    assert_eq!(got, (0..10).map(|i| i.to_string()).collect::<Vec<_>>());
    sub.close().await;
    t.close().await;
    connection.close(200, "done".into()).await.unwrap();
}

async fn again_and_before_the_group(url: &str) {
    let connection = connect(url).await;
    let t = transport(&connection);
    let (topic, group) = (fresh("veld-again"), fresh("g"));
    // Nobody has bound a queue: the message is dropped.
    t.publish(&topic, None, b"too early", ChannelOptions::NONE).await.unwrap();
    let script: Script = Arc::new(|m| if m.attempt < 3 { Outcome::Again } else { Outcome::Done });
    let (deliver, mut rx) = collector(script);
    let sub = t.subscribe(&topic, &group, deliver, ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, None, b"first", ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, None, b"second", ChannelOptions::NONE).await.unwrap();
    let seen: Vec<(String, u32)> = drain(&mut rx, 3_000).await.iter().map(|m| (text(m), m.attempt)).collect();
    let want = [("first", 1), ("first", 2), ("first", 3), ("second", 1), ("second", 2), ("second", 3)];
    let want: Vec<(String, u32)> = want.iter().map(|(a, b)| ((*a).to_string(), *b)).collect();
    assert_eq!(seen, want, "no message before the group, and `again` is the same consumer after a delay, prefetch 1");
    sub.close().await;
    t.close().await;
    connection.close(200, "done".into()).await.unwrap();
}

async fn two_consumers_and_ordered(url: &str) {
    let connection = connect(url).await;
    let t = transport(&connection);
    let (topic, group) = (fresh("veld-two"), fresh("g"));
    let (d1, mut rx1) = collector(always_done());
    let (d2, mut rx2) = collector(always_done());
    let s1 = t.subscribe(&topic, &group, d1, ChannelOptions::NONE).await.unwrap();
    let s2 = t.subscribe(&topic, &group, d2, ChannelOptions::NONE).await.unwrap();
    for i in 0..20 {
        t.publish(&topic, None, i.to_string().as_bytes(), ChannelOptions::NONE).await.unwrap();
    }
    let (a, b) = (drain(&mut rx1, 2_000).await, drain(&mut rx2, 2_000).await);
    assert_eq!(a.len() + b.len(), 20, "each message reaches one consumer of the group");
    assert!(!a.is_empty() && !b.is_empty(), "two consumers share the queue: {} and {}", a.len(), b.len());
    s1.close().await;
    s2.close().await;
    // `@ordered`: x-single-active-consumer, so a second consumer is idle while the first is active.
    let (topic, group) = (fresh("veld-ordered"), fresh("g"));
    let ordered = ChannelOptions { ordered: true, dead_letter: false };
    let (d1, mut rx1) = collector(always_done());
    let (d2, mut rx2) = collector(always_done());
    let s1 = t.subscribe(&topic, &group, d1, ordered).await.unwrap();
    let s2 = t.subscribe(&topic, &group, d2, ordered).await.unwrap();
    for i in 0..10 {
        t.publish(&topic, Some(&format!("k{}", i % 3)), i.to_string().as_bytes(), ordered).await.unwrap();
    }
    let (a, b) = (drain(&mut rx1, 2_000).await, drain(&mut rx2, 2_000).await);
    assert!(a.len() == 10 && b.is_empty() || b.len() == 10 && a.is_empty(), "one active consumer takes everything: {} and {}", a.len(), b.len());
    let got: Vec<String> = if a.is_empty() { b } else { a }.iter().map(text).collect();
    assert_eq!(got, (0..10).map(|i| i.to_string()).collect::<Vec<_>>(), "one total order across keys");
    s1.close().await;
    s2.close().await;
    t.close().await;
    connection.close(200, "done".into()).await.unwrap();
}

struct Failing;

impl FulfilmentEventHandlers for Failing {
    fn reserve(&self, _e: OrderPlaced, _m: EventMeta) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        std::future::ready(Err(Error::other("always")))
    }

    fn notify_customer(&self, _e: OrderShipped, _m: EventMeta) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        std::future::ready(Ok(()))
    }
}

/// `@retry`, `@deadLetter` and `@ordered` through the generated consumer: the adapter declares the
/// exchange and the durable queue `<topic>.dlq`, and the copy is kept in it. `VELD_TOPIC` is the
/// topic of the contract the test wrote with a fresh name.
async fn the_dead_letter_queue(url: &str) {
    let connection = connect(url).await;
    let t = Arc::new(transport(&connection));
    let topic = std::env::var("VELD_TOPIC").unwrap_or_else(|_| fresh("veld-dlq"));
    let options = ConsumerOptions::new().retry_delay_ms(20).retry_max_delay_ms(40);
    let handle = subscribe_fulfilment_events(t.clone(), Arc::new(Failing), options).await.unwrap();
    let publisher = create_orders_publisher(t.clone(), PublisherOptions::new());
    publisher.order_events().order_placed(&OrderPlaced { order_id: "7d1d6c8e-84c8-4b5b-9d87-0a1a2b3c4d5e".into(), quantity: 2 }).await.unwrap();
    let channel = connection.create_channel().await.unwrap();
    let dlq = format!("{topic}.dlq");
    let mut copy = None;
    for _ in 0..100 {
        if let Some(m) = channel.basic_get(dlq.as_str().into(), BasicGetOptions::default()).await.unwrap() {
            copy = Some(m.delivery.data.clone());
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let copy: Value = serde_json::from_slice(&copy.expect("the copy in the dead-letter queue")).unwrap();
    assert_eq!((copy["veldreason"].as_str(), copy["veldattempts"].as_u64()), (Some("handler"), Some(4)));
    // The queue of the group was declared durable with `x-single-active-consumer`: declaring it plain is refused.
    let other = connection.create_channel().await.unwrap();
    let again = other
        .queue_declare(format!("{topic}.fulfilment").as_str().into(), QueueDeclareOptions { durable: true, ..QueueDeclareOptions::default() }, FieldTable::default())
        .await;
    assert!(again.is_err(), "the queue exists with other arguments: PRECONDITION_FAILED");
    handle.close().await;
    t.close().await;
    connection.close(200, "done".into()).await.unwrap();
}

async fn close_stops_everything(url: &str) {
    let connection = connect(url).await;
    let t = transport(&connection);
    let (topic, group) = (fresh("veld-close"), fresh("g"));
    let (deliver, mut rx) = collector(always_done());
    let sub = t.subscribe(&topic, &group, deliver, ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, None, b"one", ChannelOptions::NONE).await.unwrap();
    assert!(next(&mut rx, 30_000).await.is_some());
    sub.close().await;
    t.publish(&topic, None, b"two", ChannelOptions::NONE).await.unwrap();
    assert!(next(&mut rx, 1_000).await.is_none(), "a closed subscription gets nothing more");
    t.close().await;
    assert!(t.publish(&topic, None, b"three", ChannelOptions::NONE).await.is_err(), "a closed transport refuses");
    connection.close(200, "done".into()).await.unwrap();
}

#[tokio::main]
async fn main() {
    let Some(url) = broker_or_skip("VELD_RABBITMQ") else { return };
    publish_consume_and_the_key(&url).await;
    again_and_before_the_group(&url).await;
    two_consumers_and_ordered(&url).await;
    the_dead_letter_queue(&url).await;
    close_stops_everything(&url).await;
    println!("rabbitmq ok");
}

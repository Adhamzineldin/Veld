//! The Kafka adapter against a real broker (`VELD_KAFKA`, `tests/toolchain/brokers/README.md`): the
//! rows of `spec/events.md` section 10.1 for Kafka. Topics are created with the admin client under
//! fresh names; the program skips without `VELD_KAFKA` (a failure with `VELD_REQUIRE_BROKERS=1`).
//! Prints `kafka ok`.
#[path = "../common.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use events_server::bookings::fulfilment_events::{FulfilmentEventHandlers, subscribe_fulfilment_events};
use events_server::bookings::orders_events::create_orders_publisher;
use events_server::bookings::t::{OrderPlaced, OrderShipped};
use events_server::adapters::kafka::{KafkaTransport, Redelivery};
use events_server::veld::Error;
use events_server::veld::events::{ChannelOptions, ConsumerOptions, EventMeta, Outcome, PublisherOptions, Transport};
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{Headers, Message};
use rdkafka::ClientConfig;
use serde_json::Value;

fn config(servers: &str) -> ClientConfig {
    let mut c = ClientConfig::new();
    c.set("bootstrap.servers", servers);
    c
}

fn transport(servers: &str) -> KafkaTransport {
    KafkaTransport::new(config(servers)).redelivery(Redelivery { initial_ms: 100, max_ms: 400 })
}

async fn create_topics(servers: &str, topics: &[&str], partitions: i32) {
    let admin: AdminClient<DefaultClientContext> = config(servers).create().unwrap();
    let new: Vec<NewTopic<'_>> = topics.iter().map(|t| NewTopic::new(t, partitions, TopicReplication::Fixed(1))).collect();
    for result in admin.create_topics(&new, &AdminOptions::new()).await.unwrap() {
        result.unwrap_or_else(|(t, e)| panic!("cannot create {t}: {e}"));
    }
}

/// Reads every record of `topic` the way a plain consumer would: partition, key, headers, value.
async fn raw(servers: &str, topic: &str, count: usize) -> Vec<(i32, Option<String>, Option<String>, String)> {
    let consumer: StreamConsumer = config(servers)
        .set("group.id", fresh("raw"))
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .create()
        .unwrap();
    consumer.subscribe(&[topic]).unwrap();
    let mut out = Vec::new();
    while out.len() < count {
        let m = tokio::time::timeout(Duration::from_secs(30), consumer.recv()).await.expect("a record").unwrap();
        let content_type = m.headers().and_then(|h| {
            (0..h.count()).find_map(|i| {
                let header = h.get(i);
                (header.key == "content-type").then(|| header.value.map(|v| String::from_utf8_lossy(v).into_owned())).flatten()
            })
        });
        out.push((m.partition(), m.key().map(|k| String::from_utf8_lossy(k).into_owned()), content_type, String::from_utf8_lossy(m.payload().unwrap_or_default()).into_owned()));
    }
    out
}

async fn publish_consume_and_the_record(servers: &str) {
    let topic = fresh("veld-rec");
    create_topics(servers, &[&topic], 1).await;
    let t = transport(servers);
    let (deliver, mut rx) = collector(always_done());
    let sub = t.subscribe(&topic, &fresh("g"), deliver, ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, Some("k1"), b"{\"a\":1}", ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, None, b"{\"a\":2}", ChannelOptions::NONE).await.unwrap();
    let first = next(&mut rx, 30_000).await.expect("the first message");
    assert_eq!((first.key.as_deref(), text(&first), first.attempt), (Some("k1"), "{\"a\":1}".to_string(), 1));
    let second = next(&mut rx, 30_000).await.expect("the second message");
    assert_eq!(second.key, None);
    let records = raw(servers, &topic, 2).await;
    assert_eq!(records[0].1.as_deref(), Some("k1"), "the key is the record key");
    assert_eq!(records[0].2.as_deref(), Some("application/cloudevents+json; charset=UTF-8"), "the content-type header");
    sub.close().await;
    t.close().await;
}

async fn order_per_key_and_ordered(servers: &str) {
    let topic = fresh("veld-order");
    create_topics(servers, &[&topic], 3).await;
    let t = transport(servers);
    let (deliver, mut rx) = collector(always_done());
    let sub = t.subscribe(&topic, &fresh("g"), deliver, ChannelOptions { ordered: true, dead_letter: false }).await.unwrap();
    let keys = ["a", "b", "c", "a", "b", "c"];
    for round in 0..4 {
        for (i, k) in keys.iter().enumerate() {
            let body = format!("{}", round * 10 + i);
            t.publish(&topic, Some(k), body.as_bytes(), ChannelOptions { ordered: true, dead_letter: false }).await.unwrap();
        }
    }
    let got: Vec<String> = drain(&mut rx, 3_000).await.iter().map(text).collect();
    let want: Vec<String> = (0..4).flat_map(|r| (0..6).map(move |i| format!("{}", r * 10 + i))).collect();
    assert_eq!(got, want, "@ordered: one total order across keys");
    let records = raw(servers, &topic, 24).await;
    assert!(records.iter().all(|r| r.0 == 0), "@ordered puts every message in partition 0");
    sub.close().await;
    // Without @ordered the records of one key stay in one partition, in order.
    let unordered = fresh("veld-keys");
    create_topics(servers, &[&unordered], 3).await;
    let (deliver, mut rx) = collector(always_done());
    let sub = t.subscribe(&unordered, &fresh("g"), deliver, ChannelOptions::NONE).await.unwrap();
    for i in 0..10 {
        t.publish(&unordered, Some("same"), i.to_string().as_bytes(), ChannelOptions::NONE).await.unwrap();
    }
    let got: Vec<String> = drain(&mut rx, 3_000).await.iter().map(text).collect();
    assert_eq!(got, (0..10).map(|i| i.to_string()).collect::<Vec<_>>(), "order per key");
    sub.close().await;
    t.close().await;
}

async fn a_failed_handler_is_delivered_again(servers: &str) {
    let topic = fresh("veld-again");
    create_topics(servers, &[&topic], 1).await;
    let t = transport(servers);
    let script: Script = Arc::new(|m| if m.attempt < 3 { Outcome::Again } else { Outcome::Done });
    let (deliver, mut rx) = collector(script);
    let group = fresh("g");
    let sub = t.subscribe(&topic, &group, deliver, ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, Some("k"), b"first", ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, Some("k"), b"second", ChannelOptions::NONE).await.unwrap();
    let seen: Vec<(String, u32)> = drain(&mut rx, 3_000).await.iter().map(|m| (text(m), m.attempt)).collect();
    let want = [("first", 1), ("first", 2), ("first", 3), ("second", 1), ("second", 2), ("second", 3)];
    let want: Vec<(String, u32)> = want.iter().map(|(a, b)| ((*a).to_string(), *b)).collect();
    assert_eq!(seen, want, "the same consumer delivers it again, and nothing else meanwhile");
    sub.close().await;
    t.close().await;
}

async fn before_the_group_subscribed_and_two_consumers(servers: &str) {
    let topic = fresh("veld-early");
    create_topics(servers, &[&topic], 2).await;
    let t = transport(servers);
    for i in 0..20 {
        t.publish(&topic, Some(&format!("key-{i}")), format!("m{i}").as_bytes(), ChannelOptions::NONE).await.unwrap();
    }
    let group = fresh("g");
    let (d1, mut rx1) = collector(always_done());
    let (d2, mut rx2) = collector(always_done());
    let s1 = t.subscribe(&topic, &group, d1, ChannelOptions::NONE).await.unwrap();
    let s2 = transport(servers).subscribe(&topic, &group, d2, ChannelOptions::NONE).await.unwrap();
    let (a, b) = (drain(&mut rx1, 4_000).await, drain(&mut rx2, 4_000).await);
    assert_eq!(a.len() + b.len(), 20, "a message published before the group subscribed is read, once");
    assert!(!a.is_empty() && !b.is_empty(), "two consumers of one group share the partitions: {} and {}", a.len(), b.len());
    s1.close().await;
    s2.close().await;
    t.close().await;
}

/// A delivery that outlives `max.poll.interval.ms`: the consumer pauses its partitions and keeps
/// polling, so it keeps its place in the group, delivers the message once, and commits it.
async fn a_long_delivery_keeps_the_place_in_the_group(servers: &str) {
    let topic = fresh("veld-long");
    create_topics(servers, &[&topic], 1).await;
    let mut cfg = config(servers);
    cfg.set("max.poll.interval.ms", "6000").set("session.timeout.ms", "6000");
    let t = KafkaTransport::new(cfg.clone()).redelivery(Redelivery { initial_ms: 100, max_ms: 400 });
    let group = fresh("g");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let deliver: events_server::veld::events::Deliver = Arc::new(move |m| {
        let tx = tx.clone();
        Box::pin(async move {
            let _ = tx.send(m);
            tokio::time::sleep(Duration::from_secs(9)).await;
            Outcome::Done
        })
    });
    let sub = t.subscribe(&topic, &group, deliver, ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, None, b"slow", ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, None, b"after", ChannelOptions::NONE).await.unwrap();
    let mut seen = Vec::new();
    while let Some(m) = next(&mut rx, 25_000).await {
        seen.push((text(&m), m.attempt));
        if seen.len() == 2 {
            break;
        }
    }
    assert_eq!(seen, vec![("slow".to_string(), 1), ("after".to_string(), 1)], "each once: the group was not left");
    // Wait for the second delivery to finish and commit, then a new consumer of the group reads nothing.
    tokio::time::sleep(Duration::from_secs(10)).await;
    sub.close().await;
    let (deliver, mut again) = collector(always_done());
    let sub = KafkaTransport::new(cfg).subscribe(&topic, &group, deliver, ChannelOptions::NONE).await.unwrap();
    assert!(next(&mut again, 3_000).await.is_none(), "both records were committed");
    sub.close().await;
    t.close().await;
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

/// `@retry` and `@deadLetter` through the generated consumer: the topic is the one in
/// `VELD_TOPIC` (the test wrote the contract with a fresh name), and so is `<topic>.dlq`.
async fn the_dead_letter_topic(servers: &str) {
    let topic = std::env::var("VELD_TOPIC").unwrap_or_else(|_| fresh("veld-dlq"));
    let dlq = format!("{topic}.dlq");
    create_topics(servers, &[&topic, &dlq], 1).await;
    let t = Arc::new(transport(servers));
    let options = ConsumerOptions::new().retry_delay_ms(20).retry_max_delay_ms(40);
    let handle = subscribe_fulfilment_events(t.clone(), Arc::new(Failing), options).await.unwrap();
    let (deliver, mut rx) = collector(always_done());
    let listener = t.subscribe(&dlq, &fresh("g"), deliver, ChannelOptions::NONE).await.unwrap();
    let publisher = create_orders_publisher(t.clone(), PublisherOptions::new());
    publisher.order_events().order_placed(&OrderPlaced { order_id: "7d1d6c8e-84c8-4b5b-9d87-0a1a2b3c4d5e".into(), quantity: 2 }).await.unwrap();
    let copy = next(&mut rx, 30_000).await.expect("the copy on the dead-letter topic");
    let copy: Value = serde_json::from_slice(&copy.bytes).unwrap();
    assert_eq!((copy["veldreason"].as_str(), copy["veldattempts"].as_u64()), (Some("handler"), Some(4)));
    handle.close().await;
    listener.close().await;
    t.close().await;
}

async fn close_stops_everything(servers: &str) {
    let topic = fresh("veld-close");
    create_topics(servers, &[&topic], 1).await;
    let t = transport(servers);
    let (deliver, mut rx) = collector(always_done());
    let sub = t.subscribe(&topic, &fresh("g"), deliver, ChannelOptions::NONE).await.unwrap();
    t.publish(&topic, None, b"one", ChannelOptions::NONE).await.unwrap();
    assert!(next(&mut rx, 30_000).await.is_some());
    sub.close().await;
    t.publish(&topic, None, b"two", ChannelOptions::NONE).await.unwrap();
    assert!(next(&mut rx, 2_000).await.is_none(), "a closed subscription gets nothing more");
    t.close().await;
    assert!(t.publish(&topic, None, b"three", ChannelOptions::NONE).await.is_err(), "a closed transport refuses");
}

#[tokio::main]
async fn main() {
    let Some(servers) = broker_or_skip("VELD_KAFKA") else { return };
    publish_consume_and_the_record(&servers).await;
    order_per_key_and_ordered(&servers).await;
    a_failed_handler_is_delivered_again(&servers).await;
    before_the_group_subscribed_and_two_consumers(&servers).await;
    a_long_delivery_keeps_the_place_in_the_group(&servers).await;
    the_dead_letter_topic(&servers).await;
    close_stops_everything(&servers).await;
    println!("kafka ok");
}

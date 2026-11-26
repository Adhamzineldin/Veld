//! The Kafka transport (`spec/events.md` section 10.7). This is the only module written against
//! `rdkafka`; the application builds the `ClientConfig` and owns its addresses, TLS and credentials.
//!
//! What the adapter adds to the application's configuration: the consumer group (the `group` of the
//! subscription), manual commits after the handler, `auto.offset.reset=earliest` for a group that
//! has no offset, an idempotent producer with `acks=all` and one request in flight, and no
//! automatic creation of topics. A setting of the application that contradicts these is
//! overridden.
//!
//! The adapter needs a tokio runtime: it spawns one task per subscription.

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rdkafka::consumer::{BaseConsumer, CommitMode, Consumer, ConsumerContext, Rebalance, StreamConsumer};
use rdkafka::message::{Header, Message, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use rdkafka::{ClientConfig, ClientContext, Offset};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use crate::veld::Error;
use crate::veld::events::{
    BoxFuture, ChannelOptions, Deliver, Outcome, Transport, TransportMessage, TransportSubscription,
};

/// The media type of the structured CloudEvents JSON mode (`spec/events.md` section 6.2).
const CONTENT_TYPE: &str = "application/cloudevents+json; charset=UTF-8";

/// How long `subscribe` waits for the group to assign partitions.
const JOIN_TIMEOUT: Duration = Duration::from_secs(60);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn failure(what: &str, e: impl std::fmt::Display) -> Error {
    Error::other(format!("veld kafka: {what}: {e}"))
}

/// The delay before a message answered `Again` is delivered again: it starts at `initial_ms` and
/// doubles up to `max_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Redelivery {
    /// Default 1000.
    pub initial_ms: u64,
    /// Default 30000.
    pub max_ms: u64,
}

impl Default for Redelivery {
    fn default() -> Self {
        Redelivery { initial_ms: 1000, max_ms: 30_000 }
    }
}

/// Tells `subscribe` that the group assigned partitions.
struct Joined {
    sender: Mutex<Option<oneshot::Sender<()>>>,
}

impl ClientContext for Joined {}

impl ConsumerContext for Joined {
    fn post_rebalance(&self, _consumer: &BaseConsumer<Self>, rebalance: &Rebalance<'_>) {
        if matches!(rebalance, Rebalance::Assign(_)) {
            if let Some(sender) = lock(&self.sender).take() {
                let _ = sender.send(());
            }
        }
    }
}

type Consuming = StreamConsumer<Joined>;

/// One running subscription: the task that polls, and the signal that stops it.
struct Running {
    stop: watch::Sender<bool>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl Running {
    async fn stop(&self) {
        let _ = self.stop.send(true);
        let task = lock(&self.task).take();
        if let Some(task) = task {
            let _ = task.await;
        }
    }
}

struct Inner {
    config: ClientConfig,
    redelivery: Redelivery,
    producer: Mutex<Option<FutureProducer>>,
    running: Mutex<Vec<Arc<Running>>>,
    closed: Mutex<bool>,
}

/// `Transport` over Kafka. Topics are not created. A message is the envelope as the record value
/// with the header `content-type`, and the key text as the record key. `@ordered` puts every
/// message in partition 0. `publish` completes when all in-sync replicas have the record. A group
/// that has no offset starts at the earliest. A message answered `Again` is not committed and is
/// delivered again by the same consumer after a delay, while nothing else is delivered. `attempt`
/// counts the deliveries by this consumer since it started: it starts at 1 again after a restart or
/// a rebalance.
///
/// While a delivery runs or waits, the partitions are paused and the consumer is still polled, so
/// a handler that runs for long does not cost it its place in the group (`max.poll.interval.ms`).
#[derive(Clone)]
pub struct KafkaTransport {
    inner: Arc<Inner>,
}

impl KafkaTransport {
    /// A transport built from the client configuration of the application.
    #[must_use]
    pub fn new(config: ClientConfig) -> Self {
        KafkaTransport {
            inner: Arc::new(Inner {
                config,
                redelivery: Redelivery::default(),
                producer: Mutex::new(None),
                running: Mutex::new(Vec::new()),
                closed: Mutex::new(false),
            }),
        }
    }

    /// The delay before a message answered `Again` is delivered again.
    #[must_use]
    pub fn redelivery(self, redelivery: Redelivery) -> Self {
        let inner = &self.inner;
        KafkaTransport {
            inner: Arc::new(Inner {
                config: inner.config.clone(),
                redelivery,
                producer: Mutex::new(None),
                running: Mutex::new(Vec::new()),
                closed: Mutex::new(false),
            }),
        }
    }

    fn check_open(&self) -> Result<(), Error> {
        if *lock(&self.inner.closed) {
            Err(Error::other("veld kafka: the transport is closed"))
        } else {
            Ok(())
        }
    }

    /// The producer, created on the first publish.
    fn producer(&self) -> Result<FutureProducer, Error> {
        let mut slot = lock(&self.inner.producer);
        if let Some(p) = slot.as_ref() {
            return Ok(p.clone());
        }
        let mut config = self.inner.config.clone();
        config
            .set("enable.idempotence", "true")
            .set("acks", "all")
            .set("max.in.flight.requests.per.connection", "1")
            .set("allow.auto.create.topics", "false");
        let producer: FutureProducer = config.create().map_err(|e| failure("cannot create the producer", e))?;
        *slot = Some(producer.clone());
        Ok(producer)
    }

    async fn send(&self, topic: &str, key: Option<&str>, bytes: &[u8], options: ChannelOptions) -> Result<(), Error> {
        self.check_open()?;
        let producer = self.producer()?;
        let headers = OwnedHeaders::new().insert(Header { key: "content-type", value: Some(CONTENT_TYPE) });
        let mut record: FutureRecord<'_, str, [u8]> = FutureRecord::to(topic).payload(bytes).headers(headers);
        if let Some(key) = key {
            record = record.key(key);
        }
        if options.ordered {
            record = record.partition(0);
        }
        producer
            .send(record, Timeout::After(Duration::from_secs(30)))
            .await
            .map(|_| ())
            .map_err(|(e, _)| failure("cannot publish", e))
    }

    async fn join(&self, topic: &str, group: &str, deliver: Deliver) -> Result<Box<dyn TransportSubscription>, Error> {
        self.check_open()?;
        let (joined, wait) = oneshot::channel();
        let context = Joined { sender: Mutex::new(Some(joined)) };
        let mut config = self.inner.config.clone();
        config
            .set("group.id", group)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .set("allow.auto.create.topics", "false")
            .set("enable.partition.eof", "false");
        let consumer: Consuming = config
            .create_with_context(context)
            .map_err(|e| failure("cannot create the consumer", e))?;
        consumer.subscribe(&[topic]).map_err(|e| failure("cannot subscribe", e))?;
        let (stop, stopped) = watch::channel(false);
        let redelivery = self.inner.redelivery;
        let task = tokio::spawn(poll_loop(consumer, deliver, redelivery, stopped));
        let running = Arc::new(Running { stop, task: Mutex::new(Some(task)) });
        // The task polls, and the group assigns during the polls.
        if tokio::time::timeout(JOIN_TIMEOUT, wait).await.is_err() {
            running.stop().await;
            return Err(Error::other("veld kafka: the consumer did not join the group in time"));
        }
        lock(&self.inner.running).push(Arc::clone(&running));
        Ok(Box::new(Subscription { running, owner: Arc::clone(&self.inner) }))
    }

    /// Stops every consumer, waits for the deliveries that run, and flushes the producer. The
    /// application closes its own client. Every later `publish` or `subscribe` fails.
    pub async fn close(&self) {
        *lock(&self.inner.closed) = true;
        let running: Vec<Arc<Running>> = std::mem::take(&mut *lock(&self.inner.running));
        for r in running {
            r.stop().await;
        }
        let producer = lock(&self.inner.producer).take();
        if let Some(producer) = producer {
            let _ = tokio::task::spawn_blocking(move || producer.flush(Duration::from_secs(10))).await;
        }
    }
}

impl Transport for KafkaTransport {
    fn publish<'a>(
        &'a self,
        topic: &'a str,
        key: Option<&'a str>,
        bytes: &'a [u8],
        options: ChannelOptions,
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(self.send(topic, key, bytes, options))
    }

    fn subscribe<'a>(
        &'a self,
        topic: &'a str,
        group: &'a str,
        deliver: Deliver,
        _options: ChannelOptions,
    ) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>> {
        // `@ordered` needs nothing here: a consumer takes one message at a time, and the publisher
        // put every ordered message in partition 0.
        Box::pin(self.join(topic, group, deliver))
    }
}

struct Subscription {
    running: Arc<Running>,
    owner: Arc<Inner>,
}

impl TransportSubscription for Subscription {
    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            lock(&self.owner.running).retain(|r| !Arc::ptr_eq(r, &self.running));
            self.running.stop().await;
        })
    }
}

/// What a record is worth to the loop.
struct Record {
    key: Option<String>,
    bytes: Vec<u8>,
}

/// The loop of one subscription: take a record, deliver it until `Done`, commit, go on.
async fn poll_loop(consumer: Consuming, deliver: Deliver, redelivery: Redelivery, mut stopped: watch::Receiver<bool>) {
    loop {
        let received = tokio::select! {
            () = wait_stopped(&mut stopped) => return,
            m = consumer.recv() => m,
        };
        let message = match received {
            Ok(m) => m,
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let record = Record {
            key: message.key().map(|k| String::from_utf8_lossy(k).into_owned()),
            bytes: message.payload().map(<[u8]>::to_vec).unwrap_or_default(),
        };
        pause_all(&consumer);
        let finished = deliver_until_done(&consumer, &deliver, &record, redelivery, &mut stopped).await;
        if finished {
            // After a rebalance the commit is refused and the new owner delivers the record again.
            let _ = consumer.commit_message(&message, CommitMode::Sync);
        }
        resume_all(&consumer);
        if !finished {
            return;
        }
    }
}

async fn wait_stopped(stopped: &mut watch::Receiver<bool>) {
    while !*stopped.borrow() {
        if stopped.changed().await.is_err() {
            return;
        }
    }
}

fn pause_all(consumer: &Consuming) {
    if let Ok(assigned) = consumer.assignment() {
        let _ = consumer.pause(&assigned);
    }
}

fn resume_all(consumer: &Consuming) {
    if let Ok(assigned) = consumer.assignment() {
        let _ = consumer.resume(&assigned);
    }
}

/// Polls the consumer while `work` runs. The partitions are paused, so nothing is expected; a
/// record that was fetched before the pause took effect (or from a partition a rebalance assigned
/// meanwhile) is given back with a seek to its offset and the partitions are paused again. The
/// pending `recv()` is polled again at least every half of `max.poll.interval.ms`: `StreamConsumer`
/// sends itself that wake-up (`rdkafka` 0.38 `stream_consumer.rs`), which is the poll that
/// librdkafka's liveness check counts.
async fn polled<T>(consumer: &Consuming, work: impl Future<Output = T>) -> T {
    tokio::pin!(work);
    loop {
        tokio::select! {
            out = &mut work => return out,
            stray = consumer.recv() => {
                if let Ok(m) = stray {
                    let _ = consumer.seek(m.topic(), m.partition(), Offset::Offset(m.offset()), Duration::from_secs(1));
                    // A rebalance may have assigned partitions that are not paused yet.
                    pause_all(consumer);
                } else {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    }
}

/// Delivers one record, and again after a delay until `Done`. `false`: the subscription was closed
/// first, and nothing is committed.
async fn deliver_until_done(
    consumer: &Consuming,
    deliver: &Deliver,
    record: &Record,
    redelivery: Redelivery,
    stopped: &mut watch::Receiver<bool>,
) -> bool {
    let mut wait = redelivery.initial_ms;
    let mut attempt = 1u32;
    loop {
        let message = TransportMessage { key: record.key.clone(), bytes: record.bytes.clone(), attempt };
        if polled(consumer, deliver(message)).await == Outcome::Done {
            return true;
        }
        if *stopped.borrow() {
            return false;
        }
        let slept = polled(consumer, async {
            tokio::select! {
                () = tokio::time::sleep(Duration::from_millis(wait)) => true,
                () = wait_stopped(stopped) => false,
            }
        })
        .await;
        if !slept {
            return false;
        }
        wait = wait.saturating_mul(2).min(redelivery.max_ms);
        attempt = attempt.saturating_add(1);
    }
}

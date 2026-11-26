//! The RabbitMQ transport (`spec/events.md` section 10.7), over AMQP 0-9-1. This is the only module
//! written against `lapin`. The application opens the connection and owns its address, TLS and
//! credentials (the TLS features of `lapin` are the application's to enable in its own manifest).
//!
//! The adapter needs a tokio runtime: it spawns one task per subscription.

use std::collections::BTreeSet;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use futures_core::Stream;
use lapin::options::{
    BasicAckOptions, BasicCancelOptions, BasicConsumeOptions, BasicPublishOptions, BasicQosOptions,
    ConfirmSelectOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
};
use lapin::types::{AMQPValue, FieldTable};
use lapin::{BasicProperties, Channel, Confirmation, Connection, Consumer, ExchangeKind};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::veld::Error;
use crate::veld::events::{
    BoxFuture, ChannelOptions, Deliver, Outcome, Transport, TransportMessage, TransportSubscription,
};

/// The media type of the structured CloudEvents JSON mode (`spec/events.md` section 6.2).
const CONTENT_TYPE: &str = "application/cloudevents+json; charset=UTF-8";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn failure(what: &str, e: impl std::fmt::Display) -> Error {
    Error::other(format!("veld rabbitmq: {what}: {e}"))
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

/// One running subscription: the task that consumes, and the signal that stops it.
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

/// How long `publish` waits for the broker's confirm by default, in milliseconds.
const PUBLISH_TIMEOUT_MS: u64 = 30_000;

struct Inner {
    connection: Arc<Connection>,
    redelivery: Redelivery,
    publish_timeout_ms: u64,
    publishing: tokio::sync::Mutex<Option<Channel>>,
    declared: Mutex<BTreeSet<String>>,
    running: Mutex<Vec<Arc<Running>>>,
    closed: Mutex<bool>,
}

/// `Transport` over RabbitMQ. A topic is a durable `fanout` exchange; a group is the durable queue
/// `<topic>.<group>`, bound to it, whose consumers share it; the key is the routing key (empty
/// when there is none). `publish` completes when the broker confirmed the message, which is
/// persistent. `@ordered` declares the queue with `x-single-active-consumer`, so the order holds
/// while one consumer is active. With `@deadLetter` the consumer also declares the durable
/// exchange `<topic>.dlq` and a durable queue of the same name, so a copy is kept. A message
/// answered `Again` stays unacknowledged and is delivered again by the same consumer after a delay
/// (prefetch 1, so nothing else is delivered meanwhile); a broker closes a channel whose delivery
/// is not acknowledged within its `consumer_timeout` (30 minutes by default). A queue that exists
/// with other arguments is the broker's `PRECONDITION_FAILED`, passed on.
#[derive(Clone)]
pub struct RabbitMqTransport {
    inner: Arc<Inner>,
}

impl RabbitMqTransport {
    /// A transport on an open connection of the application.
    #[must_use]
    pub fn new(connection: Arc<Connection>) -> Self {
        Self::with_redelivery(connection, Redelivery::default())
    }

    /// A transport that delivers a message answered `Again` again after `redelivery`.
    #[must_use]
    pub fn with_redelivery(connection: Arc<Connection>, redelivery: Redelivery) -> Self {
        RabbitMqTransport {
            inner: Arc::new(Inner {
                connection,
                redelivery,
                publish_timeout_ms: PUBLISH_TIMEOUT_MS,
                publishing: tokio::sync::Mutex::new(None),
                declared: Mutex::new(BTreeSet::new()),
                running: Mutex::new(Vec::new()),
                closed: Mutex::new(false),
            }),
        }
    }

    /// How long `publish` waits for the broker's confirm before it fails (default 30000). It is an
    /// option of the adapter, so call it before the transport is used.
    #[must_use]
    pub fn publish_timeout_ms(self, ms: u64) -> Self {
        let inner = &self.inner;
        RabbitMqTransport {
            inner: Arc::new(Inner {
                connection: Arc::clone(&inner.connection),
                redelivery: inner.redelivery,
                publish_timeout_ms: ms,
                publishing: tokio::sync::Mutex::new(None),
                declared: Mutex::new(BTreeSet::new()),
                running: Mutex::new(Vec::new()),
                closed: Mutex::new(false),
            }),
        }
    }

    fn check_open(&self) -> Result<(), Error> {
        if *lock(&self.inner.closed) {
            Err(Error::other("veld rabbitmq: the transport is closed"))
        } else {
            Ok(())
        }
    }

    /// Declares the exchange of a topic once. It is done on a channel of its own: a refused
    /// declaration closes the channel, and that must not take the publisher's with it.
    async fn declare_exchange(&self, topic: &str) -> Result<(), Error> {
        if lock(&self.inner.declared).contains(topic) {
            return Ok(());
        }
        let channel = self.inner.connection.create_channel().await.map_err(|e| failure("cannot open a channel", e))?;
        let declared = exchange(&channel, topic).await;
        let _ = channel.close(200, "done".into()).await;
        declared?;
        lock(&self.inner.declared).insert(topic.to_string());
        Ok(())
    }

    /// The channel of the publisher, with confirms. A channel the broker closed is made again.
    async fn publish_channel(&self) -> Result<Channel, Error> {
        let mut slot = self.inner.publishing.lock().await;
        if let Some(channel) = slot.as_ref().filter(|c| c.status().connected()) {
            return Ok(channel.clone());
        }
        let channel = self.inner.connection.create_channel().await.map_err(|e| failure("cannot open a channel", e))?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .map_err(|e| failure("cannot enable publisher confirms", e))?;
        *slot = Some(channel.clone());
        Ok(channel)
    }

    async fn send(&self, topic: &str, key: Option<&str>, bytes: &[u8]) -> Result<(), Error> {
        self.check_open()?;
        self.declare_exchange(topic).await?;
        let channel = self.publish_channel().await?;
        let properties = BasicProperties::default().with_content_type(CONTENT_TYPE.into()).with_delivery_mode(2);
        let wait = Duration::from_millis(self.inner.publish_timeout_ms);
        let publish = async {
            channel
                .basic_publish(topic.into(), key.unwrap_or("").into(), BasicPublishOptions::default(), bytes, properties)
                .await
                .map_err(|e| failure("cannot publish", e))?
                .await
                .map_err(|e| failure("the broker did not confirm", e))
        };
        let confirm = match tokio::time::timeout(wait, publish).await {
            Ok(result) => result?,
            Err(_) => {
                return Err(Error::other(format!(
                    "veld rabbitmq: the broker did not confirm within {} ms",
                    self.inner.publish_timeout_ms
                )));
            }
        };
        match confirm {
            Confirmation::Ack(_) => Ok(()),
            Confirmation::Nack(_) => Err(Error::other("veld rabbitmq: the broker refused the message")),
            Confirmation::NotRequested => Err(Error::other("veld rabbitmq: publisher confirms are off")),
        }
    }

    async fn join(&self, topic: &str, group: &str, deliver: Deliver, options: ChannelOptions) -> Result<Box<dyn TransportSubscription>, Error> {
        self.check_open()?;
        let channel = self.inner.connection.create_channel().await.map_err(|e| failure("cannot open a channel", e))?;
        let queue = format!("{topic}.{group}");
        let consumer = match start(&channel, topic, &queue, options).await {
            Ok(consumer) => consumer,
            Err(e) => {
                let _ = channel.close(200, "done".into()).await;
                return Err(e);
            }
        };
        let (stop, stopped) = watch::channel(false);
        let task = tokio::spawn(consume_loop(channel, consumer, deliver, self.inner.redelivery, stopped));
        let running = Arc::new(Running { stop, task: Mutex::new(Some(task)) });
        lock(&self.inner.running).push(Arc::clone(&running));
        Ok(Box::new(Subscription { running, owner: Arc::clone(&self.inner) }))
    }

    /// Stops every consumer, waits for the deliveries that run, and closes the channel of the
    /// publisher. The application closes its connection. Every later `publish` or `subscribe`
    /// fails.
    pub async fn close(&self) {
        *lock(&self.inner.closed) = true;
        let running: Vec<Arc<Running>> = std::mem::take(&mut *lock(&self.inner.running));
        for r in running {
            r.stop().await;
        }
        if let Some(channel) = self.inner.publishing.lock().await.take() {
            let _ = channel.close(200, "done".into()).await;
        }
    }
}

impl Transport for RabbitMqTransport {
    fn publish<'a>(
        &'a self,
        topic: &'a str,
        key: Option<&'a str>,
        bytes: &'a [u8],
        _options: ChannelOptions,
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(self.send(topic, key, bytes))
    }

    fn subscribe<'a>(
        &'a self,
        topic: &'a str,
        group: &'a str,
        deliver: Deliver,
        options: ChannelOptions,
    ) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>> {
        Box::pin(self.join(topic, group, deliver, options))
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

async fn exchange(channel: &Channel, name: &str) -> Result<(), Error> {
    let options = ExchangeDeclareOptions { durable: true, ..ExchangeDeclareOptions::default() };
    channel
        .exchange_declare(name.into(), ExchangeKind::Fanout, options, FieldTable::default())
        .await
        .map_err(|e| failure("cannot declare an exchange", e))
}

async fn declare_queue(channel: &Channel, queue: &str, arguments: FieldTable) -> Result<(), Error> {
    let options = QueueDeclareOptions { durable: true, ..QueueDeclareOptions::default() };
    channel
        .queue_declare(queue.into(), options, arguments)
        .await
        .map(|_| ())
        .map_err(|e| failure("cannot declare a queue", e))
}

/// The exchange, the queue and the binding of a group, and the dead-letter pair when the channel
/// has `@deadLetter`; then the consumer, with prefetch 1.
async fn start(channel: &Channel, topic: &str, queue: &str, options: ChannelOptions) -> Result<Consumer, Error> {
    exchange(channel, topic).await?;
    let mut arguments = FieldTable::default();
    if options.ordered {
        arguments.insert("x-single-active-consumer".into(), AMQPValue::Boolean(true));
    }
    declare_queue(channel, queue, arguments).await?;
    channel
        .queue_bind(queue.into(), topic.into(), "".into(), QueueBindOptions::default(), FieldTable::default())
        .await
        .map_err(|e| failure("cannot bind a queue", e))?;
    if options.dead_letter {
        let dead = format!("{topic}.dlq");
        exchange(channel, &dead).await?;
        declare_queue(channel, &dead, FieldTable::default()).await?;
        channel
            .queue_bind(dead.as_str().into(), dead.as_str().into(), "".into(), QueueBindOptions::default(), FieldTable::default())
            .await
            .map_err(|e| failure("cannot bind a queue", e))?;
    }
    channel.basic_qos(1, BasicQosOptions::default()).await.map_err(|e| failure("cannot set the prefetch", e))?;
    channel
        .basic_consume(queue.into(), "".into(), BasicConsumeOptions::default(), FieldTable::default())
        .await
        .map_err(|e| failure("cannot consume", e))
}

async fn wait_stopped(stopped: &mut watch::Receiver<bool>) {
    while !*stopped.borrow() {
        if stopped.changed().await.is_err() {
            return;
        }
    }
}

/// The next delivery of the consumer, `None` when it ended.
async fn next(consumer: &mut Consumer) -> Option<lapin::Result<lapin::message::Delivery>> {
    std::future::poll_fn(|cx| Pin::new(&mut *consumer).poll_next(cx)).await
}

/// The loop of one subscription: take a delivery, deliver it until `Done`, acknowledge, go on.
async fn consume_loop(
    channel: Channel,
    mut consumer: Consumer,
    deliver: Deliver,
    redelivery: Redelivery,
    mut stopped: watch::Receiver<bool>,
) {
    let tag = consumer.tag();
    loop {
        let delivery = tokio::select! {
            () = wait_stopped(&mut stopped) => break,
            d = next(&mut consumer) => d,
        };
        let Some(Ok(delivery)) = delivery else { break };
        let key = Some(delivery.routing_key.as_str()).filter(|k| !k.is_empty()).map(str::to_string);
        if deliver_until_done(&deliver, key, &delivery.data, redelivery, &mut stopped).await {
            // The channel is gone: the broker delivers the message again.
            let _ = delivery.ack(BasicAckOptions::default()).await;
        } else {
            break;
        }
    }
    let _ = channel.basic_cancel(tag, BasicCancelOptions::default()).await;
    let _ = channel.close(200, "done".into()).await;
}

/// Delivers one message, and again after a delay until `Done`. `false`: the subscription was closed
/// first, and the message stays unacknowledged for the next consumer.
async fn deliver_until_done(
    deliver: &Deliver,
    key: Option<String>,
    bytes: &[u8],
    redelivery: Redelivery,
    stopped: &mut watch::Receiver<bool>,
) -> bool {
    let mut wait = redelivery.initial_ms;
    let mut attempt = 1u32;
    loop {
        let message = TransportMessage { key: key.clone(), bytes: bytes.to_vec(), attempt };
        if deliver(message).await == Outcome::Done {
            return true;
        }
        let slept = tokio::select! {
            () = tokio::time::sleep(Duration::from_millis(wait)) => true,
            () = wait_stopped(stopped) => false,
        };
        if !slept {
            return false;
        }
        wait = wait.saturating_mul(2).min(redelivery.max_ms);
        attempt = attempt.saturating_add(1);
    }
}

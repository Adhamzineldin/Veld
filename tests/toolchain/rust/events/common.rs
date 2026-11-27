//! What the drivers of the events tests share (`crates/emitters/rust/tests/events.rs`): a transport
//! that records what is published, a fixed clock and hex. It is included with `#[path]` by every
//! driver of a workspace whose generated crate is `events-server`.
#![allow(dead_code)]

use std::future::ready;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use events_server::veld::Error;
use events_server::veld::events::{BoxFuture, ChannelOptions, Deliver, Transport, TransportSubscription};

/// One message a [`Recorder`] was given: the topic, the key and the bytes.
pub type Sent = (String, Option<String>, Vec<u8>);

/// A subscription a [`Recorder`] was given: the topic, the group, the options of the channel and
/// the function that delivers a message to the consumer.
pub type Subscribed = (String, String, ChannelOptions, Deliver);

/// A transport that keeps what is published and what is subscribed, and delivers nothing by
/// itself: a test calls the `deliver` function of a subscription. `fail_topic` makes `publish` to
/// that topic fail.
#[derive(Default)]
pub struct Recorder {
    pub sent: Mutex<Vec<Sent>>,
    pub options: Mutex<Vec<ChannelOptions>>,
    pub subscribed: Mutex<Vec<Subscribed>>,
    pub fail_topic: Mutex<Option<String>>,
}

impl Recorder {
    pub fn new() -> Arc<Recorder> {
        Arc::new(Recorder::default())
    }

    pub fn messages(&self) -> Vec<Sent> {
        self.sent.lock().unwrap().clone()
    }

    /// The `deliver` function of the first subscription.
    pub fn deliver(&self) -> Deliver {
        Arc::clone(&self.subscribed.lock().unwrap()[0].3)
    }

    /// From now on `publish` to `topic` fails.
    pub fn fail(&self, topic: Option<&str>) {
        *self.fail_topic.lock().unwrap() = topic.map(str::to_string);
    }
}

struct Nop;

impl TransportSubscription for Nop {
    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(ready(()))
    }
}

impl Transport for Recorder {
    fn publish<'a>(
        &'a self,
        topic: &'a str,
        key: Option<&'a str>,
        bytes: &'a [u8],
        options: ChannelOptions,
    ) -> BoxFuture<'a, Result<(), Error>> {
        if self.fail_topic.lock().unwrap().as_deref() == Some(topic) {
            return Box::pin(ready(Err(Error::other("the transport cannot publish"))));
        }
        self.sent.lock().unwrap().push((topic.to_string(), key.map(str::to_string), bytes.to_vec()));
        self.options.lock().unwrap().push(options);
        Box::pin(ready(Ok(())))
    }

    fn subscribe<'a>(
        &'a self,
        topic: &'a str,
        group: &'a str,
        deliver: Deliver,
        options: ChannelOptions,
    ) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>> {
        self.subscribed.lock().unwrap().push((topic.to_string(), group.to_string(), options, deliver));
        Box::pin(ready(Ok(Box::new(Nop) as Box<dyn TransportSubscription>)))
    }
}

/// Lowercase hex of some bytes.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Days since 1970-01-01 of a civil date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The instant of `2026-06-10T12:00:02.007Z` (UTC, `Z`, an optional fraction of 3 digits).
pub fn time_from(text: &str) -> SystemTime {
    let n = |from: usize, to: usize| text[from..to].parse::<i64>().unwrap();
    let days = days_from_civil(n(0, 4), n(5, 7), n(8, 10));
    let seconds = days * 86_400 + n(11, 13) * 3600 + n(14, 16) * 60 + n(17, 19);
    let millis = if text.as_bytes().get(19) == Some(&b'.') { n(20, 23) } else { 0 };
    UNIX_EPOCH + Duration::from_millis(u64::try_from(seconds * 1000 + millis).unwrap())
}

/// A counter that gives `id-1`, `id-2`, ...
pub fn counter() -> impl Fn() -> String + Send + Sync + 'static {
    let n = std::sync::atomic::AtomicU64::new(0);
    move || format!("id-{}", n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1)
}

/// A future that is pending once: a handler that is slow enough for the order to matter.
pub async fn yield_now() {
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if yielded {
            std::task::Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
    .await;
}

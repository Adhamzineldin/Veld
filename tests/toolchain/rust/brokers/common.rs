//! What the broker programs share: the skip rule, fresh names, a collecting `deliver`.
#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use events_server::veld::events::{Deliver, Outcome, TransportMessage};
use tokio::sync::mpsc;

/// The value of the variable `name`, or `None` when the program must skip: with
/// `VELD_REQUIRE_BROKERS=1` a missing variable is a failure.
pub fn broker_or_skip(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => {
            assert!(
                std::env::var("VELD_REQUIRE_BROKERS").as_deref() != Ok("1"),
                "{name} is not set and VELD_REQUIRE_BROKERS=1"
            );
            eprintln!("SKIPPED: {name} is not set");
            None
        }
    }
}

/// A name nobody used before: the prefix, the process and the clock.
pub fn fresh(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{prefix}-{}-{nanos}", std::process::id())
}

pub type Script = Arc<dyn Fn(&TransportMessage) -> Outcome + Send + Sync>;

/// A `deliver` that sends every message it is given to the channel and answers as `script` says.
pub fn collector(script: Script) -> (Deliver, mpsc::UnboundedReceiver<TransportMessage>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let deliver: Deliver = Arc::new(move |message| {
        let answer = script(&message);
        let _ = tx.send(message);
        Box::pin(std::future::ready(answer))
    });
    (deliver, rx)
}

pub fn always_done() -> Script {
    Arc::new(|_| Outcome::Done)
}

/// The next message within `ms`, or `None`.
pub async fn next(rx: &mut mpsc::UnboundedReceiver<TransportMessage>, ms: u64) -> Option<TransportMessage> {
    tokio::time::timeout(Duration::from_millis(ms), rx.recv()).await.ok().flatten()
}

/// Everything that arrives within `ms` of silence.
pub async fn drain(rx: &mut mpsc::UnboundedReceiver<TransportMessage>, ms: u64) -> Vec<TransportMessage> {
    let mut out = Vec::new();
    while let Some(m) = next(rx, ms).await {
        out.push(m);
    }
    out
}

pub fn text(m: &TransportMessage) -> String {
    String::from_utf8_lossy(&m.bytes).into_owned()
}

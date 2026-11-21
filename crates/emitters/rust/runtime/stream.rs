//! Streams: boxed streams, a small `unfold`, and Server-Sent Events framing and parsing.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub use futures_core::Stream;

use super::error::Error;

/// A boxed, sendable stream.
pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

/// A boxed future, the step of an [`unfold`].
pub type Step<T, S> = Pin<Box<dyn Future<Output = Option<(T, S)>> + Send>>;

/// A stream built from a state and an async step: the step returns the next item and the next
/// state, or `None` to end.
pub fn unfold<S, F, T>(state: S, f: F) -> Unfold<S, F, T>
where
    F: FnMut(S) -> Step<T, S>,
{
    Unfold { state: Some(state), f, fut: None }
}

/// The stream of [`unfold`].
#[must_use = "streams do nothing unless polled"]
pub struct Unfold<S, F, T> {
    state: Option<S>,
    f: F,
    fut: Option<Step<T, S>>,
}

impl<S, F, T> std::fmt::Debug for Unfold<S, F, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Unfold")
    }
}

impl<S: Unpin, F: FnMut(S) -> Step<T, S> + Unpin, T> Stream for Unfold<S, F, T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let this = self.get_mut();
        loop {
            if let Some(fut) = this.fut.as_mut() {
                return match fut.as_mut().poll(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(None) => {
                        this.fut = None;
                        Poll::Ready(None)
                    }
                    Poll::Ready(Some((item, next))) => {
                        this.fut = None;
                        this.state = Some(next);
                        Poll::Ready(Some(item))
                    }
                };
            }
            match this.state.take() {
                Some(s) => this.fut = Some((this.f)(s)),
                None => return Poll::Ready(None),
            }
        }
    }
}

/// The next item of a stream.
pub async fn next<S: Stream + Unpin + ?Sized>(s: &mut S) -> Option<S::Item> {
    std::future::poll_fn(|cx| Pin::new(&mut *s).poll_next(cx)).await
}

/// A stream of already known items.
pub fn iter<I: IntoIterator>(items: I) -> impl Stream<Item = I::Item> + Unpin
where
    I::IntoIter: Unpin,
{
    Iter(items.into_iter())
}

struct Iter<I>(I);

impl<I: Iterator + Unpin> Stream for Iter<I> {
    type Item = I::Item;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<I::Item>> {
        Poll::Ready(self.0.next())
    }
}

/// One Server-Sent Event of a stream endpoint: the item and the optional `event`, `id` and
/// `retry` fields.
#[derive(Debug, Clone, PartialEq)]
pub struct SseEvent<T> {
    pub data: T,
    pub event: Option<String>,
    pub id: Option<String>,
    pub retry: Option<u64>,
}

impl<T> SseEvent<T> {
    /// An event with only data.
    #[must_use]
    pub fn of(data: T) -> Self {
        SseEvent { data, event: None, id: None, retry: None }
    }

    /// Sets the `event` name.
    #[must_use]
    pub fn event(mut self, name: impl Into<String>) -> Self {
        self.event = Some(name.into());
        self
    }

    /// Sets the `id`, which a reconnecting client sends back as `Last-Event-ID`.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the `retry` delay in milliseconds.
    #[must_use]
    pub fn retry(mut self, ms: u64) -> Self {
        self.retry = Some(ms);
        self
    }
}

/// The return type of a server-sent-events endpoint.
pub type EventStream<T> = BoxStream<Result<SseEvent<T>, Error>>;

/// Erases the item type of an event stream to typed JSON, for the server core.
pub fn erase_events<T: serde::Serialize + Send + 'static>(events: EventStream<T>) -> EventStream<serde_json::Value> {
    Box::pin(unfold(events, |mut events: EventStream<T>| {
        Box::pin(async move {
            let item = next(&mut events).await?;
            let item = match item {
                Ok(ev) => serde_json::to_value(&ev.data)
                    .map(|data| SseEvent { data, event: ev.event, id: ev.id, retry: ev.retry })
                    .map_err(Error::other),
                Err(e) => Err(e),
            };
            Some((item, events))
        })
    }))
}

fn one_line<'a>(field: &str, v: &'a str) -> Result<&'a str, Error> {
    if v.contains(['\r', '\n']) {
        Err(Error::other(format!("SSE {field} must not contain a line break")))
    } else {
        Ok(v)
    }
}

/// One frame: the optional `id`, `event` and `retry` fields in that order, one `data` line and a
/// blank line. `data` is canonical JSON, which never contains a line break.
///
/// # Errors
/// [`Error::Other`] when `id`, `event` or `data` contains a line break.
pub fn sse_frame(data: &str, id: Option<&str>, event: Option<&str>, retry: Option<u64>) -> Result<String, Error> {
    let mut out = String::new();
    if let Some(id) = id {
        out.push_str(&format!("id: {}\n", one_line("id", id)?));
    }
    if let Some(event) = event {
        out.push_str(&format!("event: {}\n", one_line("event", event)?));
    }
    if let Some(retry) = retry {
        out.push_str(&format!("retry: {retry}\n"));
    }
    out.push_str(&format!("data: {}\n\n", one_line("data", data)?));
    Ok(out)
}

/// A dispatched event of a `text/event-stream` body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseMessage {
    /// The event name, `message` when the event had none.
    pub event: String,
    /// The data lines joined with a line feed.
    pub data: String,
    /// The last `id` seen so far.
    pub id: Option<String>,
}

/// An incremental parser of a `text/event-stream` body (WHATWG HTML section 9.2): lines end in
/// `\n`, `\r\n` or `\r`, `:` lines are comments, a blank line dispatches the event, an event
/// without data is dropped and an event cut off by the end of the stream is discarded.
#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    data: Vec<String>,
    event: String,
    id: Option<String>,
    started: bool,
}

impl SseParser {
    /// A parser with no input yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds a chunk (it may end anywhere, even inside a character) and returns the events it
    /// completed.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseMessage> {
        self.buf.extend_from_slice(chunk);
        if !self.started && self.buf.len() >= 3 {
            self.started = true;
            if self.buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
                self.buf.drain(..3);
            }
        }
        if !self.started {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut start = 0;
        while let Some(rel) = self.buf[start..].iter().position(|b| *b == b'\n' || *b == b'\r') {
            let end = start + rel;
            // A trailing CR may be the first half of CRLF: wait for the next chunk.
            if self.buf[end] == b'\r' && end + 1 >= self.buf.len() {
                break;
            }
            let line = String::from_utf8_lossy(&self.buf[start..end]).into_owned();
            self.line(&line, &mut out);
            start = end + if self.buf[end] == b'\r' && self.buf[end + 1] == b'\n' { 2 } else { 1 };
        }
        self.buf.drain(..start);
        out
    }

    fn line(&mut self, line: &str, out: &mut Vec<SseMessage>) {
        if line.is_empty() {
            if !self.data.is_empty() {
                let event = if self.event.is_empty() { "message".to_string() } else { self.event.clone() };
                out.push(SseMessage { event, data: self.data.join("\n"), id: self.id.clone() });
            }
            self.data.clear();
            self.event.clear();
            return;
        }
        if line.starts_with(':') {
            return;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "data" => self.data.push(value.to_string()),
            "event" => self.event = value.to_string(),
            "id" if !value.contains('\0') => self.id = Some(value.to_string()),
            _ => {}
        }
    }
}


// @if ws_server
/// An unbounded queue with one consumer: a socket's frames in one direction.
pub struct Chan<T> {
    inner: std::sync::Arc<std::sync::Mutex<ChanState<T>>>,
}

struct ChanState<T> {
    items: std::collections::VecDeque<T>,
    closed: bool,
    waker: Option<std::task::Waker>,
}

impl<T> Clone for Chan<T> {
    fn clone(&self) -> Self {
        Chan { inner: std::sync::Arc::clone(&self.inner) }
    }
}

impl<T> std::fmt::Debug for Chan<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Chan")
    }
}

impl<T> Default for Chan<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Chan<T> {
    /// An empty, open queue.
    #[must_use]
    pub fn new() -> Self {
        Chan {
            inner: std::sync::Arc::new(std::sync::Mutex::new(ChanState {
                items: std::collections::VecDeque::new(),
                closed: false,
                waker: None,
            })),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut ChanState<T>) -> R) -> R {
        let mut guard = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut guard)
    }

    /// Appends an item (dropped once the queue is closed) and wakes the consumer.
    pub fn push(&self, item: T) {
        let waker = self.with(|s| {
            if !s.closed {
                s.items.push_back(item);
            }
            s.waker.take()
        });
        if let Some(w) = waker {
            w.wake();
        }
    }

    /// Ends the queue: the consumer drains what is left, then sees the end.
    pub fn close(&self) {
        let waker = self.with(|s| {
            s.closed = true;
            s.waker.take()
        });
        if let Some(w) = waker {
            w.wake();
        }
    }

    /// The next item, `None` at the end.
    pub fn poll_next(&self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.with(|s| match s.items.pop_front() {
            Some(item) => Poll::Ready(Some(item)),
            None if s.closed => Poll::Ready(None),
            None => {
                s.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
    }

    /// Waits for the next item.
    pub async fn recv(&self) -> Option<T> {
        std::future::poll_fn(|cx| self.poll_next(cx)).await
    }
}
// @end

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_follow_the_golden_order() {
        assert_eq!(
            sse_frame("{\"n\":2}", Some("7"), Some("tick"), Some(1500)).ok().as_deref(),
            Some("id: 7\nevent: tick\nretry: 1500\ndata: {\"n\":2}\n\n")
        );
        assert!(sse_frame("x", Some("a\nb"), None, None).is_err());
    }

    #[test]
    fn the_parser_splits_anywhere() {
        let mut p = SseParser::new();
        let mut events = Vec::new();
        for chunk in ["da", "ta: x\r", "\n\r\n"] {
            events.extend(p.push(chunk.as_bytes()));
        }
        assert_eq!(events, [SseMessage { event: "message".into(), data: "x".into(), id: None }]);
    }
}

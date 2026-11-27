//! The reference for rate limits (`spec/platform.md` section 5).
//!
//! * [`key`] is what a limiter is asked about: the operation signature, the subject and the
//!   identity, joined with `|`.
//! * [`MemoryLimiter`] is the rule of the generated memory limiter: **a fixed window per key,
//!   starting with the first call of the key that finds no open window**. Every other limiter a
//!   server may be given only has to allow at most `limit` calls of one key in any window it
//!   defines; this one is what three targets are compared by.
//!
//! The rule, call by call, for a key whose window is `window_seconds` long:
//!
//! 1. A key without an open window starts one at the time of the call, with no calls in it.
//!    A window is open from its start up to, **not including**, its start plus its length: a
//!    call at exactly that instant starts the next window.
//! 2. A call in an open window is allowed while the window has fewer than `limit` calls, and
//!    counts. A call beyond that is refused and does **not** count; the window stays as it was.
//! 3. `retry_after_seconds` is 0 for an allowed call. For a refused call it is the time left in
//!    the window, rounded **up** to whole seconds, and at least 1: a call at 59.001 s of a
//!    60 s window is told 1.
//! 4. A clock that goes back is not special: a call before the start of the open window is in it,
//!    and is told at most the length of the window.
//!
//! Windows are not aligned to the clock: a key that is first seen at 12:00:37 has the window
//! 12:00:37 to 12:01:37.

use std::collections::BTreeMap;

use veld_ir::LimitSubject;

/// What a limiter answers (`take(key, limit, windowSeconds) -> { allowed, retryAfterSeconds }`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub allowed: bool,
    pub retry_after_seconds: u32,
}

/// The key of a limit: `operation|k|identity`, where `operation` is the signature of the
/// endpoint, `k` the word of the subject and `identity` the text the application gave for the
/// caller (`user`), the tenant text of the caller (`tenant`) or the client address (`ip`).
#[must_use]
pub fn key(operation: &str, by: LimitSubject, identity: &str) -> String {
    format!("{operation}|{}|{identity}", by.word())
}

#[derive(Debug, Clone, Copy)]
struct Window {
    start_ms: i64,
    calls: u32,
}

/// The memory limiter of the generated servers, with the clock given by the caller.
#[derive(Debug, Clone, Default)]
pub struct MemoryLimiter {
    windows: BTreeMap<String, Window>,
}

impl MemoryLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One call of `key` at `now_ms`, with the limit of the endpoint.
    pub fn take(&mut self, key: &str, limit: u32, window_seconds: u32, now_ms: i64) -> Decision {
        let length = i64::from(window_seconds).saturating_mul(1000);
        let open = self
            .windows
            .get(key)
            .filter(|w| now_ms < w.start_ms.saturating_add(length))
            .copied();
        let mut window = open.unwrap_or(Window {
            start_ms: now_ms,
            calls: 0,
        });
        let decision = if window.calls < limit {
            window.calls += 1;
            Decision {
                allowed: true,
                retry_after_seconds: 0,
            }
        } else {
            let left = window
                .start_ms
                .saturating_add(length)
                .saturating_sub(now_ms);
            let seconds = (left.clamp(1, length.max(1)) + 999) / 1000;
            Decision {
                allowed: false,
                retry_after_seconds: u32::try_from(seconds).unwrap_or(u32::MAX),
            }
        };
        self.windows.insert(key.to_owned(), window);
        decision
    }
}

/// The decisions of a memory limiter for `calls`, each a key and the time of the call in
/// milliseconds, in the order given.
#[must_use]
pub fn replay(calls: &[(&str, i64)], limit: u32, window_seconds: u32) -> Vec<Decision> {
    let mut limiter = MemoryLimiter::new();
    calls
        .iter()
        .map(|(k, t)| limiter.take(k, limit, window_seconds, *t))
        .collect()
}

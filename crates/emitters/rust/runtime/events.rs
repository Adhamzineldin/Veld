//! Events (`spec/events.md` sections 6 to 9): the transport interface, the memory transport, the
//! envelope of a message (CloudEvents 1.0, structured JSON mode) and the publish and consume
//! pipelines that the generated publishers and subscribers call.
//!
//! The file needs nothing but `std` and the rest of this runtime: there is no async runtime in it.
//! The memory transport delivers on the executor that polls `MemoryTransport::run` or
//! `MemoryTransport::idle`, so handlers run on the application's own runtime.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::canonical::{canonical, canonical_bytes};
use super::error::{Error, Issue};
use super::schema::{Rules, Scalar, Schema};
// @if event_flags
use super::flags::{FlagContext, FlagProvider};
use super::rules::{FlagView, Reading};
// @end

/// A boxed, sendable future: what the object-safe transport traits return.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---------------------------------------------------------------------------
// The transport (section 9)
// ---------------------------------------------------------------------------

/// What `deliver` answers: `Again` only when the handler failed; everything else, a rejection
/// included, is `Done`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Again,
}

/// One message as a transport hands it to a consumer. `attempt` starts at 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportMessage {
    pub key: Option<String>,
    pub bytes: Vec<u8>,
    pub attempt: u32,
}

/// The callback a subscription gives its transport.
pub type Deliver = Arc<dyn Fn(TransportMessage) -> BoxFuture<'static, Outcome> + Send + Sync>;

/// What `Transport::subscribe` returns: `close` stops the subscription.
pub trait TransportSubscription: Send + Sync {
    fn close(&self) -> BoxFuture<'_, ()>;
}

/// The options of a channel that a transport gets with every call (`spec/events.md` section 10.6):
/// `@ordered` and `@deadLetter`. Both are false for a channel without the annotation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChannelOptions {
    pub ordered: bool,
    pub dead_letter: bool,
}

impl ChannelOptions {
    /// No annotation.
    pub const NONE: ChannelOptions = ChannelOptions { ordered: false, dead_letter: false };
}

/// What the generated publishers and subscribers depend on (`spec/events.md` section 9). A broker
/// adapter implements it; [`MemoryTransport`] is the one that ships with every target.
///
/// Both calls take the options of the channel last (section 10.6). Rust has no optional argument,
/// so a transport written for phase E1 adds the parameter. A transport that cannot give `ordered`
/// must fail the call instead of ignoring it.
pub trait Transport: Send + Sync {
    /// Completes when the transport has accepted the message.
    fn publish<'a>(
        &'a self,
        topic: &'a str,
        key: Option<&'a str>,
        bytes: &'a [u8],
        options: ChannelOptions,
    ) -> BoxFuture<'a, Result<(), Error>>;

    /// `deliver` answers `Done` or `Again`.
    fn subscribe<'a>(
        &'a self,
        topic: &'a str,
        group: &'a str,
        deliver: Deliver,
        options: ChannelOptions,
    ) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>>;
}

/// A message the memory transport gave up on after `max_attempts` deliveries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadLetter {
    pub topic: String,
    pub group: String,
    pub key: Option<String>,
    pub bytes: Vec<u8>,
    pub attempts: u32,
}

struct Member {
    id: u64,
    deliver: Deliver,
}

struct Queued {
    key: Option<String>,
    bytes: Vec<u8>,
}

struct GroupState {
    id: u64,
    members: Vec<Member>,
    queue: VecDeque<Queued>,
    next: usize,
    busy: bool,
}

#[derive(Default)]
struct State {
    groups: BTreeMap<(String, String), GroupState>,
    pending: usize,
    dead: Vec<DeadLetter>,
    next_id: u64,
    version: u64,
    wakers: Vec<Waker>,
}

struct Shared {
    max_attempts: u32,
    state: Mutex<State>,
}

/// What one step of the delivery loop found.
enum Work {
    Item { topic: String, group: String, gid: u64, item: Queued },
    /// Nothing is ready; the state had this version.
    Wait(u64),
}

/// Bumps the version of the state and takes the wakers to call once the lock is released.
fn bump(st: &mut State) -> Vec<Waker> {
    st.version += 1;
    std::mem::take(&mut st.wakers)
}

fn wake_all(wakers: Vec<Waker>) {
    for w in wakers {
        w.wake();
    }
}

/// The transport that lives in the process, for tests and for development. A message goes to one
/// subscriber of each group that was subscribed when it was published; a topic nobody subscribed
/// to drops it. A group gets its messages in publish order, one at a time, so the order per key
/// holds. A failed handler gets the message again at once, up to `max_attempts`; then it goes to
/// [`MemoryTransport::dead_letters`].
///
/// It has no thread and no executor of its own. `publish` only queues the message. Handlers run
/// while a future of [`MemoryTransport::idle`] or [`MemoryTransport::run`] is polled, on the
/// executor that polls it: a test awaits `idle()`, a program spawns `run()` once. A clone shares
/// the same queues.
#[derive(Clone)]
pub struct MemoryTransport {
    shared: Arc<Shared>,
}

impl Default for MemoryTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MemoryTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let st = lock(&self.shared.state);
        write!(f, "MemoryTransport({} groups, {} pending)", st.groups.len(), st.pending)
    }
}

impl MemoryTransport {
    /// A transport that delivers a message up to 3 times.
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_attempts(3)
    }

    /// A transport that delivers a message up to `max_attempts` times (at least once).
    #[must_use]
    pub fn with_max_attempts(max_attempts: u32) -> Self {
        MemoryTransport {
            shared: Arc::new(Shared { max_attempts: max_attempts.max(1), state: Mutex::new(State::default()) }),
        }
    }

    /// The messages that were delivered `max_attempts` times and never acknowledged.
    #[must_use]
    pub fn dead_letters(&self) -> Vec<DeadLetter> {
        lock(&self.shared.state).dead.clone()
    }

    /// Completes when every published message has been delivered or dead-lettered, also the ones
    /// that handlers publish meanwhile. It delivers while it waits.
    pub async fn idle(&self) {
        loop {
            if lock(&self.shared.state).pending == 0 {
                return;
            }
            if let Err(version) = self.step().await {
                self.changed(version).await;
            }
        }
    }

    /// A future that delivers messages for as long as it is polled: spawn it once on the
    /// application's runtime. Drop it (or abort the task) to stop.
    pub fn run(&self) -> impl Future<Output = ()> + Send + 'static {
        let me = self.clone();
        async move {
            loop {
                if let Err(version) = me.step().await {
                    me.changed(version).await;
                }
            }
        }
    }

    fn enqueue(&self, topic: &str, key: Option<&str>, bytes: &[u8]) {
        let wakers = {
            let mut st = lock(&self.shared.state);
            let mut added = 0usize;
            for ((t, _), g) in &mut st.groups {
                if t == topic {
                    g.queue.push_back(Queued { key: key.map(str::to_string), bytes: bytes.to_vec() });
                    added += 1;
                }
            }
            st.pending += added;
            bump(&mut st)
        };
        wake_all(wakers);
    }

    fn join(&self, topic: &str, group: &str, deliver: Deliver) -> MemorySubscription {
        let mut st = lock(&self.shared.state);
        st.next_id += 1;
        let member = st.next_id;
        st.next_id += 1;
        let fresh = st.next_id;
        let entry = st.groups.entry((topic.to_string(), group.to_string())).or_insert_with(|| GroupState {
            id: fresh,
            members: Vec::new(),
            queue: VecDeque::new(),
            next: 0,
            busy: false,
        });
        entry.members.push(Member { id: member, deliver });
        MemorySubscription {
            shared: Arc::clone(&self.shared),
            key: (topic.to_string(), group.to_string()),
            gid: entry.id,
            member,
        }
    }

    /// Takes the next message of a group that is not busy, or says what version of the state
    /// found none.
    fn take_work(&self) -> Work {
        let mut st = lock(&self.shared.state);
        let version = st.version;
        for ((topic, group), g) in &mut st.groups {
            if g.busy {
                continue;
            }
            if let Some(item) = g.queue.pop_front() {
                g.busy = true;
                return Work::Item { topic: topic.clone(), group: group.clone(), gid: g.id, item };
            }
        }
        Work::Wait(version)
    }

    /// Delivers one message to completion. `Err(version)`: nothing was ready.
    async fn step(&self) -> Result<(), u64> {
        match self.take_work() {
            Work::Wait(version) => Err(version),
            Work::Item { topic, group, gid, item } => {
                let dead = self.deliver_item(&topic, &group, gid, &item).await;
                let wakers = {
                    let mut st = lock(&self.shared.state);
                    if let Some(g) = st.groups.get_mut(&(topic.clone(), group.clone())).filter(|g| g.id == gid) {
                        g.busy = false;
                    }
                    if let Some(letter) = dead {
                        st.dead.push(letter);
                    }
                    st.pending = st.pending.saturating_sub(1);
                    bump(&mut st)
                };
                wake_all(wakers);
                Ok(())
            }
        }
    }

    /// The attempts at one message; the dead letter when they were all used up.
    async fn deliver_item(&self, topic: &str, group: &str, gid: u64, item: &Queued) -> Option<DeadLetter> {
        let mut attempt = 1u32;
        loop {
            let deliver = self.pick(topic, group, gid)?;
            let message = TransportMessage { key: item.key.clone(), bytes: item.bytes.clone(), attempt };
            if Guarded(call(&deliver, message)).await == Outcome::Done {
                return None;
            }
            if attempt >= self.shared.max_attempts {
                return Some(DeadLetter {
                    topic: topic.to_string(),
                    group: group.to_string(),
                    key: item.key.clone(),
                    bytes: item.bytes.clone(),
                    attempts: attempt,
                });
            }
            attempt += 1;
        }
    }

    /// The next subscriber of the group in rotation; `None` when the group is gone or empty.
    fn pick(&self, topic: &str, group: &str, gid: u64) -> Option<Deliver> {
        let mut st = lock(&self.shared.state);
        let g = st.groups.get_mut(&(topic.to_string(), group.to_string())).filter(|g| g.id == gid)?;
        let count = g.members.len();
        if count == 0 {
            return None;
        }
        g.next %= count;
        let deliver = Arc::clone(&g.members.get(g.next)?.deliver);
        g.next = (g.next + 1) % count;
        Some(deliver)
    }

    /// Completes once the state is no longer at `version`.
    async fn changed(&self, version: u64) {
        std::future::poll_fn(|cx: &mut Context<'_>| {
            let mut st = lock(&self.shared.state);
            if st.version != version {
                return Poll::Ready(());
            }
            if !st.wakers.iter().any(|w| w.will_wake(cx.waker())) {
                st.wakers.push(cx.waker().clone());
            }
            Poll::Pending
        })
        .await;
    }
}

impl Transport for MemoryTransport {
    fn publish<'a>(
        &'a self,
        topic: &'a str,
        key: Option<&'a str>,
        bytes: &'a [u8],
        _options: ChannelOptions,
    ) -> BoxFuture<'a, Result<(), Error>> {
        self.enqueue(topic, key, bytes);
        Box::pin(std::future::ready(Ok(())))
    }

    fn subscribe<'a>(
        &'a self,
        topic: &'a str,
        group: &'a str,
        deliver: Deliver,
        _options: ChannelOptions,
    ) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>> {
        let sub: Box<dyn TransportSubscription> = Box::new(self.join(topic, group, deliver));
        Box::pin(std::future::ready(Ok(sub)))
    }
}

struct MemorySubscription {
    shared: Arc<Shared>,
    key: (String, String),
    gid: u64,
    member: u64,
}

impl TransportSubscription for MemorySubscription {
    fn close(&self) -> BoxFuture<'_, ()> {
        let wakers = {
            let mut st = lock(&self.shared.state);
            let mut dropped = 0usize;
            let mut gone = false;
            if let Some(g) = st.groups.get_mut(&self.key).filter(|g| g.id == self.gid) {
                g.members.retain(|m| m.id != self.member);
                if g.members.is_empty() {
                    dropped = g.queue.len();
                    gone = true;
                }
            }
            if gone {
                st.groups.remove(&self.key);
            }
            st.pending = st.pending.saturating_sub(dropped);
            bump(&mut st)
        };
        wake_all(wakers);
        Box::pin(std::future::ready(()))
    }
}

/// Calls a subscriber; a panic while it builds its future counts as a failed handler.
fn call(deliver: &Deliver, message: TransportMessage) -> BoxFuture<'static, Outcome> {
    match catch_unwind(AssertUnwindSafe(|| deliver(message))) {
        Ok(future) => future,
        Err(_) => Box::pin(std::future::ready(Outcome::Again)),
    }
}

/// A future whose panic is an `Again`: a handler that panics gets the message again like one that
/// fails.
struct Guarded(BoxFuture<'static, Outcome>);

impl Future for Guarded {
    type Output = Outcome;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Outcome> {
        let inner = self.0.as_mut();
        match catch_unwind(AssertUnwindSafe(|| inner.poll(cx))) {
            Ok(poll) => poll,
            Err(_) => Poll::Ready(Outcome::Again),
        }
    }
}

// ---------------------------------------------------------------------------
// What the generated code describes
// ---------------------------------------------------------------------------

/// One event as the generated code describes it to the pipelines.
#[derive(Debug, Clone, Copy)]
pub struct EventDef {
    /// The name of the event (`BookingPaid`).
    pub name: &'static str,
    /// The declaration signature: the `type` of every message of this event.
    pub sig: &'static str,
    pub schema: &'static Schema,
    /// The rules digest (`veldrules`), or `None` when the event reaches no enforced invariant.
    pub digest: Option<&'static str>,
    // @if event_flags
    /// The flags of the invariants of the event, sorted: read once per message.
    pub flags: &'static [&'static str],
    // @end
    // @if tenant_events
    /// The wire name of the `@tenant` field of the event (`spec/platform.md` section 3): its text
    /// is the `veldtenant` attribute of every message of the event.
    pub tenant: Option<&'static str>,
    // @end
}

/// One channel as the generated code describes it to the publisher.
#[derive(Debug, Clone, Copy)]
pub struct ChannelDef {
    pub topic: &'static str,
    /// The wire name of the key field of the channel, or `None` when it has no key.
    pub key: Option<&'static str>,
}

// #if rules
pub use super::rules::Violation;
// #endif
// #if !rules

/// A rule violation. This crate has no invariants, so none is ever reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule: String,
    pub path: String,
    pub fields: Vec<String>,
    pub message: String,
    pub error: Option<String>,
}
// #endif

/// The facts about a message every handler gets next to the typed event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMeta {
    pub id: String,
    pub source: String,
    /// The `type` attribute as the publisher wrote it: the signature of the event it knew.
    pub r#type: String,
    /// The key text, absent when the channel has no key.
    pub subject: Option<String>,
    /// The time the publisher wrote, as text.
    pub time: Option<String>,
    /// The delivery attempt, 1 for the first.
    pub attempt: u32,
    // @if tenant_events
    /// The tenant of the event, as the publisher wrote it in `veldtenant`: a `string` as it is, a
    /// `uuid` in lower case, an `int` or a `long` in decimal digits. A consumer does not check it
    /// (`spec/platform.md` section 3).
    pub veldtenant: Option<String>,
    // @end
}

/// What a publisher call returns: the message it sent, without the data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedEvent {
    pub id: String,
    pub source: String,
    pub r#type: String,
    pub subject: Option<String>,
    pub time: String,
}

type Hook<T> = Arc<dyn Fn(&T) + Send + Sync>;
// @if event_flags
/// A provider of flags with its type erased, so that the options need no type parameter.
type FlagReader = Arc<dyn Fn(String, FlagContext) -> BoxFuture<'static, Result<bool, Error>> + Send + Sync>;
// @end
// #if rules
type RuleHook = Hook<super::rules::RuleEvent>;
// #endif

/// Calls a hook. A hook that panics never changes what happens to the message.
fn call_hook<T>(hook: Option<&Hook<T>>, event: &T) {
    if let Some(f) = hook {
        let _ = catch_unwind(AssertUnwindSafe(|| f(event)));
    }
}
// @if event_flags

fn flag_reader<P: FlagProvider>(provider: Arc<P>) -> FlagReader {
    Arc::new(move |name, context| {
        let provider = Arc::clone(&provider);
        Box::pin(async move { provider.is_on(&name, &context).await })
    })
}

/// The readings of one message (`spec/platform.md` section 11): every flag of the event, once, with
/// a context that has no principal and the signature of the event as the operation. `None`
/// without a provider: the flagged invariants are then not evaluated. A provider that fails is a
/// `Failed` reading (the invariant counts as on) and the error goes to `on_flag_error`.
async fn read_flags(reader: Option<&FlagReader>, on_error: Option<&Hook<Error>>, def: &EventDef) -> Option<FlagView> {
    let reader = reader?;
    let mut view = FlagView::new();
    for name in def.flags {
        match reader((*name).to_string(), FlagContext::of_event(def.sig)).await {
            Ok(true) => view.insert(name, Reading::On),
            Ok(false) => view.insert(name, Reading::Off),
            Err(err) => {
                call_hook(on_error, &err);
                view.insert(name, Reading::Failed);
            }
        }
    }
    Some(view)
}
// @end

/// `veld:pkg/Name@hash` -> `veld:pkg/Name`.
fn type_name(sig: &str) -> &str {
    sig.rsplit_once('@').map_or(sig, |(name, _)| name)
}

// ---------------------------------------------------------------------------
// Dates: the canonical `datetime` of a message, written here and nowhere else
// ---------------------------------------------------------------------------

fn digits(s: &str, n: usize) -> Option<i64> {
    if s.len() == n && s.bytes().all(|b| b.is_ascii_digit()) { s.parse().ok() } else { None }
}

fn leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if leap(y) => 29,
        _ => 28,
    }
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The offset of a datetime in minutes: `Z` is 0.
fn zone_minutes(zone: &str) -> Option<i64> {
    let sign = if zone.starts_with('-') { -1 } else { 1 };
    let (hours, minutes) = zone.get(1..)?.split_once(':')?;
    let (hours, minutes) = (digits(hours, 2)?, digits(minutes, 2)?);
    (hours <= 23 && minutes <= 59).then_some(sign * (hours * 60 + minutes))
}

/// The milliseconds of a fraction of a second, cut (not rounded) at 3 digits.
fn fraction_millis(frac: &str) -> Option<i64> {
    if frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut ms = 0i64;
    for (i, b) in frac.bytes().take(3).enumerate() {
        ms += i64::from(b - b'0') * [100, 10, 1].get(i).copied().unwrap_or(0);
    }
    Some(ms)
}

/// The instant of an RFC 3339 datetime, in milliseconds since the epoch. It reads exactly the
/// datetimes the schema accepts and cuts the fraction at the millisecond.
fn datetime_millis(s: &str) -> Option<i64> {
    let (date, rest) = s.split_once('T')?;
    let (time, offset) = match rest.strip_suffix('Z') {
        Some(t) => (t, 0),
        None => {
            let (t, zone) = rest.split_at(rest.rfind(['+', '-'])?);
            (t, zone_minutes(zone)?)
        }
    };
    let (main, frac_ms) = match time.split_once('.') {
        Some((m, f)) => (m, fraction_millis(f)?),
        None => (time, 0),
    };
    let [y, mo, da]: [&str; 3] = date.split('-').collect::<Vec<_>>().try_into().ok()?;
    let [h, mi, sec]: [&str; 3] = main.split(':').collect::<Vec<_>>().try_into().ok()?;
    let (y, mo, da) = (digits(y, 4)?, digits(mo, 2)?, digits(da, 2)?);
    let (h, mi, sec) = (digits(h, 2)?, digits(mi, 2)?, digits(sec, 2)?);
    if !(1..=12).contains(&mo) || da < 1 || da > days_in_month(y, mo) || h > 23 || mi > 59 || sec > 59 {
        return None;
    }
    let seconds = days_from_civil(y, mo, da) * 86_400 + h * 3600 + mi * 60 + sec - offset * 60;
    Some(seconds * 1000 + frac_ms)
}

/// A datetime in UTC with `Z`; the fraction `.fff` only when the milliseconds are not zero.
fn format_millis(ms: i64) -> String {
    let seconds = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let (y, mo, d) = civil_from_days(seconds.div_euclid(86_400));
    let rem = seconds.rem_euclid(86_400);
    let fraction = if millis == 0 { String::new() } else { format!(".{millis:03}") };
    format!("{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}{fraction}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// The canonical form of a `datetime` (`spec/ir.md` section 5.3), `None` when it is not one.
fn canon_datetime(s: &str) -> Option<String> {
    datetime_millis(s).map(format_millis)
}

fn epoch_millis(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or(i64::MAX),
        Err(e) => i64::try_from(e.duration().as_millis()).map_or(i64::MIN, |n| -n),
    }
}

/// The canonical scalars of `spec/ir.md` section 5.3 in a wire value: lowercase uuids, UTC
/// datetimes. The other scalars are written as given.
fn canon_scalars(schema: &Schema, v: &mut Value) {
    match schema {
        Schema::Scalar(Scalar::Uuid, _) => {
            if let Value::String(s) = v {
                *s = s.to_lowercase();
            }
        }
        Schema::Scalar(Scalar::Datetime, _) => {
            if let Some(text) = v.as_str().and_then(canon_datetime) {
                *v = Value::String(text);
            }
        }
        Schema::Scalar(..) | Schema::Enum(_) => {}
        Schema::List(of, _) => {
            for x in v.as_array_mut().into_iter().flatten() {
                canon_scalars(of, x);
            }
        }
        Schema::Map(of, _) => {
            for x in v.as_object_mut().into_iter().flat_map(Map::values_mut) {
                canon_scalars(of, x);
            }
        }
        Schema::Page(of) => {
            if let Some(items) = v.get_mut("items") {
                canon_scalars(&Schema::List(of, Rules::NONE), items);
            }
        }
        Schema::Model(m) => {
            for f in m.fields {
                if let Some(x) = v.get_mut(f.wire) {
                    canon_scalars(f.schema, x);
                }
            }
        }
        Schema::Union(u) => {
            let label = v.get(u.discriminator).and_then(Value::as_str);
            if let Some((_, arm)) = label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) {
                canon_scalars(arm, v);
            }
        }
    }
}

// #if rules
// ---------------------------------------------------------------------------
// Invariants
// ---------------------------------------------------------------------------

fn report(
    hook: Option<&RuleHook>,
    def: &EventDef,
    (position, outcome): (super::rules::Position, super::rules::Outcome),
    violations: &[Violation],
    micros: u64,
) {
    for violation in violations {
        let event = super::rules::RuleEvent {
            rule: violation.rule.clone(),
            operation: def.sig.to_string(),
            position,
            outcome,
            violation: violation.clone(),
            elapsed_micros: micros,
        };
        call_hook(hook, &event);
    }
}

/// The invariants of an event in wire form, in full mode: the enforced violations are returned, and
/// both kinds are reported to the hook, the enforced ones first. An audited rule never changes what
/// happens.
fn run_rules(def: &EventDef, wire: &Value, hook: Option<&RuleHook>, position: super::rules::Position) -> Vec<Violation> {
    let watch = super::rules::Stopwatch::start();
    let audited = audited_violations(def, wire);
    let violations = super::rules::check_wire(def.schema, wire, "$", super::rules::Mode::Full);
    let micros = watch.micros();
    report(hook, def, (position, super::rules::Outcome::Rejected), &violations, micros);
    report(hook, def, (position, super::rules::Outcome::Audited), &audited, micros);
    violations
}
// #endif
// #if rollout

/// The violations of the audited rules of an event (`@rollout(audit)`).
fn audited_violations(def: &EventDef, wire: &Value) -> Vec<Violation> {
    super::rules::audit_wire(def.schema, wire, "$", super::rules::Mode::Full)
}
// #endif
// #if rules_plain

/// A crate without an audited rule has none to report.
fn audited_violations(_def: &EventDef, _wire: &Value) -> Vec<Violation> {
    Vec::new()
}
// #endif

// ---------------------------------------------------------------------------
// Publishing (section 7)
// ---------------------------------------------------------------------------

/// The options of a publisher: the id source, the clock and the rule hook.
#[derive(Clone, Default)]
pub struct PublisherOptions {
    new_id: Option<Arc<dyn Fn() -> String + Send + Sync>>,
    clock: Option<Arc<dyn Fn() -> SystemTime + Send + Sync>>,
    // #if rules
    on_rule_violation: Option<RuleHook>,
    // #endif
    // @if event_flags
    flags: Option<FlagReader>,
    on_flag_error: Option<Hook<Error>>,
    // @end
}

impl PublisherOptions {
    /// The defaults: a random UUID as the id, the system clock, no hook.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The id source of the envelope: a non-empty string, unique among the events of the service.
    #[must_use]
    pub fn new_id(mut self, f: impl Fn() -> String + Send + Sync + 'static) -> Self {
        self.new_id = Some(Arc::new(f));
        self
    }

    /// The clock: the `time` of the envelope.
    #[must_use]
    pub fn clock(mut self, f: impl Fn() -> SystemTime + Send + Sync + 'static) -> Self {
        self.clock = Some(Arc::new(f));
        self
    }
    // #if rules

    /// Hears the violations of invariants: the enforced ones of an event that is not sent, and the
    /// audited ones.
    #[must_use]
    pub fn on_rule_violation(mut self, f: impl Fn(&super::rules::RuleEvent) + Send + Sync + 'static) -> Self {
        self.on_rule_violation = Some(Arc::new(f));
        self
    }
    // #endif
    // @if event_flags

    /// The provider of the flags of the invariants (`spec/platform.md` section 11): read once per
    /// event, with no principal. Without one the flagged invariants are not evaluated.
    #[must_use]
    pub fn flags<P: FlagProvider>(mut self, provider: Arc<P>) -> Self {
        self.flags = Some(flag_reader(provider));
        self
    }

    /// Hears a provider that failed: its flags count as on.
    #[must_use]
    pub fn on_flag_error(mut self, f: impl Fn(&Error) + Send + Sync + 'static) -> Self {
        self.on_flag_error = Some(Arc::new(f));
        self
    }
    // @end
}

/// What a generated publisher hands the pipeline.
pub struct PublisherContext {
    transport: Arc<dyn Transport>,
    /// `veld:<package>/<Service>`.
    source: &'static str,
    /// The signature of the `ValidationError` a failed publish returns.
    validation_sig: &'static str,
    options: PublisherOptions,
}

impl PublisherContext {
    #[must_use]
    pub fn new(
        transport: Arc<dyn Transport>,
        source: &'static str,
        validation_sig: &'static str,
        options: PublisherOptions,
    ) -> Self {
        PublisherContext { transport, source, validation_sig, options }
    }

    /// The `ValidationError` of a publish that stops: raised locally, with `issues` and, with
    /// rules, `violations`. The operation is the signature of the event.
    fn validation(&self, def: &EventDef, issues: &[Issue], violations: Vec<Violation>) -> Error {
        // #if !rules
        let _ = violations;
        // #endif
        Error::Problem(super::error::Problem {
            sig: self.validation_sig.to_string(),
            title: "ValidationError".to_string(),
            status: 422,
            op: Some(def.sig.to_string()),
            data: serde_json::json!({ "issues": issues }),
            schema: None,
            // #if rules
            rulings: super::rules::Rulings::boxed(violations, true),
            // #endif
        })
    }
}

/// A random UUID (version 4) from the standard library's per-process random keys; it has no
/// dependency, and the uniqueness of a version 4 UUID in practice.
fn random_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let word = |salt: u8| {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(n);
        h.write_u128(nanos);
        h.write_u8(salt);
        h.finish()
    };
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&word(0).to_be_bytes());
    bytes[8..].copy_from_slice(&word(1).to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// The key text (section 4): a string as it is, a uuid in lowercase, an int or a long in decimal
/// digits.
fn key_text(channel: &ChannelDef, wire: &Value) -> Result<Option<String>, Error> {
    let Some(key) = channel.key else {
        return Ok(None);
    };
    match wire.get(key) {
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        _ => Err(Error::other(format!("veld events: the key `{key}` is not a string or a number"))),
    }
}

/// Steps 1 to 3 of section 7: defaults, shape, invariants, then the key. Returns the data in wire
/// form and the key text.
fn prepare<E: Serialize>(
    ctx: &PublisherContext,
    channel: &ChannelDef,
    def: &EventDef,
    event: &E,
    // @if event_flags
    readings: &Option<FlagView>,
    // @end
) -> Result<(Value, Option<String>), Error> {
    let typed = serde_json::to_value(event).map_err(|e| Error::other(format!("veld events: {e}")))?;
    let mut wire = def.schema.to_wire(&typed, false, false);
    // @if derived
    // A publisher computes the derived fields of an event before its invariants; one that cannot be
    // computed stops the publish like a violated invariant (`spec/rules.md` section 17.2).
    let (computed, failed) = super::rules::complete_wire(def.schema, &wire, "$");
    wire = computed;
    if !failed.is_empty() {
        let micros = 0;
        report(ctx.options.on_rule_violation.as_ref(), def, (super::rules::Position::Output, super::rules::Outcome::Rejected), &failed, micros);
        let issues: Vec<Issue> = failed.iter().map(Violation::issue).collect();
        return Err(ctx.validation(def, &issues, failed));
    }
    // @end
    canon_scalars(def.schema, &mut wire);
    let issues = def.schema.validate(&wire, "$", false);
    if !issues.is_empty() {
        return Err(ctx.validation(def, &issues, Vec::new()));
    }
    // #if rules
    let hook = ctx.options.on_rule_violation.as_ref();
    // @if event_flags
    let violations = super::rules::with_view(readings.as_ref(), || run_rules(def, &wire, hook, super::rules::Position::Output));
    // @end
    // @if !event_flags
    let violations = run_rules(def, &wire, hook, super::rules::Position::Output);
    // @end
    if !violations.is_empty() {
        let issues: Vec<Issue> = violations.iter().map(Violation::issue).collect();
        return Err(ctx.validation(def, &issues, violations));
    }
    // #endif
    let subject = key_text(channel, &wire)?;
    if subject.as_deref() == Some("") {
        let path = format!("$.{}", channel.key.unwrap_or_default());
        let issue = Issue { path, code: "key".to_string(), message: "the key of the channel must not be empty".to_string() };
        return Err(ctx.validation(def, &[issue], Vec::new()));
    }
    Ok((wire, subject))
}

/// Publishes one event (section 7): defaults, shape, invariants, the key, the envelope with its
/// canonical bytes, then the transport. Nothing is sent when a step fails: the call returns the
/// `ValidationError`, raised locally.
///
/// # Errors
/// The `ValidationError` of a value that does not have the shape of the event, one that violates
/// an enforced invariant, or an empty key; `Error::Other` for a misuse (an id source that returns
/// an empty id); the transport's own error.
pub async fn publish_event<E: Serialize + Sync>(
    ctx: &PublisherContext,
    channel: &ChannelDef,
    def: &EventDef,
    event: &E,
) -> Result<PublishedEvent, Error> {
    publish_event_with(ctx, channel, ChannelOptions::NONE, def, event).await
}

/// [`publish_event`] for a channel with `@ordered` or `@deadLetter`: the transport gets `options`.
///
/// # Errors
/// As [`publish_event`].
pub async fn publish_event_with<E: Serialize + Sync>(
    ctx: &PublisherContext,
    channel: &ChannelDef,
    options: ChannelOptions,
    def: &EventDef,
    event: &E,
) -> Result<PublishedEvent, Error> {
    // @if outbox
    let (published, bytes) = build_message(ctx, channel, def, event).await?;
    ctx.transport.publish(channel.topic, published.subject.as_deref(), &bytes, options).await?;
    Ok(published)
}

/// Steps 1 to 5 of section 7 for one event: defaults and shape, invariants, the key, the id and the
/// clock, the envelope and its bytes. Publishing sends them to a transport and `emit` saves them in
/// an outbox: this is the code they share.
async fn build_message<E: Serialize + Sync>(
    ctx: &PublisherContext,
    channel: &ChannelDef,
    def: &EventDef,
    event: &E,
) -> Result<(PublishedEvent, Vec<u8>), Error> {
    // @end
    // @if event_flags
    let readings = read_flags(ctx.options.flags.as_ref(), ctx.options.on_flag_error.as_ref(), def).await;
    let (wire, subject) = prepare(ctx, channel, def, event, &readings)?;
    // @end
    // @if !event_flags
    let (wire, subject) = prepare(ctx, channel, def, event)?;
    // @end
    let id = ctx.options.new_id.as_ref().map_or_else(random_id, |f| f());
    if id.is_empty() {
        return Err(Error::other("veld events: the id source returned an empty id"));
    }
    // @if tenant_events
    let tenant = def.tenant.and_then(|name| match wire.get(name) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    });
    // @end
    let now = ctx.options.clock.as_ref().map_or_else(SystemTime::now, |f| f());
    let time = format_millis(epoch_millis(now));
    let mut envelope = Map::new();
    envelope.insert("specversion".into(), Value::from("1.0"));
    envelope.insert("id".into(), Value::from(id.as_str()));
    envelope.insert("source".into(), Value::from(ctx.source));
    envelope.insert("type".into(), Value::from(def.sig));
    envelope.insert("time".into(), Value::from(time.as_str()));
    envelope.insert("datacontenttype".into(), Value::from("application/json"));
    envelope.insert("data".into(), wire);
    if let Some(s) = &subject {
        envelope.insert("subject".into(), Value::from(s.as_str()));
    }
    if let Some(digest) = def.digest {
        envelope.insert("veldrules".into(), Value::from(digest));
    }
    // @if tenant_events
    if let Some(text) = tenant {
        envelope.insert("veldtenant".into(), Value::from(text));
    }
    // @end
    let bytes = canonical_bytes(&Value::Object(envelope));
    // @if outbox
    Ok((PublishedEvent { id, source: ctx.source.to_string(), r#type: def.sig.to_string(), subject, time }, bytes))
    // @end
    // @if !outbox
    ctx.transport.publish(channel.topic, subject.as_deref(), &bytes, options).await?;
    Ok(PublishedEvent { id, source: ctx.source.to_string(), r#type: def.sig.to_string(), subject, time })
    // @end
}
// @if outbox

// ---------------------------------------------------------------------------
// The outbox (section 11)
// ---------------------------------------------------------------------------

/// One message an outbox keeps until the relay has sent it. `id` is the application's own and
/// opaque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxEntry {
    pub id: String,
    pub topic: String,
    /// The key text (the `subject` of the message), or `None` for a channel without a key.
    pub key: Option<String>,
    pub bytes: Vec<u8>,
}

/// Where the events of a move are written, in the transaction that changes the state. The
/// application implements it over its own store: `save` is called by `emit`, `pending` and `done`
/// by the relay. Like the transport, it is object safe and its methods are async.
pub trait Outbox: Send + Sync + 'static {
    /// Keeps one message. It belongs to the application's transaction.
    fn save<'a>(&'a self, topic: &'a str, key: Option<&'a str>, bytes: &'a [u8]) -> BoxFuture<'a, Result<(), Error>>;

    /// The oldest entries not yet sent, at most `limit`, in the order they were saved.
    fn pending(&self, limit: usize) -> BoxFuture<'_, Result<Vec<OutboxEntry>, Error>>;

    /// Marks an entry as sent.
    fn done<'a>(&'a self, entry: &'a OutboxEntry) -> BoxFuture<'a, Result<(), Error>>;
}

/// An outbox in memory, for tests: the ids are `"1"`, `"2"`, ... in the order of the saves,
/// `pending(limit)` is the first `limit` entries and `done` removes one.
#[derive(Debug, Default)]
pub struct MemoryOutbox {
    state: Mutex<(u64, Vec<OutboxEntry>)>,
}

impl MemoryOutbox {
    /// An empty outbox.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many entries are pending.
    #[must_use]
    pub fn size(&self) -> usize {
        lock(&self.state).1.len()
    }

    /// The pending entries, oldest first.
    #[must_use]
    pub fn entries(&self) -> Vec<OutboxEntry> {
        lock(&self.state).1.clone()
    }
}

impl Outbox for MemoryOutbox {
    fn save<'a>(&'a self, topic: &'a str, key: Option<&'a str>, bytes: &'a [u8]) -> BoxFuture<'a, Result<(), Error>> {
        let mut state = lock(&self.state);
        state.0 += 1;
        let id = state.0.to_string();
        state.1.push(OutboxEntry { id, topic: topic.to_string(), key: key.map(str::to_string), bytes: bytes.to_vec() });
        Box::pin(std::future::ready(Ok(())))
    }

    fn pending(&self, limit: usize) -> BoxFuture<'_, Result<Vec<OutboxEntry>, Error>> {
        let entries: Vec<OutboxEntry> = lock(&self.state).1.iter().take(limit).cloned().collect();
        Box::pin(std::future::ready(Ok(entries)))
    }

    fn done<'a>(&'a self, entry: &'a OutboxEntry) -> BoxFuture<'a, Result<(), Error>> {
        lock(&self.state).1.retain(|e| e.id != entry.id);
        Box::pin(std::future::ready(Ok(())))
    }
}

/// The names of the events a request wrote to its outbox, in order. The server keeps one per
/// request (`RequestContext::emitted`) so that it can check, after the handler, what was written;
/// the test handler answers with it.
#[derive(Debug, Clone, Default)]
pub struct EmitLog {
    names: Arc<Mutex<Vec<&'static str>>>,
    /// Set by a test handler: the events go to an outbox of the request's own.
    scratch: bool,
}

impl EmitLog {
    /// A log for a request; `scratch` makes `emit` write to a memory outbox of its own instead of
    /// the application's (only a test handler does).
    #[doc(hidden)]
    #[must_use]
    pub fn new(scratch: bool) -> Self {
        EmitLog { names: Arc::default(), scratch }
    }

    fn record(&self, name: &'static str) {
        lock(&self.names).push(name);
    }

    /// Replaces what was written: the events the edge taken emits, for a test handler that does not
    /// call the application's handler.
    #[doc(hidden)]
    pub fn set(&self, names: &[&'static str]) {
        *lock(&self.names) = names.to_vec();
    }

    /// What was written, in order.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        lock(&self.names).clone()
    }

    /// The text of `Veld-Test-Emitted`: the names separated by `,`, or `None` when nothing was written.
    #[doc(hidden)]
    #[must_use]
    pub fn header(&self) -> Option<String> {
        let names = self.names();
        (!names.is_empty()).then(|| names.join(","))
    }
}

/// What the check after a handler says (`tests/wire/events/outbox.json`, `written`): `ok`, or
/// `missing E`, `twice E` or `unexpected E` for the first fault: a missing event first (in the
/// order of `expected`), then a repeated one, then an unexpected one (in the order written).
#[must_use]
pub fn check_written(expected: &[&str], written: &[&str]) -> String {
    let count = |e: &str| written.iter().filter(|w| **w == e).count();
    for e in expected {
        match count(e) {
            0 => return format!("missing {e}"),
            1 => {}
            _ => return format!("twice {e}"),
        }
    }
    match written.iter().find(|w| !expected.contains(w)) {
        Some(w) => format!("unexpected {w}"),
        None => "ok".to_string(),
    }
}

/// The check after the handler of a move that emits, for a success: every event of the edge was
/// written once and nothing else. A fault is an `Error::Other`, so the server answers `500 Internal`
/// and reports it to `on_error`; what was already saved stays in the outbox, which is why the
/// application's transaction has to end with the failure.
///
/// # Errors
/// `Error::Other` that names the endpoint and the first fault.
#[doc(hidden)]
pub fn check_emitted(endpoint: &str, expected: &[&str], log: &EmitLog) -> Result<(), Error> {
    match check_written(expected, &log.names()).as_str() {
        "ok" => Ok(()),
        fault => Err(Error::other(format!("{endpoint}: the move wrote the wrong events to the outbox: {fault}"))),
    }
}

/// The wire value of the state a move started from, as step 6 handed it over in `args["transition"]`.
#[doc(hidden)]
#[must_use]
pub fn transition_from(args: &Map<String, Value>) -> &str {
    args.get("transition").and_then(|t| t.get("from")).and_then(Value::as_str).unwrap_or_default()
}

/// A transport that sends nothing: `emit` shares the publisher's context, and its transport is never
/// asked.
struct NoTransport;

impl Transport for NoTransport {
    fn publish<'a>(&'a self, _topic: &'a str, _key: Option<&'a str>, _bytes: &'a [u8], _options: ChannelOptions) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(std::future::ready(Err(Error::other("veld events: an outbox sends nothing"))))
    }

    fn subscribe<'a>(&'a self, _topic: &'a str, _group: &'a str, _deliver: Deliver, _options: ChannelOptions) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>> {
        Box::pin(std::future::ready(Err(Error::other("veld events: an outbox sends nothing"))))
    }
}

/// Where an [`Emission`] saves: the application's outbox, or the request's own in a test handler.
enum Sink<'a> {
    Outbox(&'a dyn Outbox),
    Scratch(MemoryOutbox),
}

/// What the handler of a move that emits writes through (`<Endpoint>Emit` wraps one): the publisher's
/// pipeline of steps 1 to 5, then `Outbox::save`. Nothing is sent to a transport.
pub struct Emission<'a> {
    ctx: PublisherContext,
    sink: Sink<'a>,
    log: EmitLog,
}

impl<'a> Emission<'a> {
    /// Writes to `outbox` (or, for a test handler, to an outbox of its own), with the id source,
    /// the clock and the hooks of `options`, and tells `log` the name of every event written.
    #[must_use]
    pub fn new<O: Outbox>(outbox: &'a O, (source, validation_sig): (&'static str, &'static str), options: PublisherOptions, log: &EmitLog) -> Self {
        let sink = if log.scratch { Sink::Scratch(MemoryOutbox::new()) } else { Sink::Outbox(outbox) };
        Emission { ctx: PublisherContext::new(Arc::new(NoTransport), source, validation_sig, options), sink, log: log.clone() }
    }

    /// Steps 1 to 5 of publishing and then `save`: the same checks as a publisher, the same bytes.
    ///
    /// # Errors
    /// The `ValidationError` of a value that does not hold, nothing is written then; the outbox's
    /// own error.
    pub async fn emit<E: Serialize + Sync>(&self, channel: &ChannelDef, def: &EventDef, event: &E) -> Result<PublishedEvent, Error> {
        let (published, bytes) = build_message(&self.ctx, channel, def, event).await?;
        let key = published.subject.as_deref();
        match &self.sink {
            Sink::Outbox(outbox) => outbox.save(channel.topic, key, &bytes).await?,
            Sink::Scratch(outbox) => outbox.save(channel.topic, key, &bytes).await?,
        }
        self.log.record(def.name);
        Ok(published)
    }
}

/// The options of [`relay_outbox`]: how many entries one run takes, and what hears a failure.
#[derive(Clone)]
pub struct RelayOptions {
    limit: usize,
    on_error: Option<Arc<dyn Fn(&Error, Option<&OutboxEntry>) + Send + Sync>>,
}

impl Default for RelayOptions {
    fn default() -> Self {
        RelayOptions { limit: 100, on_error: None }
    }
}

impl RelayOptions {
    /// A run takes up to 100 entries and reports nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many entries one run takes at most (default 100).
    #[must_use]
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Hears the failure that stopped a run: the entry that failed (`None` when `pending` itself
    /// failed). A hook that panics is ignored.
    #[must_use]
    pub fn on_error(mut self, f: impl Fn(&Error, Option<&OutboxEntry>) + Send + Sync + 'static) -> Self {
        self.on_error = Some(Arc::new(f));
        self
    }
}

/// The relay (section 11.1): takes up to `limit` pending entries, publishes each with the options of
/// its channel (`channels` is by topic; a topic nobody knows gets none), in the order `pending` gave
/// them, and calls `done` after the transport accepted it. It stops at the first entry that fails,
/// so the order of a key is kept, and answers how many it sent: an entry whose `done` failed is
/// counted and stays pending, so it is sent again (delivery is at least once). The failure goes to
/// the `on_error` of `options`.
pub async fn relay_outbox(
    (outbox, transport): (&dyn Outbox, &dyn Transport),
    channels: &[(&str, ChannelOptions)],
    options: &RelayOptions,
) -> usize {
    let report = |error: &Error, entry: Option<&OutboxEntry>| {
        if let Some(f) = &options.on_error {
            let _ = catch_unwind(AssertUnwindSafe(|| f(error, entry)));
        }
    };
    let entries = match outbox.pending(options.limit).await {
        Ok(entries) => entries,
        Err(error) => {
            report(&error, None);
            return 0;
        }
    };
    let mut sent = 0;
    for entry in entries.iter().take(options.limit) {
        let channel = channels.iter().find(|(topic, _)| *topic == entry.topic).map_or(ChannelOptions::NONE, |(_, o)| *o);
        if let Err(error) = transport.publish(&entry.topic, entry.key.as_deref(), &entry.bytes, channel).await {
            report(&error, Some(entry));
            return sent;
        }
        sent += 1;
        if let Err(error) = outbox.done(entry).await {
            report(&error, Some(entry));
            return sent;
        }
    }
    sent
}
// @end

// ---------------------------------------------------------------------------
// Consuming (section 8)
// ---------------------------------------------------------------------------

/// Why a message is rejected (section 8): it can never succeed for this consumer, so it is
/// acknowledged and reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Malformed,
    Shape,
    Rule,
}

impl Reason {
    /// `malformed`, `shape` or `rule`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Malformed => "malformed",
            Reason::Shape => "shape",
            Reason::Rule => "rule",
        }
    }
}

/// A rejected message, as `on_event_rejected` hears it.
#[derive(Debug, Clone)]
pub struct EventRejected {
    pub reason: Reason,
    /// What was wrong, as text.
    pub message: String,
    pub topic: String,
    pub group: String,
    /// The name of the event, when the `type` of the message names one this subscription handles.
    pub event: Option<String>,
    pub issues: Vec<Issue>,
    pub violations: Vec<Violation>,
    /// The message as the transport delivered it.
    pub bytes: Vec<u8>,
}

/// The consumer and the publisher were generated from different versions of the event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractMismatch {
    pub event: String,
    /// The signature and the rules digest this consumer was generated with.
    pub own_signature: String,
    pub own_digest: Option<String>,
    /// The signature and the rules digest the message carries.
    pub signature: String,
    pub digest: Option<String>,
}

/// A handler failed: the message is not acknowledged and the transport delivers it again by its
/// own policy.
#[derive(Debug, Clone)]
pub struct HandlerFailure {
    pub event: String,
    pub topic: String,
    pub group: String,
    pub meta: EventMeta,
    pub error: Error,
    /// The delivery attempt of this run: `meta.attempt`.
    pub attempt: u32,
    /// True when this consumer will not handle the message again (section 10.4): the last run of a
    /// channel with `@retry`. Always false on a channel without `@retry`, where the transport
    /// delivers the message again. (`final` in the spec; a keyword in Rust.)
    pub is_final: bool,
}

/// The hooks of a consumer (`spec/events.md` section 8.1). All of them are optional, and a hook
/// that panics changes nothing.
#[derive(Clone, Default)]
pub struct ConsumerOptions {
    on_event_rejected: Option<Hook<EventRejected>>,
    on_contract_mismatch: Option<Hook<ContractMismatch>>,
    on_handler_error: Option<Hook<HandlerFailure>>,
    inbox: Option<Arc<dyn Inbox>>,
    sleep: Option<SleepFn>,
    retry_delay_ms: Option<u64>,
    retry_max_delay_ms: Option<u64>,
    // #if rules
    on_rule_violation: Option<RuleHook>,
    // #endif
    // @if event_flags
    flags: Option<FlagReader>,
    on_flag_error: Option<Hook<Error>>,
    // @end
}

impl ConsumerOptions {
    /// No hook.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A rejected message (it is acknowledged). Without this hook, and without a dead letter, it
    /// is gone.
    #[must_use]
    pub fn on_event_rejected(mut self, f: impl Fn(&EventRejected) + Send + Sync + 'static) -> Self {
        self.on_event_rejected = Some(Arc::new(f));
        self
    }

    /// Once per distinct pair of signature and digest this consumer had not seen.
    #[must_use]
    pub fn on_contract_mismatch(mut self, f: impl Fn(&ContractMismatch) + Send + Sync + 'static) -> Self {
        self.on_contract_mismatch = Some(Arc::new(f));
        self
    }

    /// A run of a handler that failed.
    #[must_use]
    pub fn on_handler_error(mut self, f: impl Fn(&HandlerFailure) + Send + Sync + 'static) -> Self {
        self.on_handler_error = Some(Arc::new(f));
        self
    }

    /// Step 5 of section 8: a message whose `source` and `id` were handled is acknowledged and
    /// skipped.
    #[must_use]
    pub fn inbox(mut self, inbox: Arc<dyn Inbox>) -> Self {
        self.inbox = Some(inbox);
        self
    }

    /// How the consumer waits before a run of `@retry` (section 10.4): a function from
    /// milliseconds to a future. The default is a timer thread, which needs no async runtime; a
    /// test passes one that does not wait.
    #[must_use]
    pub fn sleep(mut self, f: impl Fn(u64) -> BoxFuture<'static, ()> + Send + Sync + 'static) -> Self {
        self.sleep = Some(Arc::new(f));
        self
    }

    /// The wait before the second run (default 1000).
    #[must_use]
    pub fn retry_delay_ms(mut self, ms: u64) -> Self {
        self.retry_delay_ms = Some(ms);
        self
    }

    /// No wait is longer than this (default 60000).
    #[must_use]
    pub fn retry_max_delay_ms(mut self, ms: u64) -> Self {
        self.retry_max_delay_ms = Some(ms);
        self
    }
    // #if rules

    /// The violations of invariants of a message: the enforced ones that reject it and the audited
    /// ones.
    #[must_use]
    pub fn on_rule_violation(mut self, f: impl Fn(&super::rules::RuleEvent) + Send + Sync + 'static) -> Self {
        self.on_rule_violation = Some(Arc::new(f));
        self
    }
    // #endif
    // @if event_flags

    /// The provider of the flags of the invariants (`spec/platform.md` section 11): read once per
    /// message, with no principal. Without one the flagged invariants are not evaluated.
    #[must_use]
    pub fn flags<P: FlagProvider>(mut self, provider: Arc<P>) -> Self {
        self.flags = Some(flag_reader(provider));
        self
    }

    /// Hears a provider that failed: its flags count as on.
    #[must_use]
    pub fn on_flag_error(mut self, f: impl Fn(&Error) + Send + Sync + 'static) -> Self {
        self.on_flag_error = Some(Arc::new(f));
        self
    }
    // @end
}

/// The function a consumer waits with: milliseconds to a future.
pub type SleepFn = Arc<dyn Fn(u64) -> BoxFuture<'static, ()> + Send + Sync>;

/// The application's record of the messages it has handled (`spec/events.md` section 10.3).
/// `handle` runs `run` and records `(source, id)`, or does nothing when the pair is recorded, and
/// answers whether it ran `run`. When `run` fails the pair MUST NOT be recorded and the failure is
/// the result of `handle`. An implementation that records the pair in the transaction of the
/// handler's own writes prevents a duplicated business effect; one that cannot narrows the window
/// and does not close it.
pub trait Inbox: Send + Sync {
    fn handle<'a>(
        &'a self,
        source: &'a str,
        id: &'a str,
        run: BoxFuture<'a, Result<(), Error>>,
    ) -> BoxFuture<'a, Result<bool, Error>>;
}

/// An inbox that lives in the process, for tests. It does not serialise two concurrent calls with
/// one pair, because a group of the memory transport delivers one message at a time.
#[derive(Debug, Default)]
pub struct MemoryInbox {
    seen: Mutex<BTreeSet<(String, String)>>,
}

impl MemoryInbox {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True when the pair was handled.
    #[must_use]
    pub fn has(&self, source: &str, id: &str) -> bool {
        lock(&self.seen).contains(&(source.to_string(), id.to_string()))
    }

    /// How many pairs are recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.seen).len()
    }

    /// True when no pair is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Inbox for MemoryInbox {
    fn handle<'a>(
        &'a self,
        source: &'a str,
        id: &'a str,
        run: BoxFuture<'a, Result<(), Error>>,
    ) -> BoxFuture<'a, Result<bool, Error>> {
        Box::pin(async move {
            if self.has(source, id) {
                return Ok(false);
            }
            run.await?;
            lock(&self.seen).insert((source.to_string(), id.to_string()));
            Ok(true)
        })
    }
}

/// `@retry(max: n, backoff: fixed | exponential)` of a channel (section 10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max: u32,
    pub backoff: Backoff,
}

/// The `backoff` of a [`RetryPolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backoff {
    Fixed,
    Exponential,
}

/// What a subscription gets from the annotations of its channel (`@retry`, `@deadLetter`,
/// `@ordered`). A channel without any has [`Delivery::NONE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delivery {
    /// The consumer runs the handler again by itself (section 10.4).
    pub retry: Option<RetryPolicy>,
    /// Rejected messages and used-up retries are copied to `<topic>.dlq` (section 10.5).
    pub dead_letter: bool,
    /// Passed to the transport (section 10.6).
    pub ordered: bool,
}

impl Delivery {
    /// No annotation.
    pub const NONE: Delivery = Delivery { retry: None, dead_letter: false, ordered: false };
}

/// Why a message is copied to `<topic>.dlq`: the three rejections, or the used-up retries of a
/// handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadLetterReason {
    Malformed,
    Shape,
    Rule,
    Handler,
}

impl DeadLetterReason {
    /// `malformed`, `shape`, `rule` or `handler`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            DeadLetterReason::Malformed => "malformed",
            DeadLetterReason::Shape => "shape",
            DeadLetterReason::Rule => "rule",
            DeadLetterReason::Handler => "handler",
        }
    }
}

impl From<Reason> for DeadLetterReason {
    fn from(r: Reason) -> Self {
        match r {
            Reason::Malformed => DeadLetterReason::Malformed,
            Reason::Shape => DeadLetterReason::Shape,
            Reason::Rule => DeadLetterReason::Rule,
        }
    }
}

/// The members a copy adds. A member of the original with one of these names is replaced or
/// removed.
const COPY_MEMBERS: [&str; 5] = ["veldreason", "veldgroup", "veldattempts", "veldissues", "veldviolations"];

/// The bytes of the dead-letter copy of `original` (section 10.5). When the original is a JSON
/// object, the copy is that object with `veldreason`, `veldgroup`, `veldattempts` (the runs of the
/// handler, 0 for a rejected message) and, when not empty, `veldissues` and `veldviolations`, each
/// a string that holds the canonical JSON of its list, written as canonical JSON. Anything else
/// is returned as it is. Nothing of a handler's error is in a copy.
#[must_use]
pub fn dead_letter_copy(
    original: &[u8],
    reason: DeadLetterReason,
    group: &str,
    attempts: u32,
    issues: &[Issue],
    violations: &[Violation],
) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(original) else {
        return original.to_vec();
    };
    let Ok(Value::Object(mut copy)) = serde_json::from_str::<Value>(text) else {
        return original.to_vec();
    };
    for name in COPY_MEMBERS {
        copy.remove(name);
    }
    copy.insert("veldreason".into(), Value::from(reason.as_str()));
    copy.insert("veldgroup".into(), Value::from(group));
    copy.insert("veldattempts".into(), Value::from(attempts));
    if !issues.is_empty() {
        let list: Vec<Value> = issues
            .iter()
            .map(|i| serde_json::json!({ "path": i.path, "code": i.code, "message": i.message }))
            .collect();
        copy.insert("veldissues".into(), Value::from(canonical(&Value::Array(list))));
    }
    if !violations.is_empty() {
        let list: Vec<Value> = violations.iter().map(violation_json).collect();
        copy.insert("veldviolations".into(), Value::from(canonical(&Value::Array(list))));
    }
    canonical_bytes(&Value::Object(copy))
}

fn violation_json(v: &Violation) -> Value {
    let mut m = Map::new();
    m.insert("rule".into(), Value::from(v.rule.as_str()));
    m.insert("path".into(), Value::from(v.path.as_str()));
    m.insert("fields".into(), Value::from(v.fields.clone()));
    m.insert("message".into(), Value::from(v.message.as_str()));
    if let Some(e) = &v.error {
        m.insert("error".into(), Value::from(e.as_str()));
    }
    Value::Object(m)
}

/// What a handler did.
pub enum Run {
    Done,
    /// The handler returned an error: the message is not acknowledged.
    Failed(Error),
    /// The value passed validation and still could not be read as the event's type.
    Undecodable(String),
}

type RunFn = Arc<dyn Fn(Value, EventMeta) -> BoxFuture<'static, Run> + Send + Sync>;

/// One `on` line: the event and the call into the application's handler.
#[derive(Clone)]
pub struct EventBinding {
    def: &'static EventDef,
    run: RunFn,
}

impl EventBinding {
    /// Binds `f` to the event described by `def`; the typed value is read from the wire form that
    /// passed the checks.
    pub fn typed<E, F, Fut>(def: &'static EventDef, f: F) -> EventBinding
    where
        E: DeserializeOwned + Send + 'static,
        F: Fn(E, EventMeta) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), Error>> + Send + 'static,
    {
        let run: RunFn = Arc::new(move |typed, meta| match serde_json::from_value::<E>(typed) {
            Ok(event) => {
                let future = f(event, meta);
                Box::pin(async move {
                    match future.await {
                        Ok(()) => Run::Done,
                        Err(e) => Run::Failed(e),
                    }
                })
            }
            Err(e) => Box::pin(std::future::ready(Run::Undecodable(e.to_string()))),
        });
        EventBinding { def, run }
    }
}

/// One subscription of a service.
pub struct SubscriptionDef {
    pub topic: &'static str,
    pub group: &'static str,
    pub on: Vec<EventBinding>,
}

/// The handle `subscribe_events` returns: `close` stops every subscription.
pub struct SubscriptionHandle {
    opened: Vec<Box<dyn TransportSubscription>>,
}

impl SubscriptionHandle {
    /// Stops the subscriptions.
    pub async fn close(self) {
        self.close_all().await;
    }

    async fn close_all(&self) {
        for h in &self.opened {
            h.close().await;
        }
    }
}

struct Envelope {
    id: String,
    source: String,
    r#type: String,
    subject: Option<String>,
    time: Option<String>,
    veldrules: Option<String>,
    // @if tenant_events
    veldtenant: Option<String>,
    // @end
    data: Value,
}

/// A string attribute that may be absent.
fn optional_text(m: &Map<String, Value>, name: &str) -> Result<Option<String>, String> {
    match m.get(name) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("`{name}` is not a string")),
    }
}

/// A string attribute that must be there and not empty.
fn required_text(m: &Map<String, Value>, name: &str) -> Result<String, String> {
    match m.get(name) {
        Some(Value::String(s)) if !s.is_empty() => Ok(s.clone()),
        _ => Err(format!("`{name}` is missing, empty or not a string")),
    }
}

/// Step 1: the envelope, or the reason it is malformed.
fn parse_envelope(bytes: &[u8]) -> Result<Envelope, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "the message is not UTF-8 JSON".to_string())?;
    let Ok(Value::Object(m)) = serde_json::from_str::<Value>(text) else {
        return Err("the message is not a JSON object".to_string());
    };
    if m.get("specversion") != Some(&Value::from("1.0")) {
        return Err("`specversion` is not \"1.0\"".to_string());
    }
    let (id, source, r#type) = (required_text(&m, "id")?, required_text(&m, "source")?, required_text(&m, "type")?);
    let (subject, time) = (optional_text(&m, "subject")?, optional_text(&m, "time")?);
    let (content_type, veldrules) = (optional_text(&m, "datacontenttype")?, optional_text(&m, "veldrules")?);
    if let Some(ct) = content_type.filter(|c| c != "application/json") {
        return Err(format!("`datacontenttype` is `{ct}`"));
    }
    if m.contains_key("data_base64") {
        return Err("`data_base64` is not allowed".to_string());
    }
    // @if tenant_events
    let veldtenant = optional_text(&m, "veldtenant")?;
    // @end
    match m.get("data") {
        // @if !tenant_events
        Some(data @ Value::Object(_)) => Ok(Envelope { id, source, r#type, subject, time, veldrules, data: data.clone() }),
        // @end
        // @if tenant_events
        Some(data @ Value::Object(_)) => Ok(Envelope { id, source, r#type, subject, time, veldrules, veldtenant, data: data.clone() }),
        // @end
        _ => Err("`data` is not an object".to_string()),
    }
}

struct Consumer {
    transport: Arc<dyn Transport>,
    sub: SubscriptionDef,
    delivery: Delivery,
    options: ConsumerOptions,
    seen: Arc<Mutex<BTreeSet<String>>>,
}

/// What a rejection reports besides its reason.
#[derive(Default)]
struct Rejection {
    message: String,
    event: Option<String>,
    issues: Vec<Issue>,
    violations: Vec<Violation>,
}

/// Publishes the copy of a message to `<topic>.dlq` when the channel has `@deadLetter`: `Again` when
/// that fails (section 10.5).
async fn copy_to_dead_letter(
    c: &Consumer,
    message: &TransportMessage,
    reason: DeadLetterReason,
    attempts: u32,
    (issues, violations): (&[Issue], &[Violation]),
) -> Outcome {
    if !c.delivery.dead_letter {
        return Outcome::Done;
    }
    let bytes = dead_letter_copy(&message.bytes, reason, c.sub.group, attempts, issues, violations);
    let topic = format!("{}.dlq", c.sub.topic);
    match c.transport.publish(&topic, message.key.as_deref(), &bytes, ChannelOptions::NONE).await {
        Ok(()) => Outcome::Done,
        Err(_) => Outcome::Again,
    }
}

/// A rejected message (steps 1, 3 and 4) is copied when the channel has `@deadLetter`, then
/// acknowledged and reported (the hook after the copy was decided, whether it succeeded or not).
async fn reject(c: &Consumer, message: &TransportMessage, reason: Reason, detail: Rejection) -> Outcome {
    let outcome = copy_to_dead_letter(c, message, reason.into(), 0, (&detail.issues, &detail.violations)).await;
    let rejected = EventRejected {
        reason,
        message: detail.message,
        topic: c.sub.topic.to_string(),
        group: c.sub.group.to_string(),
        event: detail.event,
        issues: detail.issues,
        violations: detail.violations,
        bytes: message.bytes.clone(),
    };
    call_hook(c.options.on_event_rejected.as_ref(), &rejected);
    outcome
}

/// Tells the application once per distinct pair that the publisher is on another version.
fn note_mismatch(c: &Consumer, def: &EventDef, env: &Envelope) {
    let digest = env.veldrules.as_deref();
    if env.r#type == def.sig && digest == def.digest {
        return;
    }
    let pair = format!("{}\u{0}{}\u{0}{}", def.name, env.r#type, digest.unwrap_or(""));
    if !lock(&c.seen).insert(pair) {
        return;
    }
    let mismatch = ContractMismatch {
        event: def.name.to_string(),
        own_signature: def.sig.to_string(),
        own_digest: def.digest.map(str::to_string),
        signature: env.r#type.clone(),
        digest: env.veldrules.clone(),
    };
    call_hook(c.options.on_contract_mismatch.as_ref(), &mismatch);
}

/// Steps 3 and 4: the shape, then the invariants when the digests agree. `None`: the message goes
/// on.
async fn decode(c: &Consumer, message: &TransportMessage, def: &EventDef, env: &Envelope) -> Option<Outcome> {
    let issues = def.schema.validate(&env.data, "$", false);
    if !issues.is_empty() {
        let text: Vec<String> = issues.iter().map(|i| format!("{} {}", i.path, i.message)).collect();
        let detail = Rejection { message: text.join("; "), event: Some(def.name.to_string()), issues, ..Rejection::default() };
        return Some(reject(c, message, Reason::Shape, detail).await);
    }
    // #if rules
    let hook = c.options.on_rule_violation.as_ref();
    let violations = if env.veldrules.as_deref() == def.digest {
        // @if event_flags
        let readings = read_flags(c.options.flags.as_ref(), c.options.on_flag_error.as_ref(), def).await;
        super::rules::with_view(readings.as_ref(), || run_rules(def, &env.data, hook, super::rules::Position::Input))
        // @end
        // @if !event_flags
        run_rules(def, &env.data, hook, super::rules::Position::Input)
        // @end
    } else {
        // @if derived
        // The invariants are not run, but a derived field that cannot be computed means that the
        // consumer cannot build the value its handler is promised (`spec/rules.md` 17.2).
        let failed = super::rules::derive_violations(def.schema, &env.data, "$");
        report(hook, def, (super::rules::Position::Input, super::rules::Outcome::Rejected), &failed, 0);
        failed
        // @end
        // @if !derived
        Vec::new()
        // @end
    };
    if !violations.is_empty() {
        let text: Vec<&str> = violations.iter().map(|v| v.message.as_str()).collect();
        let detail = Rejection {
            message: text.join("; "),
            event: Some(def.name.to_string()),
            issues: violations.iter().map(Violation::issue).collect(),
            violations: violations.clone(),
        };
        return Some(reject(c, message, Reason::Rule, detail).await);
    }
    // #endif
    None
}

/// A timer without an async runtime: a thread sleeps and wakes the task.
struct ThreadSleep {
    ms: u64,
    state: Option<Arc<Mutex<(bool, Option<Waker>)>>>,
}

impl Future for ThreadSleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if let Some(state) = &self.state {
            let mut st = lock(state);
            if st.0 {
                return Poll::Ready(());
            }
            st.1 = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let state = Arc::new(Mutex::new((false, Some(cx.waker().clone()))));
        let theirs = Arc::clone(&state);
        let ms = self.ms;
        self.state = Some(state);
        let spawned = std::thread::Builder::new().spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            let waker = {
                let mut st = lock(&theirs);
                st.0 = true;
                st.1.take()
            };
            if let Some(w) = waker {
                w.wake();
            }
        });
        if spawned.is_err() {
            return Poll::Ready(());
        }
        Poll::Pending
    }
}

async fn wait(c: &Consumer, ms: u64) {
    match &c.options.sleep {
        Some(f) => f(ms).await,
        None => ThreadSleep { ms, state: None }.await,
    }
}

/// The wait after run `k` failed (`k` from 1): the delay for `fixed`, doubled each time for
/// `exponential`, never more than the maximum.
fn retry_wait(c: &Consumer, retry: RetryPolicy, k: u32) -> u64 {
    let delay = c.options.retry_delay_ms.unwrap_or(1000);
    let max = c.options.retry_max_delay_ms.unwrap_or(60_000);
    let wait = match retry.backoff {
        Backoff::Fixed => delay,
        Backoff::Exponential => delay.saturating_mul(1u64 << (k - 1).min(62)),
    };
    wait.min(max)
}

/// What one run of steps 5 and 6 came to.
enum Step {
    /// The handler returned, or the inbox skipped the message.
    Done,
    Failed(Error),
    Undecodable(String),
}

/// A future whose panic is a failed run.
struct GuardedRun(BoxFuture<'static, Run>);

impl Future for GuardedRun {
    type Output = Run;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Run> {
        let inner = self.0.as_mut();
        match catch_unwind(AssertUnwindSafe(|| inner.poll(cx))) {
            Ok(poll) => poll,
            Err(_) => Poll::Ready(Run::Failed(Error::other("veld events: the handler panicked"))),
        }
    }
}

/// Starts the handler; a panic while it builds its future is a failed run.
fn start(binding: &EventBinding, typed: Value, meta: EventMeta) -> GuardedRun {
    let run = Arc::clone(&binding.run);
    match catch_unwind(AssertUnwindSafe(|| run(typed, meta))) {
        Ok(future) => GuardedRun(future),
        Err(_) => GuardedRun(Box::pin(std::future::ready(Run::Failed(Error::other("veld events: the handler panicked"))))),
    }
}

/// Steps 5 and 6 of one run: the inbox, then the handler.
async fn run_once(c: &Consumer, binding: &EventBinding, env: &Envelope, typed: Value, meta: EventMeta) -> Step {
    let future = start(binding, typed, meta);
    let Some(inbox) = c.options.inbox.as_ref() else {
        return match future.await {
            Run::Done => Step::Done,
            Run::Failed(e) => Step::Failed(e),
            Run::Undecodable(text) => Step::Undecodable(text),
        };
    };
    let undecodable: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let slot = Arc::clone(&undecodable);
    let run: BoxFuture<'_, Result<(), Error>> = Box::pin(async move {
        match future.await {
            Run::Done => Ok(()),
            Run::Failed(e) => Err(e),
            Run::Undecodable(text) => {
                *lock(&slot) = Some(text.clone());
                Err(Error::other(text))
            }
        }
    });
    match inbox.handle(&env.source, &env.id, run).await {
        Ok(_) => Step::Done,
        Err(e) => match lock(&undecodable).take() {
            Some(text) => Step::Undecodable(text),
            None => Step::Failed(e),
        },
    }
}

/// The handler runs until it returns or, with `@retry(max: n)`, up to `n + 1` times, waiting between
/// the runs. A channel without `@retry` answers `Again` at the first failure; the last run of one
/// with `@retry` is copied when the channel has `@deadLetter` and acknowledged.
async fn handle(c: &Consumer, message: &TransportMessage, binding: &EventBinding, env: &Envelope, typed: &Value) -> Outcome {
    let allowed = c.delivery.retry.map_or(1, |r| r.max.saturating_add(1));
    let mut run = 1u32;
    loop {
        let meta = EventMeta {
            id: env.id.clone(),
            source: env.source.clone(),
            r#type: env.r#type.clone(),
            subject: env.subject.clone(),
            time: env.time.clone(),
            attempt: message.attempt.saturating_add(run - 1),
            // @if tenant_events
            veldtenant: env.veldtenant.clone(),
            // @end
        };
        let error = match run_once(c, binding, env, typed.clone(), meta.clone()).await {
            Step::Done => return Outcome::Done,
            Step::Undecodable(text) => {
                let detail = Rejection { message: text, event: Some(binding.def.name.to_string()), ..Rejection::default() };
                return reject(c, message, Reason::Shape, detail).await;
            }
            Step::Failed(error) => error,
        };
        let is_final = c.delivery.retry.is_some() && run == allowed;
        let failure = HandlerFailure {
            event: binding.def.name.to_string(),
            topic: c.sub.topic.to_string(),
            group: c.sub.group.to_string(),
            attempt: meta.attempt,
            meta,
            error,
            is_final,
        };
        call_hook(c.options.on_handler_error.as_ref(), &failure);
        let Some(retry) = c.delivery.retry else {
            return Outcome::Again;
        };
        if is_final {
            return copy_to_dead_letter(c, message, DeadLetterReason::Handler, run, (&[], &[])).await;
        }
        wait(c, retry_wait(c, retry, run)).await;
        run += 1;
    }
}

async fn consume(c: &Consumer, message: TransportMessage) -> Outcome {
    let env = match parse_envelope(&message.bytes) {
        Ok(env) => env,
        Err(why) => return reject(c, &message, Reason::Malformed, Rejection { message: why, ..Rejection::default() }).await,
    };
    let Some(binding) = c.sub.on.iter().find(|b| type_name(b.def.sig) == type_name(&env.r#type)) else {
        return Outcome::Done;
    };
    // @if derived
    // A consumer does not believe a derived value in the message: it computes its own.
    let env = Envelope { data: binding.def.schema.without_derived(&env.data), ..env };
    // @end
    if let Some(outcome) = decode(c, &message, binding.def, &env).await {
        return outcome;
    }
    note_mismatch(c, binding.def, &env);
    // @if !derived
    let typed = binding.def.schema.from_wire(&env.data, false);
    // @end
    // @if derived
    let mut typed = binding.def.schema.from_wire(&env.data, false);
    super::rules::derive_walk(binding.def.schema, &mut typed, "$", super::rules::Mode::Full, &mut Vec::new());
    // @end
    handle(c, &message, binding, &env, &typed).await
}

/// Connects the handlers of a service to a transport, one subscription per `subscribes` line, each
/// under its group (section 8). A message the subscription has no `on` line for is acknowledged and
/// ignored; a rejected one is acknowledged and reported to `on_event_rejected`; a handler that
/// fails leaves it unacknowledged. For channels without `@retry`, `@deadLetter` and `@ordered`.
///
/// # Errors
/// The transport's own error; the subscriptions made so far are closed.
pub async fn subscribe_events(
    transport: Arc<dyn Transport>,
    subs: Vec<SubscriptionDef>,
    options: ConsumerOptions,
) -> Result<SubscriptionHandle, Error> {
    subscribe_events_with(transport, subs.into_iter().map(|s| (s, Delivery::NONE)).collect(), options).await
}

/// [`subscribe_events`] with the annotations of each subscription's channel (section 10).
///
/// # Errors
/// The transport's own error; the subscriptions made so far are closed.
pub async fn subscribe_events_with(
    transport: Arc<dyn Transport>,
    subs: Vec<(SubscriptionDef, Delivery)>,
    options: ConsumerOptions,
) -> Result<SubscriptionHandle, Error> {
    let seen = Arc::new(Mutex::new(BTreeSet::new()));
    let mut handle = SubscriptionHandle { opened: Vec::new() };
    for (sub, delivery) in subs {
        let (topic, group) = (sub.topic, sub.group);
        let channel = ChannelOptions { ordered: delivery.ordered, dead_letter: delivery.dead_letter };
        let consumer = Arc::new(Consumer {
            transport: Arc::clone(&transport),
            sub,
            delivery,
            options: options.clone(),
            seen: Arc::clone(&seen),
        });
        let deliver: Deliver = Arc::new(move |message| {
            let consumer = Arc::clone(&consumer);
            Box::pin(async move { consume(&consumer, message).await })
        });
        match transport.subscribe(topic, group, deliver, channel).await {
            Ok(opened) => handle.opened.push(opened),
            Err(e) => {
                handle.close_all().await;
                return Err(e);
            }
        }
    }
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetimes_are_written_in_utc_with_the_fraction_only_when_it_is_not_zero() {
        assert_eq!(canon_datetime("2026-06-10T12:00:00Z").as_deref(), Some("2026-06-10T12:00:00Z"));
        assert_eq!(canon_datetime("2026-06-10T12:00:00.000Z").as_deref(), Some("2026-06-10T12:00:00Z"));
        assert_eq!(canon_datetime("2026-06-10T11:59:58.250Z").as_deref(), Some("2026-06-10T11:59:58.250Z"));
        assert_eq!(canon_datetime("2026-06-10T13:59:58.250+02:00").as_deref(), Some("2026-06-10T11:59:58.250Z"));
        assert_eq!(canon_datetime("2026-06-10T12:00:02.0079Z").as_deref(), Some("2026-06-10T12:00:02.007Z"));
        assert_eq!(canon_datetime("2024-12-31T23:30:00-01:00").as_deref(), Some("2025-01-01T00:30:00Z"));
        assert!(canon_datetime("2024-13-01T00:00:00Z").is_none());
        assert!(canon_datetime("yesterday").is_none());
    }

    #[test]
    fn an_id_is_a_version_4_uuid() {
        let a = random_id();
        let b = random_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(a.as_bytes().get(14), Some(&b'4'));
    }
}

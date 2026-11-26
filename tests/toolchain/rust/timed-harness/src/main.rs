//! Timed moves over the production handler (`spec/platform.md` section 10), generated from
//! `tests/toolchain/contract/platform_p2/timed.veld`: the due check at its boundary, the message of a
//! refusal, `timed_moves()` and the due-moves runner with a memory loader.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bookings_client::bookings::bookings::Status as ClientStatus;
use bookings_server::bookings::bookings::{Booking, Status};
use bookings_server::bookings::bookings_server::{
    ArchiveBookingFacts, ArchiveBookingRequest, BookingsDueLoaders, BookingsDueOptions, BookingsFactLoaders, BookingsHandler, BookingsHooks,
    BookingsService, ExpireBookingFacts, ExpireBookingRequest, run_bookings_due_moves,
};
use bookings_server::veld::Error;
use bookings_server::veld::rules::Transition;
use bookings_server::veld::server::{Principal, RequestContext};
use serde_json::{Value, json};

const DAY_MS: u64 = 86_400_000;
/// 2026-06-10T12:00:00Z.
const SINCE: u64 = 1_781_092_800;
const SINCE_TEXT: &str = "2026-06-10T12:00:00Z";

fn at(ms_after_since: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(SINCE) + Duration::from_millis(ms_after_since)
}

fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

#[derive(Default)]
struct Db {
    bookings: Mutex<BTreeMap<String, Booking>>,
    /// `<endpoint>:<id>:<principal>`, in order.
    calls: Mutex<Vec<String>>,
    /// The `before` the loader was asked for and the `limit`.
    asked: Mutex<Vec<(SystemTime, usize)>>,
    /// The booking whose handler fails.
    broken: Mutex<Option<String>>,
    /// The clock of the hooks, as milliseconds after `SINCE`.
    clock: Mutex<u64>,
}

fn put(db: &Db, n: u32, status: Status, since: &str) {
    db.bookings.lock().unwrap().insert(id(n), Booking { id: id(n), status, pending_since: since.into() });
}

struct Loaders(Arc<Db>);

impl BookingsFactLoaders for Loaders {
    async fn booking(&self, key: String, _ctx: &RequestContext) -> Result<Option<Booking>, Error> {
        Ok(self.0.bookings.lock().unwrap().get(&key).cloned())
    }
}

struct Impl(Arc<Db>);

fn who(ctx: &RequestContext) -> String {
    ctx.principal::<String>().cloned().unwrap_or_else(|| "none".into())
}

impl BookingsService for Impl {
    async fn expire_booking(
        &self,
        request: ExpireBookingRequest,
        facts: ExpireBookingFacts,
        transition: Transition<Status>,
        ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        self.0.calls.lock().unwrap().push(format!("expire:{}:{}", request.id, who(ctx)));
        if self.0.broken.lock().unwrap().as_deref() == Some(request.id.as_str()) {
            return Err(Error::other("the store is down"));
        }
        let mut booking = facts.booking;
        booking.status = transition.to;
        self.0.bookings.lock().unwrap().insert(booking.id.clone(), booking.clone());
        Ok(booking)
    }

    async fn archive_booking(
        &self,
        request: ArchiveBookingRequest,
        facts: ArchiveBookingFacts,
        transition: Transition<Status>,
        ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        self.0.calls.lock().unwrap().push(format!("archive:{}:{}", request.id, who(ctx)));
        Ok(Booking { status: transition.to, ..facts.booking })
    }
}

/// The hooks; `timer` says whether they give the timer a principal.
struct Hooks {
    db: Arc<Db>,
    timer: bool,
    errors: Arc<Mutex<Vec<String>>>,
}

impl BookingsHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<Principal> {
        Some(Principal::new(format!("user:{token}")))
    }

    async fn timer_principal<'a>(&'a self) -> Option<Principal> {
        self.timer.then(|| Principal::new("timer".to_string()))
    }

    fn clock(&self, _ctx: &RequestContext) -> SystemTime {
        at(*self.db.clock.lock().unwrap())
    }

    fn on_error(&self, err: &Error, _ctx: &RequestContext) {
        self.errors.lock().unwrap().push(format!("{err:?}"));
    }
}

/// The memory loader of the runner: every pending booking whose `since` is at or before `before`.
struct Due(Arc<Db>);

impl BookingsDueLoaders for Due {
    async fn expire_booking(&self, before: SystemTime, limit: usize, _ctx: &RequestContext) -> Result<Vec<ExpireBookingRequest>, Error> {
        self.0.asked.lock().unwrap().push((before, limit));
        let before_ms = before.duration_since(UNIX_EPOCH).unwrap().as_millis();
        let mut out = Vec::new();
        for b in self.0.bookings.lock().unwrap().values() {
            let since = b.pending_since.as_str();
            // The memory store reads its own rows: `SINCE_TEXT` is the one time it holds.
            let ms = if since == SINCE_TEXT { u128::from(SINCE) * 1000 } else { u128::MAX };
            if b.status == Status::Pending && ms <= before_ms {
                out.push(ExpireBookingRequest { id: b.id.clone() });
            }
        }
        // A booking that moved already is still offered: another request was first.
        for n in self.0.bookings.lock().unwrap().values().filter(|b| b.status == Status::Cancelled && b.pending_since == SINCE_TEXT) {
            out.push(ExpireBookingRequest { id: n.id.clone() });
        }
        Ok(out)
    }

    async fn archive_booking(&self, _before: SystemTime, _limit: usize, _ctx: &RequestContext) -> Result<Vec<ArchiveBookingRequest>, Error> {
        Ok(Vec::new())
    }
}

struct World {
    base: String,
    db: Arc<Db>,
    errors: Arc<Mutex<Vec<String>>>,
    handler: bookings_server::veld::server::Handler<
        bookings_server::bookings::bookings_server::BookingsDispatch<Impl, Hooks, Loaders>,
    >,
}

async fn serve(timer: bool) -> World {
    let db = Arc::new(Db::default());
    let errors: Arc<Mutex<Vec<String>>> = Arc::default();
    let hooks = Hooks { db: db.clone(), timer, errors: errors.clone() };
    let handler = BookingsHandler::create(Impl(db.clone()), hooks, Loaders(db.clone()), None).unwrap();
    let router = bookings_server::adapters::axum::router(handler.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    World { base, db, errors, handler }
}

struct Answer {
    status: u16,
    doc: Value,
}

async fn expire(w: &World, n: u32, route: &str) -> Answer {
    let url = format!("{}/bookings/{}/{route}", w.base, id(n));
    let mut req = reqwest::Client::new().post(url);
    if route == "expire" {
        req = req.header("authorization", "Bearer t");
    }
    let response = req.send().await.unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    Answer { status, doc: serde_json::from_str(&text).unwrap_or(Value::Null) }
}

fn set_clock(w: &World, ms_after_since: u64) {
    *w.db.clock.lock().unwrap() = ms_after_since;
}

// ---- the guard ------------------------------------------------------------------------------------

/// A value that has waited exactly the duration is due, one millisecond less is not, and a `since` in
/// the future or one that is not a datetime is not due; the refusal is `409 InvalidTransition` with
/// the message `violates rule transition: not due` (`spec/platform.md` section 10).
async fn the_due_check_at_its_boundary() {
    let w = serve(true).await;
    put(&w.db, 1, Status::Pending, SINCE_TEXT);
    set_clock(&w, DAY_MS);
    assert_eq!(expire(&w, 1, "expire").await.status, 200, "exactly due");
    put(&w.db, 2, Status::Pending, SINCE_TEXT);
    set_clock(&w, DAY_MS - 1);
    let early = expire(&w, 2, "expire").await;
    assert_eq!(early.status, 409, "one millisecond early: {}", early.doc);
    assert_eq!(early.doc["data"]["field"], json!("status"));
    assert_eq!(early.doc["data"]["from"], json!("pending"));
    assert_eq!(early.doc["data"]["to"], json!("cancelled"));
    let message = early.doc["violations"][0]["message"].as_str().unwrap_or_default();
    assert_eq!(message, "violates rule transition: not due", "{}", early.doc);
    // The state check comes first: a booking that is not pending is refused without the due text.
    put(&w.db, 3, Status::Paid, SINCE_TEXT);
    let paid = expire(&w, 3, "expire").await;
    assert_eq!(paid.status, 409);
    assert_eq!(paid.doc["violations"][0]["message"], json!("violates rule transition"));
    // A `since` in the future, and one that cannot be read.
    put(&w.db, 4, Status::Pending, "2026-06-10T12:00:00.001Z");
    set_clock(&w, DAY_MS);
    assert_eq!(expire(&w, 4, "expire").await.status, 409, "since in the future of the due time");
    put(&w.db, 5, Status::Pending, "2999-01-01T00:00:00Z");
    assert_eq!(expire(&w, 5, "expire").await.status, 409, "since far in the future");
    put(&w.db, 6, Status::Pending, "not a datetime");
    assert_eq!(expire(&w, 6, "expire").await.status, 409, "a since that is not a datetime");
}

/// `timed_moves()` is on the machine, in the server and in the client.
fn the_machine_lists_its_timed_edges() {
    assert_eq!(Status::timed_moves(), &[(Status::Pending, Status::Cancelled, 86_400_000)]);
    assert_eq!(ClientStatus::timed_moves(), &[(ClientStatus::Pending, ClientStatus::Cancelled, 86_400_000)]);
    // A timed edge is an edge like any other.
    assert_eq!(Status::Pending.next_states(), &[Status::Paid, Status::Cancelled]);
}

// ---- the runner -------------------------------------------------------------------------------------

/// The runner asks the loader for what entered the state `after` before `now`, runs each answer
/// through the pipeline as the timer's principal (no `Authorization` header is involved), counts a
/// move another request made first as refused and a failing handler as failed, and one failure does
/// not stop the others.
async fn the_runner_runs_the_moves_that_are_due() {
    let w = serve(true).await;
    for n in 1..=3 {
        put(&w.db, n, Status::Pending, SINCE_TEXT);
    }
    // Booking 4 has moved already (another request was first): the loader still offers it.
    put(&w.db, 4, Status::Cancelled, SINCE_TEXT);
    *w.db.broken.lock().unwrap() = Some(id(2));
    let mut options = BookingsDueOptions::new(Due(w.db.clone()));
    options.now = Some(at(DAY_MS));
    let report = run_bookings_due_moves(&w.handler, &options).await.unwrap();
    let counts = report["expireBooking"];
    assert_eq!((counts.ran, counts.moved, counts.refused, counts.failed), (4, 2, 1, 1), "{counts:?}");
    let asked = w.db.asked.lock().unwrap().clone();
    assert_eq!(asked, vec![(at(0), 100)], "before = now - 24 h, the default limit");
    let calls = w.db.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 3, "the refused move never reached the handler: {calls:?}");
    assert!(calls.iter().all(|c| c.ends_with(":timer")), "the handler sees the timer's principal: {calls:?}");
    let status = |n: u32| w.db.bookings.lock().unwrap()[&id(n)].status;
    assert_eq!((status(1), status(2), status(3)), (Status::Cancelled, Status::Pending, Status::Cancelled));
    // The failing handler's error reached the hook: it became `Internal`, not an `InvalidTransition`.
    assert_eq!(w.errors.lock().unwrap().len(), 1, "{:?}", w.errors.lock().unwrap());
    // `archiveBooking` has no auth and no loader answer: an empty entry.
    assert_eq!(report["archiveBooking"].ran, 0);
}

/// `limit` bounds the calls per endpoint, and a run before anything is due runs nothing.
async fn the_runner_honours_the_limit_and_the_clock() {
    let w = serve(true).await;
    for n in 1..=4 {
        put(&w.db, n, Status::Pending, SINCE_TEXT);
    }
    let mut options = BookingsDueOptions::new(Due(w.db.clone()));
    options.now = Some(at(DAY_MS - 1));
    options.limit = 2;
    let report = run_bookings_due_moves(&w.handler, &options).await.unwrap();
    assert_eq!(report["expireBooking"].ran, 0, "nothing was due a millisecond early");
    options.now = Some(at(DAY_MS));
    let report = run_bookings_due_moves(&w.handler, &options).await.unwrap();
    assert_eq!((report["expireBooking"].ran, report["expireBooking"].moved), (2, 2), "limit 2");
    // Without `now` the clock of the hooks is used.
    set_clock(&w, DAY_MS);
    options.now = None;
    options.limit = 100;
    let report = run_bookings_due_moves(&w.handler, &options).await.unwrap();
    assert_eq!(report["expireBooking"].moved, 2);
}

/// A server whose timed endpoint has auth and whose hooks give the timer no principal does not
/// start the run: an error, and nothing was asked of the loader or run.
async fn a_run_without_the_timer_principal_does_not_start() {
    let w = serve(false).await;
    put(&w.db, 1, Status::Pending, SINCE_TEXT);
    let mut options = BookingsDueOptions::new(Due(w.db.clone()));
    options.now = Some(at(DAY_MS));
    let err = run_bookings_due_moves(&w.handler, &options).await.unwrap_err();
    assert!(format!("{err:?}").contains("timer_principal is required"), "{err:?}");
    assert!(w.db.asked.lock().unwrap().is_empty());
    assert!(w.db.calls.lock().unwrap().is_empty());
}

#[tokio::main]
async fn main() {
    the_due_check_at_its_boundary().await;
    the_machine_lists_its_timed_edges();
    the_runner_runs_the_moves_that_are_due().await;
    the_runner_honours_the_limit_and_the_clock().await;
    a_run_without_the_timer_principal_does_not_start().await;
    println!("timed ok");
}

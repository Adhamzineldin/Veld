//! The client of `tests/toolchain/contract/platform_p1/limits.veld` against a transport that
//! answers what the program says: a `get` and an `@idempotent` `post` that are answered `429` with
//! `retryAfter` wait and send the same request once more, a `post` that is not idempotent raises the
//! typed error at once, `retry_on_limit(false)` turns the wait off, a second `429`, one with no
//! `retryAfter` and a wait that does not fit in the call's timeout raise the typed error. The wait
//! goes through an injected sleep, so nothing waits. The program prints `limits client ok`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bookings_client::bookings::bookings::Room;
use bookings_client::bookings::rooms_client::{ArchiveRequest, ByUserRequest, CreateRequest, RoomsClient};
use bookings_client::prelude::TooManyRequests;
use bookings_client::veld::client::{ClientOptions, HttpRequest, HttpResponse, Transport};
use bookings_client::veld::Error;
use serde_json::json;

const ID: &str = "00000000-0000-4000-8000-000000000001";

/// A transport that answers from a script and keeps what it was sent.
#[derive(Clone, Default)]
struct Scripted {
    replies: Arc<Mutex<VecDeque<(u16, String)>>>,
    seen: Arc<Mutex<Vec<HttpRequest>>>,
}

impl Transport for Scripted {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, Error> {
        self.seen.lock().unwrap().push(req);
        let (status, body) = self.replies.lock().unwrap().pop_front().expect("a scripted reply");
        Ok(HttpResponse { status, headers: Vec::new(), body: body.into_bytes() })
    }

    async fn sleep(&self, _d: Duration) {
        panic!("the injected sleep is used, not the transport's");
    }
}

fn limited(retry_after: Option<u32>) -> (u16, String) {
    let data = retry_after.map_or_else(|| json!({}), |s| json!({ "retryAfter": s }));
    (429, json!({ "type": TooManyRequests::VELD_SIG, "title": "TooManyRequests", "status": 429, "operation": null, "data": data }).to_string())
}

fn ok_room() -> (u16, String) {
    (200, json!({ "id": ID }).to_string())
}

struct Run {
    transport: Scripted,
    client: RoomsClient<Scripted>,
    slept: Arc<Mutex<Vec<Duration>>>,
}

fn client(replies: Vec<(u16, String)>, options: impl FnOnce(ClientOptions) -> ClientOptions) -> Run {
    let transport = Scripted::default();
    transport.replies.lock().unwrap().extend(replies);
    let slept = Arc::new(Mutex::new(Vec::new()));
    let log = slept.clone();
    let options = options(ClientOptions::new().base_url("http://rooms.test").sleeper(move |d| {
        log.lock().unwrap().push(d);
        std::future::ready(())
    }));
    let client = RoomsClient::new(transport.clone(), options).unwrap();
    Run { transport, client, slept }
}

fn is_limited(err: &Error) -> bool {
    matches!(err, Error::Problem(p) if p.status == 429)
}

fn by_user() -> ByUserRequest {
    ByUserRequest { id: ID.into() }
}

fn main() {
    use bookings_client::veld::block_on;
    // A `get`: it waits `retryAfter` and sends the same request again, once.
    let run = client(vec![limited(Some(3)), ok_room()], |o| o);
    let room: Room = block_on(run.client.by_user(by_user())).unwrap();
    assert_eq!(room.id, ID);
    assert_eq!(run.slept.lock().unwrap().clone(), vec![Duration::from_secs(3)]);
    let seen = run.transport.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert_eq!((&seen[0].url, &seen[0].headers), (&seen[1].url, &seen[1].headers), "the same request");
    // A second `429` is raised: the wait happened once.
    let run = client(vec![limited(Some(1)), limited(Some(1))], |o| o);
    assert!(is_limited(&block_on(run.client.by_user(by_user())).unwrap_err()));
    assert_eq!((run.transport.seen.lock().unwrap().len(), run.slept.lock().unwrap().len()), (2, 1));
    // No `retryAfter`: raised at once. `retry_on_limit(false)`: raised at once.
    let run = client(vec![limited(None)], |o| o);
    assert!(is_limited(&block_on(run.client.by_user(by_user())).unwrap_err()));
    assert_eq!(run.transport.seen.lock().unwrap().len(), 1);
    let run = client(vec![limited(Some(3))], |o| o.retry_on_limit(false));
    assert!(is_limited(&block_on(run.client.by_user(by_user())).unwrap_err()));
    assert!(run.slept.lock().unwrap().is_empty() && run.transport.seen.lock().unwrap().len() == 1);
    // A wait that does not fit in the timeout of the call is not waited for.
    let run = client(vec![limited(Some(5))], |o| o.timeout(Duration::from_secs(2)));
    assert!(is_limited(&block_on(run.client.by_user(by_user())).unwrap_err()));
    assert!(run.slept.lock().unwrap().is_empty());
    // An `@idempotent` `post`: it waits and sends again with the same key.
    let create = CreateRequest { body: Room { id: ID.into() }, idempotency_key: Some("k-1".into()) };
    let run = client(vec![limited(Some(2)), (201, json!({ "id": ID }).to_string())], |o| o);
    block_on(run.client.create(create)).unwrap();
    let seen = run.transport.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    let key = |r: &HttpRequest| r.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("idempotency-key")).map(|(_, v)| v.clone());
    assert_eq!((key(&seen[0]), key(&seen[1])), (Some("k-1".into()), Some("k-1".into())));
    // A `post` that is not idempotent: the typed error at once.
    let run = client(vec![limited(Some(2))], |o| o);
    assert!(is_limited(&block_on(run.client.archive(ArchiveRequest { id: ID.into() })).unwrap_err()));
    assert!(run.slept.lock().unwrap().is_empty() && run.transport.seen.lock().unwrap().len() == 1);
    println!("limits client ok");
}

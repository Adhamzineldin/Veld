//! Deadlines over the production handler (`spec/platform.md` section 12), generated from
//! `tests/toolchain/contract/platform_p2/service_levels.veld`: an answer in time, the `504` and its
//! body, the cancellation of the handler's future, the idempotency key given back, and the client's
//! default timeout.

use std::time::{Duration, Instant};

use levels_harness::{ID, send, serve};
use rooms_client::rooms::rooms_client::{GetRoomRequest, RoomsClient};
use rooms_client::veld::Error;
use rooms_client::veld::client::ClientOptions;
use serde_json::json;

/// A handler that answers in time: the context has the deadline, an endpoint without one has none.
async fn an_answer_in_time() {
    let s = serve().await;
    assert_eq!(send(&s, "GET", &format!("/rooms/{ID}"), None, None).await.status, 200);
    assert_eq!(send(&s, "GET", "/rooms", None, None).await.status, 200);
    let saw = s.world.saw.lock().unwrap().clone();
    assert_eq!(saw, vec!["getRoom:true:false", "listRooms:false:false"]);
    assert_eq!(*s.world.errors.lock().unwrap(), 0);
}

/// A handler that does not: `504 DeadlineExceeded` with `timeoutMs`, as problem+json, answered at the
/// deadline and not when the handler would have finished; the handler's future is dropped (it never
/// reaches its end, even after the time it needed has passed); an endpoint without a deadline is
/// not cut; nothing goes to `on_error`.
async fn a_handler_that_is_too_slow_is_cancelled() {
    let s = serve().await;
    *s.world.delay_ms.lock().unwrap() = 600;
    let started = Instant::now();
    let a = send(&s, "GET", &format!("/rooms/{ID}"), None, None).await;
    let took = started.elapsed();
    assert_eq!(a.status, 504, "{}", a.doc);
    assert!(a.content_type.starts_with("application/problem+json"), "{}", a.content_type);
    assert_eq!(a.doc["status"], json!(504));
    assert_eq!(a.doc["title"], json!("DeadlineExceeded"));
    assert_eq!(a.doc["data"]["timeoutMs"], json!("200"));
    assert!(took >= Duration::from_millis(190) && took < Duration::from_millis(550), "answered at the deadline: {took:?}");
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert!(s.world.finished.lock().unwrap().is_empty(), "the future was dropped: {:?}", s.world.finished.lock().unwrap());
    assert_eq!(*s.world.errors.lock().unwrap(), 0, "a deadline is a declared error, not a failure");
    // No deadline, no cut: the same delay on `listRooms` is answered late but whole.
    assert_eq!(send(&s, "GET", "/rooms", None, None).await.status, 200);
    assert_eq!(s.world.finished.lock().unwrap().clone(), vec!["listRooms"]);
}

/// A request that hit its deadline gives its idempotency key back (a `5xx` is not stored), once: the
/// same key is run again and answered, and what it answers is then replayed with the handler run
/// twice in all, not three times.
async fn the_key_of_a_request_that_hit_its_deadline_is_released() {
    let s = serve().await;
    let body = json!({ "name": "A" });
    *s.world.delay_ms.lock().unwrap() = 500;
    let slow = send(&s, "POST", "/rooms", Some("k1"), Some(body.clone())).await;
    assert_eq!(slow.status, 504);
    *s.world.delay_ms.lock().unwrap() = 0;
    let again = send(&s, "POST", "/rooms", Some("k1"), Some(body.clone())).await;
    assert_eq!(again.status, 201, "the key was released, not left running or stored: {}", again.doc);
    assert!(!again.replayed);
    let replay = send(&s, "POST", "/rooms", Some("k1"), Some(body)).await;
    assert_eq!((replay.status, replay.replayed), (201, true));
    assert_eq!(*s.world.builds.lock().unwrap(), 2, "the slow run and the run that was answered");
    // The cancelled handler never finished; the late settle of nothing changed the key.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(s.world.finished.lock().unwrap().iter().filter(|n| *n == "build").count(), 1);
}

/// A server that never answers: the method of an endpoint with a deadline fails with the client's
/// timeout error at the deadline when the caller set no timeout, and at the caller's when it did.
async fn the_client_uses_the_deadline_as_its_default_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });
    let request = || GetRoomRequest { id: ID.into() };
    let client = RoomsClient::reqwest(ClientOptions::new().base_url(&base)).unwrap();
    let started = Instant::now();
    let err = client.get_room(request()).await.unwrap_err();
    let took = started.elapsed();
    assert!(matches!(err, Error::Transport(_)), "{err:?}");
    assert!(took >= Duration::from_millis(180) && took < Duration::from_millis(1500), "{took:?}");
    let client = RoomsClient::reqwest(ClientOptions::new().base_url(&base).timeout(Duration::from_millis(700))).unwrap();
    let started = Instant::now();
    assert!(client.get_room(request()).await.is_err());
    assert!(started.elapsed() >= Duration::from_millis(650), "the caller's timeout wins: {:?}", started.elapsed());
}

#[tokio::main]
async fn main() {
    an_answer_in_time().await;
    a_handler_that_is_too_slow_is_cancelled().await;
    the_key_of_a_request_that_hit_its_deadline_is_released().await;
    the_client_uses_the_deadline_as_its_default_timeout().await;
    println!("deadline ok");
}

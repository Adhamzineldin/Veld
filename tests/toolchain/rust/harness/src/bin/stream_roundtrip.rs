//! The Rust streaming client against the Rust server, in one process, on a real socket.

use std::sync::Arc;

use harness::stream_impl::{Probe, app};
use harness::stream_scenario;

#[tokio::main]
async fn main() {
    let probe = Arc::new(Probe::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap_or_else(|e| panic!("bind: {e}"));
    let base = format!("http://127.0.0.1:{}", listener.local_addr().map(|a| a.port()).unwrap_or(0));
    let router = app(&probe);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    stream_scenario::sse(&base, Some(&probe)).await;
    stream_scenario::auth(&base).await;
    stream_scenario::ws(&base, Some(&probe), false).await;
    stream_scenario::ws_auth(&base).await;
    println!("stream roundtrip ok");
}

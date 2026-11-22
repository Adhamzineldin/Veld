//! The Rust client against the Rust server, in one process, on a real socket.

use std::sync::Arc;

use harness::scenario;
use harness::server_impl::{State, app};

#[tokio::main]
async fn main() {
    let state = Arc::new(State::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap_or_else(|e| panic!("bind: {e}"));
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let router = app(&state);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    scenario::run("rust-to-rust", &format!("http://127.0.0.1:{port}"), Some(&state)).await;
    println!("roundtrip ok");
}

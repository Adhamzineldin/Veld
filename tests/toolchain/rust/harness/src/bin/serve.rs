//! Starts the generated Rust server, prints `PORT=<n>`, and runs until stdin closes.

use std::sync::Arc;

use harness::server_impl::{State, app};
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() {
    let state = Arc::new(State::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap_or_else(|e| panic!("bind: {e}"));
    println!("PORT={}", listener.local_addr().map(|a| a.port()).unwrap_or(0));
    let router = app(&state);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let mut stdin = tokio::io::stdin();
    let mut buf = [0u8; 64];
    while matches!(stdin.read(&mut buf).await, Ok(n) if n > 0) {}
}

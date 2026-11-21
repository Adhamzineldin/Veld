//! Reconnect against the flaky Node server: `BASE=http://127.0.0.1:3000 stream_reconnect`.

use harness::stream_scenario;

#[tokio::main]
async fn main() {
    let base = std::env::var("BASE").unwrap_or_else(|_| panic!("set BASE to the flaky server's base URL"));
    stream_scenario::reconnect(&base).await;
    println!("stream reconnect ok");
}

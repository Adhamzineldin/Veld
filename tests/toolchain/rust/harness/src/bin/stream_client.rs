//! The Rust streaming client against any server: `BASE=http://127.0.0.1:3000 stream_client`.

use harness::stream_scenario;

#[tokio::main]
async fn main() {
    let base = std::env::var("BASE").unwrap_or_else(|_| panic!("set BASE to the server's base URL"));
    stream_scenario::sse(&base, None).await;
    stream_scenario::auth(&base).await;
    stream_scenario::ws(&base, None, true).await;
    stream_scenario::ws_auth(&base).await;
    println!("stream client ok");
}

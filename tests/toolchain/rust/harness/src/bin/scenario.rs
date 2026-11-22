//! The Rust client against any server: `BASE=http://127.0.0.1:3000 scenario`.

use harness::scenario;

#[tokio::main]
async fn main() {
    let base = std::env::var("BASE").unwrap_or_else(|_| panic!("set BASE to the server's base URL"));
    scenario::run("rust-client", &base, None).await;
    println!("scenario ok");
}

//! A request body whose model has a required `@readonly` field (`spec/language.md` section 7:
//! "Ignored if present in a request body"): the generated axum-free handler decodes it with and
//! without the field and answers alike. Without `cargo` the test is skipped, unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;

const SENT: &str = "11111111-1111-1111-1111-111111111111";

const DRIVER: &str = r#"use readonly_server::drafts::drafts_server::*;
use readonly_server::veld::server::{Body, Request, RequestContext};
use readonly_server::veld::Error;

struct Impl;

impl DraftsService for Impl {
    async fn create(&self, r: CreateRequest, _c: &RequestContext) -> Result<readonly_server::drafts::drafts::Draft, Error> {
        Ok(r.body)
    }
}

struct Hooks;

impl DraftsHooks for Hooks {}

fn main() {
    for body in std::env::args().skip(1) {
        let handler = DraftsHandler::create(Impl, Hooks);
        let req = Request {
            method: "POST".into(),
            path: "/drafts".into(),
            headers: vec![("content-type".into(), "application/json".into())],
            body: body.into_bytes(),
            ..Request::default()
        };
        let Some(response) = readonly_server::veld::block_on(handler.handle(req)) else {
            println!("no route");
            continue;
        };
        let text = match &response.body {
            Body::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
            _ => String::new(),
        };
        println!("{} {}", response.status, text);
    }
}
"#;

#[test]
fn a_request_without_its_required_readonly_field_is_decoded_and_one_with_it_ignores_it() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path("tests/toolchain/contract/readonly/drafts.veld", "drafts");
    let mut server = gen_server_services(&ir, &["Drafts"], "readonly-server");
    server.add("src/bin/ro.rs", DRIVER).unwrap();
    let ws = work_dir().join("readonly-request");
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    write_files(&ws.join("server"), &server);
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"server\"]\n",
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    let without = json!({ "title": "x" }).to_string();
    let with = json!({ "id": SENT, "title": "x" }).to_string();
    let out = run_ok(
        &mut cargo(
            &ws,
            &[
                "run",
                "-q",
                "-p",
                "readonly-server",
                "--bin",
                "ro",
                "--",
                &without,
                &with,
            ],
        ),
        "the driver",
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    for line in &lines {
        assert!(line.starts_with("201 "), "{out}");
        assert!(line.contains(r#""title":"x""#), "{out}");
    }
    assert_eq!(
        lines[0], lines[1],
        "the read-only field of the request is ignored"
    );
    assert!(!out.contains(SENT), "{out}");
    let _ = std::fs::remove_dir_all(&ws);
}

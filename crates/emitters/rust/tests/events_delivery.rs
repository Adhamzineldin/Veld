//! Events phase E2 in the generated Rust server (`spec/events.md` section 10,
//! `spec/targets/rust.md` section 13): the delivery goldens (`dead-letter.json`, `retry.json`)
//! through the generated consumer, two services in one process over the memory transport, every
//! event example of `tests/events` through the generated test handlers and `veld_testing::Live`,
//! the `messaging` option, the gates and the snapshots of the adapters and of the event files of
//! `tests/events/delivery.veld`.
//!
//! The toolchain tests build a real crate in a scratch workspace (`cargo` and the pinned
//! `Cargo.lock` of `tests/toolchain/rust`) and run drivers from `tests/toolchain/rust/events`.
//! Without `cargo` they are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::*;
use serde_json::json;
use veld_codegen::naming::snake;
use veld_codegen::{DynGenerator, FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_rust::Rust;
use veld_ir::{Ir, PackageKind};

const DELIVERY: &str = "tests/events/delivery.veld";
const BOOKINGS: &str = "tests/events/bookings.veld";

pub fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap()
        .replace("\r\n", "\n")
}

/// Compiles `text` as the package `bookings` from the file `t.veld`: the goldens were made so.
pub fn compile_text(text: String) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "the contract failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap()
}

pub fn server_with(ir: &Ir, options: serde_json::Value) -> FileSet {
    let mut all = json!({ "crate-name": "events-server" });
    all.as_object_mut()
        .unwrap()
        .extend(options.as_object().cloned().unwrap_or_default());
    run(
        ir,
        Role::Server,
        ir.services.iter().map(|s| s.id).collect(),
        vec![],
        None,
        all,
    )
}

pub fn delivery_ir() -> Ir {
    compile_text(read(DELIVERY))
}

// ---- the toolchain tests ---------------------------------------------------------------------

/// The services that subscribe, with the module of their test handler.
fn subscribers(ir: &Ir) -> Vec<String> {
    ir.services
        .iter()
        .filter(|s| !s.subscriptions.is_empty())
        .map(|s| snake(&s.name))
        .collect()
}

/// The driver that serves the test handlers of every subscribing service as one server
/// (`GET /__veld/test`, `POST /__veld/test/event`) and prints `PORT=<n>`.
pub fn serve_driver(ir: &Ir) -> String {
    let mods = subscribers(ir);
    let uses: Vec<String> = mods
        .iter()
        .map(|m| format!("use events_server::bookings::{m}_test_server as {m};"))
        .collect();
    let probes: Vec<String> = mods.iter().map(|m| format!("{m}::probe()")).collect();
    let delivers: Vec<String> = mods
        .iter()
        .map(|m| format!("        answers.push({m}::deliver_event(&topic, key.as_deref(), deliveries, &body).await.unwrap().to_json());"))
        .collect();
    format!(
        r#"//! The test handlers of every subscribing service of the contract behind one address.
use axum::body::Bytes;
use axum::http::{{HeaderMap, StatusCode, header}};
use axum::response::IntoResponse;
use axum::routing::{{get, post}};
use serde_json::{{Value, json}};
{uses}

fn merged_probe() -> String {{
    let mut digests = serde_json::Map::new();
    let mut subscribes = serde_json::Map::new();
    for text in [{probes}] {{
        let one: Value = serde_json::from_str(&text).unwrap();
        digests.extend(one["digests"].as_object().unwrap().clone());
        subscribes.extend(one["subscribes"].as_object().unwrap().clone());
    }}
    json!({{ "veldTestHandler": 1, "digests": digests, "subscribes": subscribes }}).to_string()
}}

async fn event(headers: HeaderMap, body: Bytes) -> axum::response::Response {{
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
    let Some(topic) = text("veld-test-topic") else {{ return StatusCode::BAD_REQUEST.into_response() }};
    let key = text("veld-test-key");
    let deliveries = text("veld-test-deliveries").and_then(|n| n.parse::<u8>().ok()).unwrap_or(1);
    let mut answers = Vec::new();
{delivers}
    let rejected = answers.iter().find(|a| a["outcome"] == "rejected");
    let handled = answers.iter().any(|a| a["outcome"] == "handled");
    let mut called = serde_json::Map::new();
    for a in &answers {{
        called.extend(a["called"].as_object().unwrap().clone());
    }}
    let body = json!({{
        "outcome": if rejected.is_some() {{ "rejected" }} else if handled {{ "handled" }} else {{ "ignored" }},
        "reason": rejected.map_or(Value::Null, |a| a["reason"].clone()),
        "called": called,
        "violations": rejected.map_or(json!([]), |a| a["violations"].clone()),
        "deadLetters": answers.iter().map(|a| a["deadLetters"].as_u64().unwrap()).sum::<u64>(),
    }});
    ([(header::CONTENT_TYPE, "application/json")], body.to_string()).into_response()
}}

#[tokio::main]
async fn main() {{
    let app = axum::Router::new()
        .route("/__veld/test", get(|| async {{ ([(header::CONTENT_TYPE, "application/json")], merged_probe()) }}))
        .route("/__veld/test/event", post(event));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={{}}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}}
"#,
        uses = uses.join("\n"),
        probes = probes.join(", "),
        delivers = delivers.join("\n"),
    )
}

/// A workspace of the generated crate `events-server` and the harness crate `events-harness`
/// whose binaries are `bins`; `files` are written next to the workspace file.
pub fn workspace(
    name: &str,
    server: &FileSet,
    bins: &[(&str, String)],
    files: &[(&str, String)],
) -> PathBuf {
    let ws = work_dir().join(name);
    let _ = std::fs::remove_dir_all(&ws);
    write_files(&ws.join("server"), server);
    let harness = ws.join("harness");
    std::fs::create_dir_all(harness.join("src/bin")).unwrap();
    std::fs::write(
        harness.join("Cargo.toml"),
        "[package]\nname = \"events-harness\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\nevents-server = { path = \"../server\" }\nserde_json = \"1\"\naxum = \"0.8\"\ntokio = { version = \"1\", features = [\"rt-multi-thread\", \"macros\", \"net\"] }\n",
    )
    .unwrap();
    let common = read("tests/toolchain/rust/events/common.rs");
    std::fs::write(harness.join("src/common.rs"), common).unwrap();
    for (bin, source) in bins {
        std::fs::write(harness.join(format!("src/bin/{bin}.rs")), source).unwrap();
    }
    for (file, text) in files {
        std::fs::write(ws.join(file), text).unwrap();
    }
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"server\", \"harness\"]\n",
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    ws
}

fn build_and_lint(ws: &std::path::Path) {
    run_ok(
        &mut cargo(ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    run_ok(
        &mut cargo(
            ws,
            &[
                "clippy",
                "--all-targets",
                "-p",
                "events-server",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
}

fn run_driver(ws: &std::path::Path, bin: &str) -> String {
    run_ok(
        &mut cargo(ws, &["run", "-q", "-p", "events-harness", "--bin", bin]),
        &format!("the driver {bin}"),
    )
}

/// The goldens of the dead-letter copy and of the retry, two services in one process with retries,
/// copies, the inbox and the options, and every event example of `delivery.veld` through the
/// generated test handlers of `Fulfilment` and `Billing`. The crate passes `clippy -D warnings`.
#[test]
fn the_delivery_crate_reproduces_the_goldens_runs_two_services_and_passes_the_examples() {
    if !cargo_or_skip() {
        return;
    }
    let ir = delivery_ir();
    let order_placed = ir.decls.iter().find(|d| d.name == "OrderPlaced").unwrap();
    let digest = match &order_placed.kind {
        veld_ir::DeclKind::Event(e) => e.rules_digest.clone(),
        veld_ir::DeclKind::Model(_)
        | veld_ir::DeclKind::Error(_)
        | veld_ir::DeclKind::Enum(_)
        | veld_ir::DeclKind::Union(_)
        | veld_ir::DeclKind::Const(_) => None,
    };
    let def = json!({ "sig": order_placed.sig, "digest": digest });
    let ws = workspace(
        "events-delivery",
        &server_with(&ir, json!({ "testing": true })),
        &[
            (
                "delivery_wire",
                read("tests/toolchain/rust/events/delivery_wire.rs"),
            ),
            (
                "delivery_process",
                read("tests/toolchain/rust/events/delivery_process.rs"),
            ),
            ("delivery_serve", serve_driver(&ir)),
        ],
        &[
            (
                "dead-letter.json",
                read("tests/wire/events/dead-letter.json"),
            ),
            ("retry.json", read("tests/wire/events/retry.json")),
            ("delivery.json", def.to_string()),
        ],
    );
    build_and_lint(&ws);
    assert!(run_driver(&ws, "delivery_wire").contains("delivery wire ok"));
    assert!(run_driver(&ws, "delivery_process").contains("delivery process ok"));
    run_every_example(&ir, "delivery_serve", ir.examples.len());
}

/// Every event example of `tests/events/bookings.veld` through the generated test handlers of
/// `Notifications` and `Ledger`: a contract without the three annotations answers as before.
#[test]
fn every_event_example_of_the_bookings_contract_passes_through_the_test_handlers() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_text(read(BOOKINGS));
    let ws = workspace(
        "events-bookings-live",
        &server_with(&ir, json!({ "testing": true })),
        &[("bookings_serve", serve_driver(&ir))],
        &[],
    );
    build_and_lint(&ws);
    run_every_example(&ir, "bookings_serve", ir.examples.len());
}

// ---- the generator ---------------------------------------------------------------------------

/// A contract with events and none of the three annotations generates the events files it
/// generated before: `subscribe_events`, `publish_event`, no `Delivery`, no options.
#[test]
fn a_channel_without_annotations_generates_the_events_files_of_phase_e1() {
    let ir = compile_text(read(BOOKINGS));
    let files = server_with(&ir, json!({}));
    for (path, bytes) in files.iter() {
        if path.ends_with("_events.rs") {
            let text = String::from_utf8_lossy(bytes);
            assert!(
                !text.contains("Delivery") && !text.contains("ChannelOptions"),
                "{path}"
            );
            assert!(!text.contains("_with("), "{path}");
        }
    }
    assert!(
        text(&files, "src/bookings/ledger_events.rs").contains("veld::events::subscribe_events(")
    );
    assert!(
        text(&files, "src/bookings/bookings_events.rs").contains("veld::events::publish_event(")
    );
    assert!(
        files.get("src/adapters/kafka.rs").is_none()
            && files.get("src/adapters/rabbitmq.rs").is_none()
    );
    assert!(
        !text(&files, "Cargo.toml").contains("rdkafka")
            && !text(&files, "Cargo.toml").contains("lapin")
    );
}

/// The three annotations reach the generated code: the options of the channel for the publisher
/// and for the transport, the retry policy and the dead letter in the subscription.
#[test]
fn the_annotations_reach_the_generated_publisher_and_subscriber() {
    let files = server_with(&delivery_ir(), json!({}));
    let orders = text(&files, "src/bookings/orders_events.rs");
    assert!(orders.contains("static ORDER_EVENTS_OPTIONS: veld::events::ChannelOptions = veld::events::ChannelOptions { ordered: true, dead_letter: true };"), "{orders}");
    assert!(orders.contains("veld::events::publish_event_with(self.ctx, &ORDER_EVENTS_CHANNEL, ORDER_EVENTS_OPTIONS,"), "{orders}");
    let fulfilment = text(&files, "src/bookings/fulfilment_events.rs");
    assert!(
        fulfilment.contains("veld::events::subscribe_events_with("),
        "{fulfilment}"
    );
    assert!(fulfilment.contains("retry: Some(veld::events::RetryPolicy { max: 3, backoff: veld::events::Backoff::Exponential }), dead_letter: true, ordered: true"), "{fulfilment}");
}

fn adapters() -> FileSet {
    server_with(
        &delivery_ir(),
        json!({ "messaging": ["rabbitmq", "kafka", "kafka"] }),
    )
}

/// `messaging` names the adapters (deduplicated); each is one module, the only one that uses its
/// library, and the manifest names the library only for the chosen adapter.
#[test]
fn messaging_generates_one_module_per_broker_and_names_the_library_only_then() {
    let both = adapters();
    assert!(text(&both, "src/adapters/mod.rs").contains("pub mod kafka;\npub mod rabbitmq;"));
    assert!(text(&both, "src/lib.rs").contains("pub mod adapters;"));
    let manifest = text(&both, "Cargo.toml");
    assert!(
        manifest.contains("rdkafka") && manifest.contains("lapin"),
        "{manifest}"
    );
    for (path, bytes) in both.iter() {
        let body = String::from_utf8_lossy(bytes);
        let uses_library = body.contains("rdkafka::") || body.contains("lapin::");
        assert_eq!(
            uses_library,
            path == "src/adapters/kafka.rs" || path == "src/adapters/rabbitmq.rs",
            "{path}"
        );
    }
    let kafka = server_with(&delivery_ir(), json!({ "messaging": "kafka" }));
    assert!(kafka.get("src/adapters/rabbitmq.rs").is_none());
    assert!(
        text(&kafka, "Cargo.toml").contains("rdkafka")
            && !text(&kafka, "Cargo.toml").contains("lapin")
    );
    let rabbit = server_with(&delivery_ir(), json!({ "messaging": ["rabbitmq"] }));
    assert!(rabbit.get("src/adapters/kafka.rs").is_none());
    assert!(!text(&rabbit, "Cargo.toml").contains("rdkafka"));
    let none = server_with(&delivery_ir(), json!({ "messaging": [] }));
    assert_eq!(
        none.paths().collect::<Vec<_>>(),
        server_with(&delivery_ir(), json!({}))
            .paths()
            .collect::<Vec<_>>()
    );
}

fn refused(ir: &Ir, role: Role, options: serde_json::Value) -> String {
    let mut all = json!({ "crate-name": "events-server" });
    all.as_object_mut()
        .unwrap()
        .extend(options.as_object().cloned().unwrap_or_default());
    let all = veld_codegen::with_target_name(&all, "t");
    let req = veld_codegen::GenRequest {
        ir,
        role,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: None,
    };
    let diags = Rust.generate_dyn(&req, &all).unwrap_err();
    assert!(diags.iter().all(|d| d.code == "E0802"), "{diags:?}");
    diags[0].message.clone()
}

#[test]
fn an_unknown_broker_and_the_option_on_a_target_without_events_are_e0802() {
    let ir = delivery_ir();
    assert!(refused(&ir, Role::Server, json!({ "messaging": "nats" })).contains("`nats`"));
    assert!(refused(&ir, Role::Server, json!({ "messaging": ["kafka", "sqs"] })).contains("`sqs`"));
    assert!(refused(&ir, Role::Client, json!({ "messaging": "kafka" })).contains("messaging"));
    let plain = compile_text("model Item { name: string }\nservice Plain at \"/p\" {\n post \"/\" create(body: Item) -> 201 Item\n}\n".to_string());
    assert!(
        refused(&plain, Role::Server, json!({ "messaging": "kafka" }))
            .contains("publishes or subscribes")
    );
}

/// Without `testing` a server has no event endpoint; with it, a service that only subscribes has a
/// test handler that takes no application and answers `subscribes` in its probe.
#[test]
fn the_test_handler_of_a_subscriber_serves_the_event_endpoint_and_lists_its_topics() {
    let ir = delivery_ir();
    let off = server_with(&ir, json!({}));
    assert!(off.get("src/bookings/fulfilment_test_server.rs").is_none());
    let on = server_with(&ir, json!({ "testing": true }));
    let file = text(&on, "src/bookings/fulfilment_test_server.rs");
    assert!(
        file.contains("\"subscribes\":{\"Fulfilment\":[\"shop.orders.v1\"]}"),
        "{file}"
    );
    assert!(
        file.contains("/__veld/test/event") && file.contains("veld-test-deliveries"),
        "{file}"
    );
    assert!(
        file.contains("pub fn router() -> ::axum::Router"),
        "no application: {file}"
    );
    assert!(
        file.contains("MemoryInbox") && file.contains("Box::pin(std::future::ready(()))"),
        "{file}"
    );
    assert!(
        on.get("src/bookings/orders_test_server.rs").is_none(),
        "a publisher has none"
    );
    assert!(text(&on, "Cargo.toml").contains("axum"));
    // The production modules do not change with `testing`.
    for (path, bytes) in off.iter() {
        if path != "src/bookings/mod.rs" && path != "Cargo.toml" {
            assert_eq!(Some(bytes), on.get(path), "{path}");
        }
    }
}

// ---- snapshots (the main session stores them) --------------------------------------------------

#[test]
fn snapshots_of_the_event_files_of_the_delivery_contract() {
    let files = server_with(
        &delivery_ir(),
        json!({ "testing": true, "messaging": ["kafka", "rabbitmq"] }),
    );
    insta::assert_snapshot!(
        "delivery_file_list",
        files.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "delivery_publisher",
        text(&files, "src/bookings/orders_events.rs")
    );
    insta::assert_snapshot!(
        "delivery_subscriber",
        text(&files, "src/bookings/fulfilment_events.rs")
    );
    insta::assert_snapshot!(
        "delivery_events_runtime",
        text(&files, "src/veld/events.rs")
    );
    insta::assert_snapshot!(
        "delivery_test_handler",
        text(&files, "src/bookings/fulfilment_test_server.rs")
    );
    insta::assert_snapshot!("delivery_manifest", text(&files, "Cargo.toml"));
}

#[test]
fn snapshots_of_the_adapters() {
    let files = adapters();
    insta::assert_snapshot!("adapter_kafka", text(&files, "src/adapters/kafka.rs"));
    insta::assert_snapshot!("adapter_rabbitmq", text(&files, "src/adapters/rabbitmq.rs"));
    insta::assert_snapshot!("adapter_mod", text(&files, "src/adapters/mod.rs"));
}

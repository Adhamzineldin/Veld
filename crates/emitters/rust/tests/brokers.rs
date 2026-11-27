//! The broker adapters against real brokers (`spec/events.md` section 10.7 and 10.9,
//! `spec/targets/rust.md` section 13, `tests/toolchain/brokers/README.md`): the generated crate of
//! `tests/events/delivery.veld` with `messaging = ["kafka", "rabbitmq"]` and the programs of
//! `tests/toolchain/rust/brokers`. The programs are compiled always (`cargo build`, `clippy`) and
//! run when `VELD_KAFKA` (bootstrap servers) and `VELD_RABBITMQ` (an AMQP URL) are set;
//! `VELD_REQUIRE_BROKERS=1` turns a skipped program into a failure. The topic of the contract is
//! renamed for every run so that a program starts with offsets and queues of its own.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap()
        .replace("\r\n", "\n")
}

/// `delivery.veld` with a topic that nobody used: the process and the clock.
pub fn contract(topic: &str) -> Ir {
    let text = read("tests/events/delivery.veld").replace("shop.orders.v1", topic);
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

fn server(ir: &Ir) -> FileSet {
    run(
        ir,
        Role::Server,
        ir.services.iter().map(|s| s.id).collect(),
        vec![],
        None,
        json!({ "crate-name": "events-server", "messaging": ["kafka", "rabbitmq"] }),
    )
}

pub fn fresh_topic() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("shop.orders.{}.{nanos}", std::process::id())
}

/// The generated crate and the harness crate, whose binaries are the two broker programs.
pub fn workspace(ir: &Ir) -> PathBuf {
    let ws = work_dir().join("events-brokers");
    let _ = std::fs::remove_dir_all(&ws);
    write_files(&ws.join("server"), &server(ir));
    let harness = ws.join("harness");
    std::fs::create_dir_all(harness.join("src/bin")).unwrap();
    // The same library features as the generated crate, so that the C library is built once.
    std::fs::write(
        harness.join("Cargo.toml"),
        "[package]\nname = \"events-harness\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\nevents-server = { path = \"../server\" }\nserde_json = \"1\"\nrdkafka = { version = \"0.38\", default-features = false, features = [\"cmake-build\", \"tokio\"] }\nlapin = { version = \"4\", default-features = false, features = [\"tokio\"] }\ntokio = { version = \"1\", features = [\"rt-multi-thread\", \"macros\", \"time\", \"sync\"] }\n",
    )
    .unwrap();
    std::fs::write(
        harness.join("src/common.rs"),
        read("tests/toolchain/rust/brokers/common.rs"),
    )
    .unwrap();
    for bin in ["kafka", "rabbitmq"] {
        std::fs::write(
            harness.join(format!("src/bin/{bin}.rs")),
            read(&format!("tests/toolchain/rust/brokers/{bin}.rs")),
        )
        .unwrap();
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

/// Compiles the programs always; runs each one when its variable is set. The programs themselves
/// decide to skip (and fail under `VELD_REQUIRE_BROKERS=1`), so a missing variable is visible there.
#[test]
fn the_broker_programs_compile_and_run_against_real_brokers_when_they_are_configured() {
    if !cargo_or_skip() {
        return;
    }
    let topic = fresh_topic();
    let ws = workspace(&contract(&topic));
    run_ok(
        &mut cargo(&ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    run_ok(
        &mut cargo(
            &ws,
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
        "cargo clippy of the generated crate",
    );
    for (bin, variable, done) in [
        ("kafka", "VELD_KAFKA", "kafka ok"),
        ("rabbitmq", "VELD_RABBITMQ", "rabbitmq ok"),
    ] {
        let configured = std::env::var(variable).is_ok_and(|v| !v.is_empty());
        let require = std::env::var("VELD_REQUIRE_BROKERS").as_deref() == Ok("1");
        if !configured && !require {
            eprintln!("SKIPPED: {variable} is not set");
            continue;
        }
        let mut cmd = cargo(&ws, &["run", "-q", "-p", "events-harness", "--bin", bin]);
        cmd.env("VELD_TOPIC", &topic);
        let out = run_ok(&mut cmd, &format!("the broker program {bin}"));
        assert!(out.contains(done), "{bin}: {out}");
    }
}

/// The adapters are the only generated files that mention their library, and neither is generated
/// without the option (`tests/events_delivery.rs` has the other gates).
#[test]
fn only_the_adapter_modules_use_a_broker_library() {
    let files = server(&contract(&fresh_topic()));
    for (path, bytes) in files.iter() {
        let body = String::from_utf8_lossy(bytes);
        let mentions = body.contains("rdkafka") || body.contains("lapin");
        let allowed = matches!(
            path,
            "src/adapters/kafka.rs" | "src/adapters/rabbitmq.rs" | "Cargo.toml" | "README.md"
        );
        assert!(!mentions || allowed, "{path} mentions a broker library");
    }
}

//! The broker adapters of events phase E2 (`spec/events.md` sections 10.1 and 10.7,
//! `spec/targets/java.md` section 12) against real brokers.
//!
//! Two programs, `tests/toolchain/java/brokers/EventsKafka.java` and `EventsRabbitmq.java`, drive the
//! generated `KafkaTransport` and `RabbitMqTransport` through every row of the matrix. They run only
//! when `VELD_KAFKA` (bootstrap servers) and `VELD_RABBITMQ` (an AMQP URL) name a broker;
//! `VELD_REQUIRE_BROKERS=1` turns a missing variable into a failure. Both programs are always
//! compiled with `javac -Xlint:all -Werror` against the real client jars, so the adapters are
//! type-checked even where no broker runs. `tests/toolchain/brokers/README.md` says how to start the
//! two containers.
//!
//! The jars are found in `~/.m2/repository` (`kafka-clients` 4.1.2, `amqp-client` 5.27.1, `slf4j-api`
//! 2.0.17) or through `VELD_KAFKA_CLIENTS_JAR`, `VELD_AMQP_CLIENT_JAR` and `VELD_SLF4J_JAR`. Without
//! them the tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::live::{require_javac, write_files};
use common::*;
use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;

const DELIVERY: &str = "tests/events/delivery.veld";

/// The contract as the package `bookings` from the file `t.veld`, with its three services.
fn contract() -> veld_ir::Ir {
    let text = std::fs::read_to_string(repo_root().join(DELIVERY))
        .unwrap()
        .replace("\r\n", "\n");
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
        consumes: Default::default(),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    out.ir.unwrap_or_else(|| unreachable!())
}

fn jar(variable: &str, relative: &str) -> Option<PathBuf> {
    let path = std::env::var(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".m2/repository")
                .join(relative)
        });
    path.exists().then_some(path)
}

/// The class path of the two clients, or `None` (and a skip) when a jar is missing.
fn class_path(test: &str) -> Option<String> {
    let jars = [
        jar(
            "VELD_KAFKA_CLIENTS_JAR",
            "org/apache/kafka/kafka-clients/4.1.2/kafka-clients-4.1.2.jar",
        ),
        jar(
            "VELD_AMQP_CLIENT_JAR",
            "com/rabbitmq/amqp-client/5.27.1/amqp-client-5.27.1.jar",
        ),
        jar(
            "VELD_SLF4J_JAR",
            "org/slf4j/slf4j-api/2.0.17/slf4j-api-2.0.17.jar",
        ),
    ];
    if jars.iter().any(Option::is_none) {
        assert!(
            std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
            "toolchain required but a client jar is missing"
        );
        eprintln!("SKIPPED {test}: the Kafka and RabbitMQ client jars are not in ~/.m2");
        return None;
    }
    let sep = if cfg!(windows) { ";" } else { ":" };
    Some(
        jars.into_iter()
            .flatten()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(sep),
    )
}

/// Writes the server with both adapters into `work/src` and compiles it with the programs.
fn build(work: &Path, class_path: &str) {
    let ir = contract();
    let mut opts = options("gen.e", Some("17"));
    opts["messaging"] = json!(["kafka", "rabbitmq"]);
    let files = run(
        &ir,
        veld_codegen::Role::Server,
        ir.services.iter().map(|s| s.id).collect(),
        vec![],
        None,
        opts,
    );
    let mut sources = write_files(work, &files);
    for program in ["EventsKafka.java", "EventsRabbitmq.java"] {
        sources.push(
            repo_root()
                .join("tests/toolchain/java/brokers")
                .join(program),
        );
    }
    let list: Vec<String> = sources
        .iter()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .collect();
    std::fs::write(work.join("sources.args"), list.join("\n")).unwrap();
    let out = Command::new("javac")
        .args([
            "--release",
            "17",
            "-Xlint:all",
            "-Werror",
            "-encoding",
            "UTF-8",
        ])
        .args(["-cp", class_path, "-d", "classes", "@sources.args"])
        .current_dir(work)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "javac failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn java(work: &Path, class_path: &str, class: &str) -> Output {
    let sep = if cfg!(windows) { ";" } else { ":" };
    Command::new("java")
        .args(["-cp", &format!("classes{sep}{class_path}"), class])
        .current_dir(work)
        .output()
        .unwrap()
}

/// The broker named by `variable`, or `None` when it is not set (a failure with
/// `VELD_REQUIRE_BROKERS=1`).
fn broker(test: &str, variable: &str) -> Option<()> {
    if std::env::var(variable).is_ok_and(|v| !v.is_empty()) {
        return Some(());
    }
    assert!(
        std::env::var("VELD_REQUIRE_BROKERS").as_deref() != Ok("1"),
        "{test}: brokers are required but {variable} is not set"
    );
    eprintln!("SKIPPED {test}: {variable} is not set");
    None
}

fn run_program(test: &str, variable: &str, class: &str, ok: &str) {
    if !require_javac(test) {
        return;
    }
    let Some(class_path) = class_path(test) else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    build(dir.path(), &class_path);
    if broker(test, variable).is_none() {
        return;
    }
    let out = java(dir.path(), &class_path, class);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains(ok),
        "{class} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_adapters_compile_against_the_real_clients_and_the_kafka_rows_hold() {
    run_program(
        "the_kafka_rows",
        "VELD_KAFKA",
        "veldtest.EventsKafka",
        "events kafka ok",
    );
}

#[test]
fn the_rabbitmq_rows_hold() {
    run_program(
        "the_rabbitmq_rows",
        "VELD_RABBITMQ",
        "veldtest.EventsRabbitmq",
        "events rabbitmq ok",
    );
}

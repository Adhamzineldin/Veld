//! The broker adapters of events phase E2 (`spec/events.md` sections 10.1 and 10.7,
//! `spec/targets/typescript.md` section 5f) against real brokers.
//!
//! Two programs, `tests/toolchain/ts/events_kafka.ts` and `events_rabbitmq.ts`, drive the
//! generated `KafkaTransport` and `RabbitMqTransport` through every row of the matrix. They run
//! only when `VELD_KAFKA` (bootstrap servers) and `VELD_RABBITMQ` (an AMQP URL) name a broker;
//! `VELD_REQUIRE_BROKERS=1` turns a missing variable into a failure. Both programs are always
//! compiled (with `tsc`, against the real `kafkajs` and `amqplib` types), so the adapters are
//! type-checked even where no broker runs. `tests/toolchain/brokers/README.md` says how to start
//! the two containers.
//!
//! The toolchain tests need `node` and `tests/toolchain/ts/node_modules`; without them they are
//! skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::Path;
use std::process::Command;

use std::collections::BTreeMap;

use common::toolchain::{run, scratch, toolchain_dir, tsc_path};
use common::*;
use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;
use veld_project::output::{WriteOptions, write};

const DELIVERY: &str = "tests/events/delivery.veld";

/// The contract as the package `bookings` from the file `t.veld`, as the drivers expect: the
/// events are in `events/t` and the `source` of a message is `veld:bookings/<Service>`.
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
        consumes: BTreeMap::new(),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    let ir = out.ir.unwrap_or_else(|| unreachable!());
    assert_eq!(ir.services.len(), 3);
    ir
}

/// Writes the server with both adapters into `work/server` and compiles it with the drivers
/// under the Node types (the adapters use `Buffer` and timers, which the plain server's strict
/// pass, with no types packages, cannot see).
fn build(work: &Path, tsc: &Path, drivers: &[&str], edition: &str) {
    let target = TARGETS
        .iter()
        .find(|(e, _)| *e == edition)
        .map_or("ES2020", |(_, t)| *t);
    let ir = contract();
    let options = json!({ "typescript": edition, "messaging": ["kafka", "rabbitmq"] });
    let files = gen_servers_with(&ir, &["Orders", "Fulfilment", "Billing"], &[], &options);
    write(&work.join("server"), &files, &WriteOptions::default()).unwrap();
    for driver in drivers {
        std::fs::copy(toolchain_dir().join(driver), work.join(driver)).unwrap();
    }
    std::fs::write(
        work.join("tsconfig.json"),
        serde_json::to_string_pretty(&json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target, "dom"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "server/**/*.ts"]
        }))
        .unwrap(),
    )
    .unwrap();
    run(
        Command::new("node")
            .arg(tsc)
            .args(["-p", "tsconfig.json"])
            .current_dir(work),
        "tsc the adapters and the broker programs",
    );
}

/// The value of `name`, or `None` when the test is skipped; a failure with `VELD_REQUIRE_BROKERS=1`.
fn broker(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => {
            assert!(
                std::env::var("VELD_REQUIRE_BROKERS").as_deref() != Ok("1"),
                "{name} is not set and VELD_REQUIRE_BROKERS=1"
            );
            eprintln!("SKIPPED broker test: {name} is not set");
            None
        }
    }
}

/// Compiles `driver` for both editions and, when `variable` names a broker, runs it there.
fn drive(driver: &str, variable: &str, ok: &str) {
    let Some(tsc) = tsc_path() else {
        return;
    };
    let url = broker(variable);
    for (edition, _) in TARGETS {
        let guard = scratch("events-broker-");
        let work = guard.path();
        build(work, &tsc, &[driver], edition);
        let Some(url) = &url else {
            continue;
        };
        let out = Command::new("node")
            .arg(format!("out/{}.js", driver.trim_end_matches(".ts")))
            .env(variable, url)
            .current_dir(work)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && stdout.contains(ok),
            "{driver} ({edition}) failed\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn the_kafka_transport_keeps_every_row_of_the_matrix_against_a_real_broker() {
    drive("events_kafka.ts", "VELD_KAFKA", "events kafka ok");
}

#[test]
fn the_rabbitmq_transport_keeps_every_row_of_the_matrix_against_a_real_broker() {
    drive("events_rabbitmq.ts", "VELD_RABBITMQ", "events rabbitmq ok");
}

#[test]
fn the_adapters_compile_against_the_real_libraries_and_take_the_library_objects_as_they_are() {
    // The broker programs pass `await amqp.connect(url)` as the adapter's connection and a
    // `new Kafka(..)` as the `Kafka`: a type error there is a change of the libraries' API.
    let Some(tsc) = tsc_path() else {
        return;
    };
    for (edition, _) in TARGETS {
        let guard = scratch("events-adapters-");
        build(
            guard.path(),
            &tsc,
            &["events_kafka.ts", "events_rabbitmq.ts"],
            edition,
        );
    }
}

#[test]
fn the_broker_programs_exist_and_name_the_variables_the_readme_documents() {
    let kafka = std::fs::read_to_string(toolchain_dir().join("events_kafka.ts")).unwrap();
    let rabbit = std::fs::read_to_string(toolchain_dir().join("events_rabbitmq.ts")).unwrap();
    assert!(kafka.contains("VELD_KAFKA") && rabbit.contains("VELD_RABBITMQ"));
    let readme =
        std::fs::read_to_string(repo_root().join("tests/toolchain/brokers/README.md")).unwrap();
    for word in [
        "VELD_KAFKA",
        "VELD_RABBITMQ",
        "VELD_REQUIRE_BROKERS",
        "apache/kafka-native:3.9.0",
        "rabbitmq:4-management-alpine",
    ] {
        assert!(readme.contains(word), "the README does not mention {word}");
    }
}

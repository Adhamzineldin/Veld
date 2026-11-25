//! Rules in the generated TypeScript: snapshots, `tsc --strict`, and a round trip over HTTP
//! (`spec/targets/typescript.md` section 5b).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use serde_json::json;
use veld_project::output::{WriteOptions, write};

#[test]
fn rules_files_exist_only_for_a_contract_with_rules() {
    let with = gen_client(&rules_ir());
    assert!(with.get("runtime/rules.ts").is_some());
    assert!(text(&with, "runtime/veld.ts").contains("checkValue"));
    let without = gen_client(&standard_ir());
    assert!(without.get("runtime/rules.ts").is_none());
    assert!(!text(&without, "runtime/veld.ts").contains("checkValue"));
    let server = gen_server(&rules_ir(), "Bookings", &[]);
    assert_eq!(
        text(&server, "runtime/veld.ts"),
        text(&with, "runtime/veld.ts")
    );
}

#[test]
fn snapshots_of_a_contract_with_rules() {
    let ir = rules_ir();
    let client = gen_client(&ir);
    insta::assert_snapshot!(
        "rules_client_file_list",
        client.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "rules_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
    let server = gen_server(&ir, "Bookings", &[]);
    insta::assert_snapshot!("rules_server_bookings", text(&server, "server/bookings.ts"));
    insta::assert_snapshot!("rules_runtime_rules.ts", text(&server, "runtime/rules.ts"));
}

fn toolchain_dir() -> PathBuf {
    repo_root().join("tests/toolchain/ts")
}

fn tsc_path() -> Option<PathBuf> {
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    let tsc = toolchain_dir().join("node_modules/typescript/bin/tsc");
    if node_ok && tsc.exists() {
        return Some(tsc);
    }
    assert!(
        std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
        "toolchain required but unavailable"
    );
    eprintln!("SKIPPED rules toolchain test");
    None
}

fn run(cmd: &mut Command, what: &str) {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{what} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn strict_config(dirs: &[&str], target: &str) -> serde_json::Value {
    json!({
        "compilerOptions": {
            "strict": true, "target": target, "lib": [target], "types": [],
            "module": "commonjs", "moduleResolution": "node10", "noEmit": true,
            "skipLibCheck": false, "forceConsistentCasingInFileNames": true
        },
        "include": dirs.iter().map(|d| format!("{d}/**/*.ts")).collect::<Vec<_>>()
    })
}

fn write_config(work: &Path, name: &str, value: &serde_json::Value) {
    std::fs::write(
        work.join(name),
        serde_json::to_string_pretty(value).unwrap(),
    )
    .unwrap();
}

#[test]
fn generated_rules_compile_strict_and_round_trip_over_http() {
    let Some(tsc) = tsc_path() else { return };
    let ir = rules_ir();
    for (edition, target) in TARGETS {
        std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
        let guard = tempfile::Builder::new()
            .prefix("rules-http-")
            .tempdir_in(toolchain_dir().join(".work"))
            .unwrap();
        let work = guard.path();
        let opts = WriteOptions::default();
        let option = typescript_option(edition);
        let server = gen_server_with(&ir, "Bookings", &[], &option);
        write(&work.join("server"), &server, &opts).unwrap();
        write(&work.join("client"), &gen_client_with(&ir, &option), &opts).unwrap();
        std::fs::copy(
            toolchain_dir().join("rules_http.ts"),
            work.join("rules_http.ts"),
        )
        .unwrap();
        // 1. Each generated package compiles on its own, with no types packages.
        write_config(
            work,
            "tsconfig.generated.json",
            &strict_config(&["server", "client"], target),
        );
        run(
            Command::new("node")
                .arg(&tsc)
                .args(["-p", "tsconfig.generated.json"])
                .current_dir(work),
            "tsc generated",
        );
        // 2. The round trip compiles with the Node types and runs.
        write_config(
            work,
            "tsconfig.json",
            &json!({
                "compilerOptions": {
                    "strict": true, "target": target, "lib": [target, "dom"], "types": ["node"],
                    "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                    "skipLibCheck": true, "outDir": "out", "rootDir": "."
                },
                "include": ["*.ts", "server/**/*.ts", "client/**/*.ts"]
            }),
        );
        run(
            Command::new("node")
                .arg(&tsc)
                .args(["-p", "tsconfig.json"])
                .current_dir(work),
            "tsc round trip",
        );
        run(
            Command::new("node")
                .arg("out/rules_http.js")
                .current_dir(work),
            "node rules_http.js",
        );
    }
}

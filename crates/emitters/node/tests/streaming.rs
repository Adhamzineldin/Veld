//! Streaming (Server-Sent Events and WebSocket) against a real TypeScript toolchain.
//!
//! The streaming contract is generated for the `node` server and the `typescript` client (plain
//! and with React), compiled with `tsc --strict`, and run: the generated client against the
//! generated server over real sockets, and the React hooks against a live SSE endpoint.
//!
//! Without `node` or `node_modules` the tests are skipped, unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use serde_json::{Value, json};
use veld_project::output::{WriteOptions, write};

fn toolchain_dir() -> PathBuf {
    repo_root().join("tests/toolchain/ts")
}

fn locate() -> Result<PathBuf, String> {
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !node_ok {
        return Err("`node` is not on PATH".into());
    }
    let tsc = toolchain_dir().join("node_modules/typescript/bin/tsc");
    if !tsc.exists()
        || !toolchain_dir()
            .join("node_modules/ws/package.json")
            .exists()
    {
        return Err("run `npm ci` in tests/toolchain/ts (needs typescript and ws)".into());
    }
    Ok(tsc)
}

fn run(cmd: &mut Command, what: &str) {
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{what}: cannot start: {e}"));
    print!("{}", String::from_utf8_lossy(&out.stdout));
    assert!(
        out.status.success(),
        "{what} failed ({})\nstdout:\n{}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

fn tsc(tsc_path: &Path, work: &Path, project: &str) {
    run(
        Command::new("node")
            .arg(tsc_path)
            .args(["-p", project])
            .current_dir(work),
        &format!("tsc -p {project}"),
    );
}

fn node_script(work: &Path, script: &str) {
    let mut cmd = Command::new("node");
    cmd.arg(format!("out/{script}.js")).current_dir(work);
    run(&mut cmd, &format!("node {script}.js"));
}

fn matrix(body: fn(&Path, &str, &str)) {
    let tsc_path = match locate() {
        Ok(p) => p,
        Err(why) => {
            assert!(
                std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
                "toolchain required but unavailable: {why}"
            );
            eprintln!("SKIPPED toolchain test: {why}");
            return;
        }
    };
    for (edition, target) in TARGETS {
        body(&tsc_path, edition, target);
    }
}

fn work_dir(prefix: &str) -> tempfile::TempDir {
    std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(toolchain_dir().join(".work"))
        .unwrap()
}

fn only_generated(dirs: &[&str], module: &str, target: &str, exclude: &[&str]) -> Value {
    let (resolution, module) = if module == "nodenext" {
        ("nodenext", "nodenext")
    } else {
        ("node10", "commonjs")
    };
    json!({
        "compilerOptions": {
            "strict": true, "target": target, "lib": [target], "types": [],
            "module": module, "moduleResolution": resolution,
            "noEmit": true, "skipLibCheck": false, "forceConsistentCasingInFileNames": true
        },
        "include": dirs.iter().map(|d| format!("{d}/**/*.ts")).collect::<Vec<_>>(),
        "exclude": exclude
    })
}

/// Generates the streaming contract into `server`, `client` and `client_react`, plus the scripts.
fn generate(work: &Path, edition: &str, scripts: &[&str]) {
    let ir = streaming_ir();
    let opts = WriteOptions::default();
    let ts = typescript_option(edition);
    let react = json!({ "typescript": edition, "framework-version": "18" });
    write(
        &work.join("server"),
        &gen_server_services(&ir, &["Live", "Vault"], &ts),
        &opts,
    )
    .unwrap();
    write(&work.join("client"), &gen_client_with(&ir, &ts), &opts).unwrap();
    write(
        &work.join("client_react"),
        &gen_client_fw(&ir, &react, Some("react")),
        &opts,
    )
    .unwrap();
    let vue = json!({ "typescript": edition, "framework-version": "3" });
    write(
        &work.join("client_vue"),
        &gen_client_fw(&ir, &vue, Some("vue")),
        &opts,
    )
    .unwrap();
    for script in scripts {
        std::fs::copy(toolchain_dir().join(script), work.join(script)).unwrap();
    }
}

fn behaviour_config(target: &str, extra: &Value) -> Value {
    let mut compiler = json!({
        "strict": true, "target": target, "lib": [target], "types": ["node"],
        "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
        "skipLibCheck": true, "outDir": "out", "rootDir": "."
    });
    for (k, v) in extra.as_object().unwrap() {
        compiler[k] = v.clone();
    }
    json!({
        "compilerOptions": compiler,
        "include": ["*.ts", "*.tsx", "server/**/*.ts", "client/**/*.ts", "client_react/**/*.ts", "client_vue/**/*.ts"]
    })
}

fn streams_round_trip(tsc_path: &Path, edition: &str, target: &str) {
    let guard = work_dir("stream-");
    let work = guard.path();
    generate(
        work,
        edition,
        &[
            "stream_server.ts",
            "stream_roundtrip.ts",
            "stream_wire.ts",
            "react_stream.tsx",
            "vue_stream.ts",
        ],
    );
    // 1. Generated code alone: strict, no types packages (the `ws` adapter is structural).
    for module in ["commonjs", "nodenext"] {
        let cfg = only_generated(&["server", "client"], module, target, &[]);
        write_json(&work.join(format!("tsconfig.plain-{module}.json")), &cfg);
        tsc(tsc_path, work, &format!("tsconfig.plain-{module}.json"));
    }
    // 2. The React hooks and the scripts, then the round trip.
    write_json(
        &work.join("tsconfig.json"),
        &behaviour_config(target, &json!({ "jsx": "react-jsx" })),
    );
    tsc(tsc_path, work, "tsconfig.json");
    let mut wire = Command::new("node");
    wire.arg("out/stream_wire.js")
        .current_dir(work)
        .env("WIRE_DIR", repo_root().join("tests/wire"));
    run(&mut wire, "node stream_wire.js");
    node_script(work, "stream_roundtrip");
    node_script(work, "react_stream");
    node_script(work, "vue_stream");
}

/// SSE and WebSocket against real Fastify and Hono servers (the other tests use node:http).
fn frameworks_round_trip(tsc_path: &Path, edition: &str, target: &str) {
    let guard = work_dir("stream-fw-");
    let work = guard.path();
    generate(
        work,
        edition,
        &[
            "stream_server.ts",
            "stream_roundtrip.ts",
            "stream_frameworks.ts",
        ],
    );
    write_json(
        &work.join("tsconfig.json"),
        &behaviour_config(target, &json!({})),
    );
    tsc(tsc_path, work, "tsconfig.json");
    node_script(work, "stream_frameworks");
}

#[test]
fn sse_and_websocket_round_trip_on_fastify_and_hono() {
    matrix(frameworks_round_trip);
}

#[test]
fn sse_and_websocket_round_trip_and_react_hooks_per_version() {
    matrix(streams_round_trip);
}

#[test]
fn streaming_snapshots_determinism_and_signatures() {
    let ir = streaming_ir();
    let ts = serde_json::Value::Null;
    let server = gen_server_services(&ir, &["Live", "Vault"], &ts);
    let client = gen_client(&ir);
    let react = gen_client_fw(&ir, &json!({ "framework-version": "18" }), Some("react"));
    insta::assert_snapshot!(
        "streaming_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("streaming_server__live.ts", text(&server, "server/live.ts"));
    insta::assert_snapshot!("streaming_server__ws.ts", text(&server, "server/ws.ts"));
    insta::assert_snapshot!(
        "streaming_server_package.json",
        text(&server, "package.json")
    );
    insta::assert_snapshot!(
        "streaming_client__client.ts",
        text(&client, "live/client.ts")
    );
    insta::assert_snapshot!("streaming_react__react.ts", text(&react, "live/react.ts"));
    // Deterministic.
    assert_eq!(server, gen_server_services(&ir, &["Live", "Vault"], &ts));
    assert_eq!(client, gen_client(&ir));
    // Every operation signature is embedded verbatim, streaming endpoints included.
    for service in &ir.services {
        for ep in &service.endpoints {
            assert!(
                text(
                    &server,
                    &format!("server/{}.ts", service.name.to_lowercase())
                )
                .contains(&ep.sig)
            );
            assert!(text(&client, "live/client.ts").contains(&ep.sig));
        }
    }
}

#[test]
fn a_server_without_ws_endpoints_has_no_ws_adapter_and_no_peer_dependency() {
    let ir = common::standard_ir();
    let files = gen_server(&ir, "Orders", &["Iam"]);
    assert!(files.get("server/ws.ts").is_none());
    assert!(!text(&files, "package.json").contains("peerDependencies"));
}

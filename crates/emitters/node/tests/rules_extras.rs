//! The rules that reach past request bodies (`spec/rules.md` sections 5.2 to 5.4), in generated TypeScript:
//! invariants of declared errors, SSE items and WebSocket messages, request validation turned off and the
//! `W0904` warning, the React and Vue form helpers, and the cross-target golden of a `422` problem document
//! (`tests/wire/rules`).
//!
//! The toolchain tests need `node` and `tests/toolchain/ts/node_modules`; without them they are skipped
//! unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use serde_json::{Value, json};
use veld_codegen::canonical::canonical;
use veld_codegen::{DynGenerator, GenRequest, Role};
use veld_emit_node::Node;
use veld_emit_typescript::TypeScript;
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

fn run(cmd: &mut Command, what: &str) -> String {
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{what}: cannot start: {e}"));
    assert!(
        out.status.success(),
        "{what} failed ({})\nstdout:\n{}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
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

/// The generated directories alone, strict, with no types packages.
fn plain_config(dirs: &[&str], target: &str, exclude: &[&str]) -> Value {
    json!({
        "compilerOptions": {
            "strict": true, "target": target, "lib": [target], "types": [],
            "module": "commonjs", "moduleResolution": "node10",
            "noEmit": true, "skipLibCheck": false, "forceConsistentCasingInFileNames": true
        },
        "include": dirs.iter().map(|d| format!("{d}/**/*.ts")).collect::<Vec<_>>(),
        "exclude": exclude
    })
}

/// Everything, with the node types, plus the scripts.
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

fn copy_scripts(work: &Path, scripts: &[&str]) {
    for script in scripts {
        std::fs::copy(toolchain_dir().join(script), work.join(script)).unwrap();
    }
}

fn node_script(work: &Path, script: &str) -> String {
    run(
        Command::new("node")
            .arg(format!("out/{script}.js"))
            .current_dir(work),
        &format!("node {script}.js"),
    )
}

// ---------------------------------------------------------------------------
// Errors, SSE items and WebSocket messages
// ---------------------------------------------------------------------------

fn streams_round_trip(tsc_path: &Path, edition: &str, target: &str) {
    let guard = work_dir("rules-live-");
    let work = guard.path();
    let ir = rules_live_ir();
    let opts = WriteOptions::default();
    let ts = typescript_option(edition);
    write(
        &work.join("server"),
        &gen_server_with(&ir, "Live", &[], &ts),
        &opts,
    )
    .unwrap();
    write(&work.join("client"), &gen_client_with(&ir, &ts), &opts).unwrap();
    copy_scripts(work, &["rules_streams.ts", "rules_live_impl.ts"]);
    write_json(
        &work.join("tsconfig.plain.json"),
        &plain_config(&["server", "client"], target, &[]),
    );
    tsc(tsc_path, work, "tsconfig.plain.json");
    write_json(
        &work.join("tsconfig.json"),
        &behaviour_config(target, &json!({})),
    );
    tsc(tsc_path, work, "tsconfig.json");
    assert!(node_script(work, "rules_streams").contains("rules streams ok"));
}

#[test]
fn errors_sse_items_and_websocket_messages_are_checked() {
    matrix(streams_round_trip);
}

fn nest_round_trip(tsc_path: &Path, edition: &str, target: &str) {
    let guard = work_dir("rules-nest-");
    let work = guard.path();
    let ir = rules_live_ir();
    let nest = json!({ "typescript": edition, "framework-version": "10" });
    let server = gen_server_fw(&ir, "Live", &[], &nest, Some("nest"));
    write(&work.join("server"), &server, &WriteOptions::default()).unwrap();
    copy_scripts(work, &["rules_nest.ts", "rules_live_impl.ts"]);
    write_json(
        &work.join("tsconfig.json"),
        &behaviour_config(
            target,
            &json!({ "experimentalDecorators": true, "emitDecoratorMetadata": true }),
        ),
    );
    tsc(tsc_path, work, "tsconfig.json");
    assert!(node_script(work, "rules_nest").contains("rules nest ok"));
}

#[test]
fn the_nest_adapter_checks_errors_and_stream_items_too() {
    matrix(nest_round_trip);
}

// ---------------------------------------------------------------------------
// Request validation turned off
// ---------------------------------------------------------------------------

fn no_validate_round_trip(tsc_path: &Path, edition: &str, target: &str) {
    let guard = work_dir("rules-novalidate-");
    let work = guard.path();
    let ir = rules_ir();
    let opts = WriteOptions::default();
    let off = json!({ "typescript": edition, "validate": false });
    write(
        &work.join("server"),
        &gen_server_with(&ir, "Bookings", &[], &off),
        &opts,
    )
    .unwrap();
    write(&work.join("client"), &gen_client_with(&ir, &off), &opts).unwrap();
    copy_scripts(work, &["rules_no_validate.ts"]);
    write_json(
        &work.join("tsconfig.json"),
        &behaviour_config(target, &json!({})),
    );
    tsc(tsc_path, work, "tsconfig.json");
    assert!(node_script(work, "rules_no_validate").contains("rules no validate ok"));
}

#[test]
fn a_server_built_with_validation_off_accepts_a_rule_violating_body_and_says_so_once() {
    matrix(no_validate_round_trip);
}

fn request(role: Role, ir: &veld_ir::Ir) -> GenRequest<'_> {
    let services = ir.services.iter().map(|s| s.id).collect();
    GenRequest {
        ir,
        role,
        services,
        consumes: vec![],
        framework: None,
    }
}

#[test]
fn w0904_is_reported_for_a_target_with_validation_off_that_reaches_rules() {
    let ir = rules_ir();
    let off = json!({ "validate": false });
    for (generator, role) in [
        (&Node as &dyn DynGenerator, Role::Server),
        (&TypeScript as &dyn DynGenerator, Role::Client),
    ] {
        let req = request(role, &ir);
        let warnings = generator.warnings_dyn(&req, &off);
        assert_eq!(warnings.len(), 1, "{}: {warnings:?}", generator.id());
        assert_eq!(warnings[0].code, "W0904");
        assert!(warnings[0].message.contains("bookings:bookings.Booking"));
        assert!(
            warnings[0].message.contains("and 2 more"),
            "{:?}",
            warnings[0]
        );
        assert!(warnings[0].origin.is_some(), "points at the first rule");
        // Validation on (the default, or asked for): nothing to say.
        assert!(generator.warnings_dyn(&req, &json!({})).is_empty());
        assert!(
            generator
                .warnings_dyn(&req, &json!({ "validate": true }))
                .is_empty()
        );
    }
}

#[test]
fn w0904_is_not_reported_when_the_contract_has_no_rules() {
    let ir = standard_ir();
    let off = json!({ "validate": false });
    let server = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![service_id(&ir, "Orders")],
        consumes: vec![service_id(&ir, "Iam")],
        framework: None,
    };
    assert!(Node.warnings_dyn(&server, &off).is_empty());
    assert!(
        TypeScript
            .warnings_dyn(&request(Role::Client, &ir), &off)
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// React and Vue form helpers
// ---------------------------------------------------------------------------

fn forms_round_trip(tsc_path: &Path, edition: &str, target: &str) {
    let guard = work_dir("rules-forms-");
    let work = guard.path();
    let ir = rules_ir();
    let opts = WriteOptions::default();
    let react = json!({ "typescript": edition, "framework-version": "18" });
    let vue = json!({ "typescript": edition, "framework-version": "3" });
    write(
        &work.join("client_react"),
        &gen_client_fw(&ir, &react, Some("react")),
        &opts,
    )
    .unwrap();
    write(
        &work.join("client_vue"),
        &gen_client_fw(&ir, &vue, Some("vue")),
        &opts,
    )
    .unwrap();
    copy_scripts(work, &["rules_form_react.tsx", "rules_form_vue.ts"]);
    write_json(
        &work.join("tsconfig.json"),
        &behaviour_config(target, &json!({ "jsx": "react-jsx" })),
    );
    tsc(tsc_path, work, "tsconfig.json");
    assert!(node_script(work, "rules_form_react").contains("rules form react ok"));
    assert!(node_script(work, "rules_form_vue").contains("rules form vue ok"));
}

#[test]
fn the_react_and_vue_form_helpers_compile_strict_and_work() {
    matrix(forms_round_trip);
}

#[test]
fn form_helpers_exist_only_for_a_contract_with_rules() {
    let with = rules_ir();
    let without = standard_ir();
    for (framework, version, helper) in [
        ("react", "18", "useFormRules"),
        ("vue", "3", "useFormRules"),
    ] {
        let options = json!({ "framework-version": version });
        let ruled = gen_client_fw(&with, &options, Some(framework));
        let plain = gen_client_fw(&without, &options, Some(framework));
        assert!(text(&ruled, &format!("runtime/{framework}.ts")).contains(helper));
        assert!(text(&ruled, &format!("bookings/{framework}.ts")).contains("useFormRules"));
        for (path, bytes) in plain.iter() {
            let body = std::str::from_utf8(bytes).unwrap();
            assert!(
                !body.contains("useFormRules") && !body.contains("formRules"),
                "{path}"
            );
        }
        insta::assert_snapshot!(
            format!("rules_{framework}_runtime"),
            text(&ruled, &format!("runtime/{framework}.ts"))
        );
        insta::assert_snapshot!(
            format!("rules_{framework}_package"),
            text(&ruled, &format!("bookings/{framework}.ts"))
        );
    }
}

// ---------------------------------------------------------------------------
// The golden
// ---------------------------------------------------------------------------

fn golden_round_trip(tsc_path: &Path, edition: &str, target: &str) {
    let guard = work_dir("rules-golden-");
    let work = guard.path();
    let ir = rules_ir();
    let ts = typescript_option(edition);
    write(
        &work.join("server"),
        &gen_server_with(&ir, "Bookings", &[], &ts),
        &WriteOptions::default(),
    )
    .unwrap();
    copy_scripts(work, &["rules_golden.ts"]);
    write_json(
        &work.join("tsconfig.json"),
        &behaviour_config(target, &json!({})),
    );
    tsc(tsc_path, work, "tsconfig.json");
    let out = run(
        Command::new("node")
            .arg("out/rules_golden.js")
            .env("WIRE_DIR", repo_root().join("tests/wire"))
            .current_dir(work),
        "node rules_golden.js",
    );
    let answer: Value = serde_json::from_str(out.trim()).unwrap();
    let fixed: Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("tests/wire/rules/request.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(answer["status"], fixed["status"]);
    assert_eq!(answer["contentType"], fixed["contentType"]);
    let body: Value = serde_json::from_str(answer["body"].as_str().unwrap()).unwrap();
    assert!(
        body["violations"].as_array().unwrap().len() >= 2,
        "the fixed request has at least two violations"
    );
    // The golden is the canonical JSON (RFC 8785) of the whole problem document, as one line without a
    // final newline; line-ending conversion of the checkout must not matter.
    let golden =
        std::fs::read_to_string(repo_root().join("tests/wire/rules/problem.json")).unwrap();
    assert_eq!(canonical(&body), golden.trim_end_matches(['\r', '\n']));
}

#[test]
fn the_node_server_answers_the_golden_422_problem_document_byte_for_byte() {
    matrix(golden_round_trip);
}

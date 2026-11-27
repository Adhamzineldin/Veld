//! Rules beyond bodies (`spec/targets/java.md` section 8): the 422 golden on both adapters,
//! declared streams, a server built with validation off, and `W0904`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;

use common::live::*;
use common::*;
use veld_codegen::Role;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

fn compile_contract_file(package: &str, path: &str) -> Ir {
    compile_contract_with(package, path, &|s| s.to_string())
}

fn compile_contract_with(package: &str, path: &str, patch: &dyn Fn(&str) -> String) -> Ir {
    let text = patch(&std::fs::read_to_string(repo_root().join(path)).unwrap());
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: package.into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: std::path::Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let rendered = out.render();
    out.ir
        .unwrap_or_else(|| panic!("{path} does not compile:\n{rendered}"))
}

fn all(ir: &Ir) -> Vec<veld_ir::ServiceId> {
    ir.services.iter().map(|s| s.id).collect()
}

// -- the 422 golden ---------------------------------------------------------------------------

fn golden_request() -> serde_json::Value {
    let path = repo_root().join("tests/wire/rules/request.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Compares a response with `tests/wire/rules`: status, content type and the canonical bytes of
/// the body.
fn assert_golden(status: u16, content_type: &str, body: &str) {
    let request = golden_request();
    assert_eq!(
        u64::from(status),
        request["status"].as_u64().unwrap(),
        "{body}"
    );
    assert!(
        content_type.starts_with(request["contentType"].as_str().unwrap()),
        "{content_type}"
    );
    let parsed: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
    let canonical = veld_codegen::canonical::canonical(&parsed);
    let expected =
        std::fs::read_to_string(repo_root().join("tests/wire/rules/problem.json")).unwrap();
    assert_eq!(
        canonical,
        expected.trim_end(),
        "the problem document differs from the golden"
    );
}

fn golden_files(framework: Option<&str>) -> (Ir, veld_codegen::FileSet) {
    let ir = compile_contract_file("bookings", "tests/toolchain/contract/rules/bookings.veld");
    let files = run(
        &ir,
        Role::Server,
        all(&ir),
        vec![],
        framework,
        options("gen.rg", Some("17")),
    );
    (ir, files)
}

#[test]
fn the_jdk_server_answers_the_422_golden() {
    if !require_javac("the_jdk_server_answers_the_422_golden") {
        return;
    }
    let (_, files) = golden_files(None);
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &files);
    sources.push(repo_root().join("tests/toolchain/java/rules/RuleGolden.java"));
    javac_ok(work.path(), &sources, "17");
    let request = golden_request();
    let body = request["body"].to_string();
    let out = java_main(
        work.path(),
        "veldtest.RuleGolden",
        &[
            request["method"].as_str().unwrap(),
            request["path"].as_str().unwrap(),
            &body,
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut lines = text.lines();
    let status: u16 = lines.next().unwrap().parse().unwrap();
    let content_type = lines.next().unwrap();
    assert_golden(status, content_type, lines.next().unwrap());
}

#[test]
fn the_spring_adapter_answers_the_422_golden() {
    let name = "the_spring_adapter_answers_the_422_golden";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    let (_, files) = golden_files(Some("spring"));
    let app = start_spring(
        "spring-golden",
        &files,
        &["rules/spring-golden/App.java"],
        "/bookings/report?broken=false",
        &[],
    );
    let request = golden_request();
    let raw = raw_http(
        app.port,
        request["method"].as_str().unwrap(),
        request["path"].as_str().unwrap(),
        &[],
        &request["body"].to_string(),
    )
    .unwrap();
    let content_type = raw
        .headers
        .lines()
        .find_map(|l| l.strip_prefix("content-type: "))
        .unwrap_or("")
        .to_string();
    assert_golden(raw.status, &content_type, &raw.body);
}

// -- streams ----------------------------------------------------------------------------------

const LIVE: &str = "tests/toolchain/contract/rules_live/live.veld";

#[test]
fn an_sse_item_that_violates_its_rule_is_handled_by_the_output_mode_on_the_jdk_server() {
    if !require_javac("an_sse_item_that_violates") {
        return;
    }
    let ir = compile_contract_with("live", LIVE, &without_ws);
    let work = tempfile::tempdir().unwrap();
    let mut sources = Vec::new();
    for (role, base) in [(Role::Server, "gen.rs"), (Role::Client, "gen.rc")] {
        let files = run(&ir, role, all(&ir), vec![], None, options(base, Some("17")));
        sources.extend(write_files(work.path(), &files));
    }
    sources.push(repo_root().join("tests/toolchain/java/rules/RuleStreamsSse.java"));
    javac_ok(work.path(), &sources, "17");
    let out = java_main(work.path(), "veldtest.RuleStreamsSse", &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn stream_items_and_websocket_messages_are_checked_through_the_spring_adapter() {
    let name = "stream_items_and_websocket_messages";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    let ir = compile_contract_file("live", LIVE);
    let server = run(
        &ir,
        Role::Server,
        all(&ir),
        vec![],
        Some("spring"),
        options("gen.rs", Some("17")),
    );
    let client = run(
        &ir,
        Role::Client,
        all(&ir),
        vec![],
        None,
        options("gen.rc", Some("17")),
    );
    let work = build_spring(
        "spring-streams",
        &server,
        &[
            "rules/spring-streams/App.java",
            "rules/spring-streams/Probe.java",
        ],
    );
    let driver = tempfile::tempdir().unwrap();
    let mut sources = write_files(driver.path(), &client);
    sources.push(repo_root().join("tests/toolchain/java/rules/RuleStreamsWs.java"));
    javac_ok(driver.path(), &sources, "17");
    for (mode, events) in [
        (
            "strict",
            vec!["OUTPUT REJECTED", "OUTPUT REJECTED", "INPUT REJECTED"],
        ),
        (
            "audit",
            vec!["OUTPUT AUDITED", "OUTPUT AUDITED", "INPUT REJECTED"],
        ),
        ("off", vec!["INPUT REJECTED"]),
    ] {
        let app = run_spring(
            &work,
            "/__events",
            &[&format!("--veld.rules.output={mode}")],
        );
        let port = app.port.to_string();
        let out = java_main(driver.path(), "veldtest.RuleStreamsWs", &[&port, mode]);
        assert!(
            out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
            "{mode}:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let seen = raw_http(app.port, "GET", "/__events", &[], "")
            .unwrap()
            .body;
        let seen: Vec<&str> = seen.lines().collect();
        assert_eq!(seen, events, "{mode}: the hook saw these violations");
    }
}

// -- request validation turned off ------------------------------------------------------------

fn no_validate_options(base: &str) -> serde_json::Value {
    let mut o = options(base, Some("17"));
    o["validate"] = serde_json::json!(false);
    o
}

#[test]
fn a_server_built_with_validation_off_accepts_a_violating_body_and_reports_it_once() {
    if !require_javac("a_server_built_with_validation_off") {
        return;
    }
    let ir = compile_contract_file("bookings", "tests/toolchain/contract/rules/bookings.veld");
    let work = tempfile::tempdir().unwrap();
    let mut sources = Vec::new();
    for (role, base) in [(Role::Server, "gen.rs"), (Role::Client, "gen.rc")] {
        let files = run(&ir, role, all(&ir), vec![], None, no_validate_options(base));
        sources.extend(write_files(work.path(), &files));
    }
    sources.push(repo_root().join("tests/toolchain/java/rules/RuleNoValidate.java"));
    javac_ok(work.path(), &sources, "17");
    let out = java_main(work.path(), "veldtest.RuleNoValidate", &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn warnings_of(ir: &Ir, options: &serde_json::Value, role: Role) -> Vec<String> {
    use veld_codegen::{DynGenerator, GenRequest};
    let req = GenRequest {
        ir,
        role,
        services: all(ir),
        consumes: vec![],
        framework: None,
    };
    veld_emit_java::Java
        .warnings_dyn(&req, options)
        .iter()
        .map(|w| format!("{}: {}", w.code, w.message))
        .collect()
}

#[test]
fn w0904_is_reported_when_validation_is_off_and_the_target_reaches_rules() {
    let ir = compile_contract_file("bookings", "tests/toolchain/contract/rules/bookings.veld");
    for role in [Role::Server, Role::Client] {
        let off = warnings_of(&ir, &no_validate_options("gen.w"), role);
        assert_eq!(off.len(), 1, "{off:?}");
        assert!(off[0].starts_with("W0904: "), "{off:?}");
        assert!(off[0].contains("Booking"), "{off:?}");
        assert!(warnings_of(&ir, &options("gen.w", Some("17")), role).is_empty());
    }
    let plain = standard_ir();
    let mut o = options("gen.w", None);
    o["validate"] = serde_json::json!(false);
    assert!(warnings_of(&plain, &o, Role::Server).is_empty());
}

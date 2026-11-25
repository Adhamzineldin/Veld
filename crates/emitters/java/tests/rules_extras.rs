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
    let text = std::fs::read_to_string(repo_root().join(path)).unwrap();
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: package.into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: std::path::Path::new(path).file_name().unwrap().to_string_lossy().into_owned(),
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
    assert_eq!(u64::from(status), request["status"].as_u64().unwrap(), "{body}");
    assert!(
        content_type.starts_with(request["contentType"].as_str().unwrap()),
        "{content_type}"
    );
    let parsed: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
    let canonical = veld_codegen::canonical::canonical(&parsed);
    let expected = std::fs::read_to_string(repo_root().join("tests/wire/rules/problem.json"))
        .unwrap();
    assert_eq!(canonical, expected.trim_end(), "the problem document differs from the golden");
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

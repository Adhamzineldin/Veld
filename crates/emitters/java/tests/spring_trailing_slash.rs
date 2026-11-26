//! A trailing slash on a contract without rules (`spec/targets/java.md` sections 5 and 8): the JDK
//! server and the Spring adapter route `/items` and `/items/` alike and answer the same.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;

use common::live::*;
use common::*;
use veld_codegen::Role;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;

const REQUESTS: [(&str, &str, &str); 4] = [
    ("POST", "/items", r#"{"name":"a"}"#),
    ("POST", "/items/", r#"{"name":"a"}"#),
    ("GET", "/items/count", ""),
    ("GET", "/items/count/", ""),
];

fn files(framework: Option<&str>) -> veld_codegen::FileSet {
    let path = "tests/toolchain/contract/slash/items.veld";
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "shop".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "items.veld".into(),
                text: std::fs::read_to_string(repo_root().join(path)).unwrap(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let rendered = out.render();
    let ir = out.ir.unwrap_or_else(|| panic!("{rendered}"));
    let all = ir.services.iter().map(|s| s.id).collect();
    run(
        &ir,
        Role::Server,
        all,
        vec![],
        framework,
        options("gen.sl", Some("17")),
    )
}

#[test]
fn the_spring_adapter_and_the_jdk_server_treat_a_trailing_slash_alike() {
    let name = "the_spring_adapter_and_the_jdk_server_treat_a_trailing_slash_alike";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    // The JDK server.
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &files(None));
    sources.push(repo_root().join("tests/toolchain/java/slash/SlashJdk.java"));
    javac_ok(work.path(), &sources, "17");
    let args: Vec<&str> = REQUESTS.iter().flat_map(|(m, p, b)| [*m, *p, *b]).collect();
    let out = java_main(work.path(), "veldtest.SlashJdk", &args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let jdk: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(jdk.len(), REQUESTS.len(), "{jdk:?}");
    // The Spring adapter.
    let app = start_spring(
        "spring-slash",
        &files(Some("spring")),
        &["slash/spring/App.java"],
        "/items/count",
        &[],
    );
    let spring: Vec<String> = REQUESTS
        .iter()
        .map(|(m, p, b)| {
            let raw = raw_http(app.port, m, p, &[], b).unwrap();
            format!("{} {}", raw.status, raw.body.trim())
        })
        .collect();
    assert_eq!(jdk, spring, "the two servers disagree");
    // And both routed the slash form: the same success as the exact path.
    assert!(
        jdk[0].starts_with("201 ") && jdk[1].starts_with("201 "),
        "{jdk:?}"
    );
    assert!(
        jdk[2].starts_with("200 ") && jdk[3].starts_with("200 "),
        "{jdk:?}"
    );
}

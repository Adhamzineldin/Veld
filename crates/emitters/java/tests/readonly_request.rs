//! A request body whose model has a required `@readonly` field (`spec/language.md` section 7:
//! "Ignored if present in a request body"): the generated JDK server decodes it with and without
//! the field and answers alike.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;

use common::live::*;
use common::*;
use veld_codegen::Role;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;

const SENT: &str = "11111111-1111-1111-1111-111111111111";

#[test]
fn a_request_without_its_required_readonly_field_is_decoded_and_one_with_it_ignores_it() {
    if !require_javac("a_request_without_its_required_readonly_field") {
        return;
    }
    let path = "tests/toolchain/contract/readonly/drafts.veld";
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "drafts".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "drafts.veld".into(),
                text: std::fs::read_to_string(repo_root().join(path)).unwrap(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let rendered = out.render();
    let ir = out.ir.unwrap_or_else(|| panic!("{rendered}"));
    let all = ir.services.iter().map(|s| s.id).collect();
    let files = run(
        &ir,
        Role::Server,
        all,
        vec![],
        None,
        options("gen.ro", Some("17")),
    );
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &files);
    sources.push(repo_root().join("tests/toolchain/java/rules/ReadonlyJdk.java"));
    javac_ok(work.path(), &sources, "17");
    let without = r#"{"title":"x"}"#;
    let with = format!(r#"{{"id":"{SENT}","title":"x"}}"#);
    let out = java_main(work.path(), "veldtest.ReadonlyJdk", &[without, &with]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    for line in &lines {
        assert!(line.starts_with("201 "), "{text}");
        assert!(line.contains(r#""title":"x""#), "{text}");
    }
    assert_eq!(
        lines[0], lines[1],
        "the read-only field of the request is ignored"
    );
    assert!(!text.contains(SENT), "{text}");
}

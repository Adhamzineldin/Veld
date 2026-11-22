//! `plugin:` targets through the real `veld generate`, with a scripted Node plugin.
//!
//! Without `node` the tests are skipped with a message, unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn node_available() -> bool {
    let ok = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        assert!(
            std::env::var("VELD_REQUIRE_TOOLCHAINS").is_err(),
            "VELD_REQUIRE_TOOLCHAINS is set but node is missing"
        );
    }
    ok
}

fn stub() -> String {
    let p: PathBuf =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stub_plugin.mjs");
    p.canonicalize()
        .unwrap()
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
}

const CONTRACT: &str = r#"/// A greeting.
model Greeting {
  message: string
}

/// Says hello.
service Hello at "/hello" {
  /// Greets.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}
"#;

fn project(dir: &Path, plugin_args: &str, extra_plugin_keys: &str) {
    write(dir, "contracts/hello.veld", CONTRACT);
    write(
        dir,
        "veld.toml",
        &format!(
            "[project]\nname = \"demo\"\n\n[plugins.stub]\ncommand = \"node {} {plugin_args}\"\n{extra_plugin_keys}\n\
             [[targets]]\nname = \"kt\"\nrole = \"client\"\nemitter = \"plugin:stub\"\nout = \"gen/kt\"\n",
            stub()
        ),
    );
}

#[test]
fn plugin_target_generates_through_the_staged_writer() {
    if !node_available() {
        eprintln!("skipped: node is not available");
        return;
    }
    let tmp = tempdir().unwrap();
    project(tmp.path(), "--mode ok", "version = \"1.0.0\"\n");
    veld(tmp.path())
        .arg("generate")
        .assert()
        .success()
        .stdout(predicate::str::contains("kt -> gen/kt:"))
        .stderr(predicate::str::contains(
            "[plugin:stub] stub starting in mode ok",
        ));
    let files = tree(tmp.path());
    assert!(files.contains_key("gen/kt/plugin-output.txt"));
    assert!(
        files.contains_key("gen/kt/.veld-manifest.json"),
        "plugin output is tracked by the manifest like any other"
    );
    let lock = files.get("veld.lock.ir.json").cloned();
    assert!(lock.is_some());
    veld(tmp.path())
        .arg("generate")
        .assert()
        .success()
        .stdout(predicate::str::contains("0 created, 0 updated"));
    let sig_file = files
        .iter()
        .find(|(p, _)| p.starts_with("gen/kt/ops/"))
        .unwrap();
    assert!(
        String::from_utf8_lossy(sig_file.1).starts_with("veld:demo/Hello.greet@"),
        "signature embedded verbatim"
    );
}

#[test]
fn a_crash_leaves_the_previous_output_intact() {
    if !node_available() {
        return;
    }
    let tmp = tempdir().unwrap();
    project(tmp.path(), "--mode ok", "");
    veld(tmp.path()).arg("generate").assert().success();
    let before = tree(&tmp.path().join("gen"));
    project(tmp.path(), "--mode crash", "");
    veld(tmp.path())
        .arg("generate")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0708"))
        .stderr(predicate::str::contains("exit code 3"));
    assert_eq!(before, tree(&tmp.path().join("gen")));
}

#[test]
fn escaping_paths_write_nothing() {
    if !node_available() {
        return;
    }
    let tmp = tempdir().unwrap();
    project(tmp.path(), "--mode escape", "");
    veld(tmp.path())
        .arg("generate")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0707"));
    assert!(!tmp.path().join("gen").exists());
    assert!(
        !tmp.path().join("escaped.txt").exists() && !tmp.path().join("gen/escaped.txt").exists()
    );
}

#[test]
fn a_wrong_sha256_pin_is_refused() {
    if !node_available() {
        return;
    }
    let tmp = tempdir().unwrap();
    project(
        tmp.path(),
        "--mode ok",
        &format!("sha256 = \"{}\"\n", "0".repeat(64)),
    );
    veld(tmp.path())
        .arg("generate")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0705"));
    assert!(!tmp.path().join("gen").exists());
}

#[test]
fn capabilities_are_checked_like_for_built_in_emitters() {
    if !node_available() {
        return;
    }
    let tmp = tempdir().unwrap();
    project(tmp.path(), "--mode ok", "");
    write(
        tmp.path(),
        "contracts/hello.veld",
        &CONTRACT.replace("service Hello", "@auth(bearer)\nservice Hello"),
    );
    veld(tmp.path())
        .arg("generate")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0801"));
    assert!(!tmp.path().join("gen").exists());
    let toml = fs::read_to_string(tmp.path().join("veld.toml"))
        .unwrap()
        .replace("--mode ok", "--mode ok --caps auth,constraints");
    write(tmp.path(), "veld.toml", &toml);
    veld(tmp.path()).arg("generate").assert().success();
}

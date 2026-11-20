//! `veld fmt`, `diff`, `export` and `migrate`, run through the real binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn init(dir: &Path) {
    veld(dir).args(["init", "--name", "demo"]).assert().success();
}

const MESSY: &str = "model   Greeting{\n  message:string\n}\n";

const BREAKING: &str = r#"/// A greeting for one person.
model Greeting {
  message: string
}

/// Says hello.
service Hello at "/hello" {
  /// Says goodbye.
  get "/bye/{name}" farewell(name: string) -> Greeting | NotFound
}
"#;

// --- fmt -------------------------------------------------------------------------------

#[test]
fn fmt_check_lists_files_and_fmt_rewrites_them() {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    write(tmp.path(), "contracts/messy.veld", MESSY);
    veld(tmp.path())
        .args(["fmt", "--check"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("would reformat contracts/messy.veld"));
    assert_eq!(fs::read_to_string(tmp.path().join("contracts/messy.veld")).unwrap(), MESSY, "--check never writes");
    veld(tmp.path()).arg("fmt").assert().success();
    let formatted = fs::read_to_string(tmp.path().join("contracts/messy.veld")).unwrap();
    assert!(formatted.contains("model Greeting {"), "{formatted}");
    veld(tmp.path()).args(["fmt", "--check"]).assert().success();
    let names: Vec<String> = fs::read_dir(tmp.path().join("contracts"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.contains("veld-tmp")), "no temp files left: {names:?}");
}

#[test]
fn fmt_reports_syntax_errors_and_leaves_those_files_alone() {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    let broken = "model Broken {\n  x: \n";
    write(tmp.path(), "contracts/broken.veld", broken);
    write(tmp.path(), "contracts/messy.veld", MESSY);
    veld(tmp.path())
        .arg("fmt")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("broken.veld"))
        .stderr(predicate::str::contains("not formatted because of syntax errors"));
    assert_eq!(fs::read_to_string(tmp.path().join("contracts/broken.veld")).unwrap(), broken);
    assert!(fs::read_to_string(tmp.path().join("contracts/messy.veld")).unwrap().contains("model Greeting {"));
}

#[test]
fn fmt_with_paths_touches_only_those_files_and_needs_no_project() {
    let tmp = tempdir().unwrap();
    write(tmp.path(), "a.veld", MESSY);
    write(tmp.path(), "b.veld", MESSY);
    veld(tmp.path()).args(["fmt", "a.veld"]).assert().success();
    assert!(fs::read_to_string(tmp.path().join("a.veld")).unwrap().contains("model Greeting {"));
    assert_eq!(fs::read_to_string(tmp.path().join("b.veld")).unwrap(), MESSY);
    veld(tmp.path()).args(["fmt", "missing.veld"]).assert().code(1);
}

// --- lock file and diff ------------------------------------------------------------------

#[test]
fn generate_writes_a_deterministic_lock_and_diff_uses_it() {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    veld(tmp.path()).args(["diff"]).assert().code(1).stderr(predicate::str::contains("veld generate"));
    veld(tmp.path()).arg("generate").assert().success();
    let lock = fs::read(tmp.path().join("veld.lock.ir.json")).unwrap();
    veld(tmp.path()).arg("generate").assert().success();
    assert_eq!(lock, fs::read(tmp.path().join("veld.lock.ir.json")).unwrap(), "lock is deterministic");
    veld(tmp.path()).arg("diff").assert().success().stdout(predicate::str::contains("no changes"));

    write(tmp.path(), "contracts/hello.veld", BREAKING);
    veld(tmp.path())
        .arg("diff")
        .assert()
        .success()
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains("Non-breaking changes"));
    veld(tmp.path()).args(["diff", "--lock", "--strict"]).assert().code(1);
    // a dry run and a single-target run do not move the baseline
    veld(tmp.path()).args(["generate", "--dry-run"]).assert().success();
    assert_eq!(lock, fs::read(tmp.path().join("veld.lock.ir.json")).unwrap());
}

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn diff_against_a_git_ref_never_touches_the_working_tree() {
    if !Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success()) {
        eprintln!("skipped: git is not available");
        return;
    }
    let tmp = tempdir().unwrap();
    init(tmp.path());
    assert!(git(tmp.path(), &["init", "-q"]));
    assert!(git(tmp.path(), &["add", "."]));
    assert!(git(tmp.path(), &["commit", "-q", "-m", "base"]));
    write(tmp.path(), "contracts/hello.veld", BREAKING);
    let before = tree(tmp.path());
    veld(tmp.path())
        .args(["diff", "--against", "HEAD"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains("greet"));
    veld(tmp.path()).args(["diff", "--against", "HEAD", "--strict"]).assert().code(1);
    assert_eq!(before, tree(tmp.path()), "the working tree is untouched");
    assert!(git(tmp.path(), &["status", "--porcelain"]));
    veld(tmp.path()).args(["diff", "--against", "no-such-ref"]).assert().code(1).stderr(predicate::str::contains("not a git"));
}

// --- export ------------------------------------------------------------------------------

#[test]
fn export_openapi_and_ir() {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    let out = veld(tmp.path()).args(["export", "openapi"]).assert().success().get_output().stdout.clone();
    let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(doc["openapi"], "3.1.0");
    veld(tmp.path()).args(["export", "openapi", "-o", "api.json", "--service", "Hello"]).assert().success();
    let file: serde_json::Value = serde_json::from_slice(&fs::read(tmp.path().join("api.json")).unwrap()).unwrap();
    assert_eq!(file["openapi"], "3.1.0");
    veld(tmp.path()).args(["export", "openapi", "--service", "Nope"]).assert().code(1).stderr(predicate::str::contains("known services"));
    let ir = veld(tmp.path()).args(["export", "ir"]).assert().success().get_output().stdout.clone();
    let ir: serde_json::Value = serde_json::from_slice(&ir).unwrap();
    assert!(ir["services"].is_array());
    let debug = veld(tmp.path()).args(["debug", "ir"]).assert().success().get_output().stdout.clone();
    assert_eq!(ir, serde_json::from_slice::<serde_json::Value>(&debug).unwrap());
}

// --- migrate -----------------------------------------------------------------------------

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            fs::copy(&src, &dst).unwrap();
        }
    }
}

fn migrate_example(name: &str) -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    let dir = tmp.path().join(name);
    copy_dir(&repo_root().join("examples").join(name).join("veld"), &dir);
    veld(&dir).arg("migrate").assert().success();
    veld(&dir).arg("check").assert().success();
    tmp
}

#[test]
fn migrated_microservices_example_passes_check() {
    let tmp = migrate_example("microservices");
    let dir = tmp.path().join("microservices");
    let toml = fs::read_to_string(dir.join("veld.toml")).unwrap();
    assert!(toml.contains("[services.accounts]") && toml.contains("consumes = [\"iam\"]"), "{toml}");
    assert!(toml.contains("# [[targets]]"), "python and go targets are commented out:\n{toml}");
    assert!(dir.join("veld.config.json").is_file(), "v1 files are left alone");
}

#[test]
fn migrated_node_react_example_passes_check_and_generates() {
    let tmp = migrate_example("node-react");
    let dir = tmp.path().join("node-react");
    veld(&dir).arg("generate").assert().success();
    assert!(tmp.path().join("backend/src/generated").is_dir());
}

#[test]
fn migrate_never_overwrites_and_dry_run_writes_nothing() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path().join("p");
    copy_dir(&repo_root().join("examples/node-react/veld"), &dir);
    let before = tree(&dir);
    veld(&dir).args(["migrate", "--dry-run"]).assert().success().stdout(predicate::str::contains("would write: veld.toml"));
    assert_eq!(before, tree(&dir), "dry run changes nothing");
    write(&dir, "veld.toml", "# mine\n");
    veld(&dir)
        .arg("migrate")
        .assert()
        .success()
        .stderr(predicate::str::contains("`veld.toml` already exists and was not overwritten"));
    assert_eq!(fs::read_to_string(dir.join("veld.toml")).unwrap(), "# mine\n");
    // a second run skips every file it wrote the first time
    let after_first = tree(&dir);
    veld(&dir).arg("migrate").assert().success();
    assert_eq!(after_first, tree(&dir));
}

#[test]
fn migrate_without_a_v1_config_is_a_user_error() {
    let tmp = tempdir().unwrap();
    veld(tmp.path()).arg("migrate").assert().code(1).stderr(predicate::str::contains("veld.config.json"));
}

//! `veld fmt`, `diff` and `export`, run through the real binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn init(dir: &Path) {
    veld(dir)
        .args(["init", "--name", "demo"])
        .assert()
        .success();
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
        .stdout(predicate::str::contains(
            "would reformat contracts/messy.veld",
        ));
    assert_eq!(
        fs::read_to_string(tmp.path().join("contracts/messy.veld")).unwrap(),
        MESSY,
        "--check never writes"
    );
    veld(tmp.path()).arg("fmt").assert().success();
    let formatted = fs::read_to_string(tmp.path().join("contracts/messy.veld")).unwrap();
    assert!(formatted.contains("model Greeting {"), "{formatted}");
    veld(tmp.path()).args(["fmt", "--check"]).assert().success();
    let names: Vec<String> = fs::read_dir(tmp.path().join("contracts"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().all(|n| !n.contains("veld-tmp")),
        "no temp files left: {names:?}"
    );
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
        .stderr(predicate::str::contains(
            "not formatted because of syntax errors",
        ));
    assert_eq!(
        fs::read_to_string(tmp.path().join("contracts/broken.veld")).unwrap(),
        broken
    );
    assert!(
        fs::read_to_string(tmp.path().join("contracts/messy.veld"))
            .unwrap()
            .contains("model Greeting {")
    );
}

#[test]
fn fmt_with_paths_touches_only_those_files_and_needs_no_project() {
    let tmp = tempdir().unwrap();
    write(tmp.path(), "a.veld", MESSY);
    write(tmp.path(), "b.veld", MESSY);
    veld(tmp.path()).args(["fmt", "a.veld"]).assert().success();
    assert!(
        fs::read_to_string(tmp.path().join("a.veld"))
            .unwrap()
            .contains("model Greeting {")
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("b.veld")).unwrap(),
        MESSY
    );
    veld(tmp.path())
        .args(["fmt", "missing.veld"])
        .assert()
        .code(1);
}

// --- lock file and diff ------------------------------------------------------------------

#[test]
fn generate_writes_a_deterministic_lock_and_diff_uses_it() {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    veld(tmp.path())
        .args(["diff"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("veld generate"));
    veld(tmp.path()).arg("generate").assert().success();
    let lock = fs::read(tmp.path().join("veld.lock.ir.json")).unwrap();
    veld(tmp.path()).arg("generate").assert().success();
    assert_eq!(
        lock,
        fs::read(tmp.path().join("veld.lock.ir.json")).unwrap(),
        "lock is deterministic"
    );
    veld(tmp.path())
        .arg("diff")
        .assert()
        .success()
        .stdout(predicate::str::contains("no changes"));

    write(tmp.path(), "contracts/hello.veld", BREAKING);
    veld(tmp.path())
        .arg("diff")
        .assert()
        .success()
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains("Non-breaking changes"));
    veld(tmp.path())
        .args(["diff", "--lock", "--strict"])
        .assert()
        .code(1);
    // a dry run and a single-target run do not move the baseline
    veld(tmp.path())
        .args(["generate", "--dry-run"])
        .assert()
        .success();
    assert_eq!(
        lock,
        fs::read(tmp.path().join("veld.lock.ir.json")).unwrap()
    );
}

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn diff_against_a_git_ref_never_touches_the_working_tree() {
    if !Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("skipped: git is not available");
        return;
    }
    let tmp = tempdir().unwrap();
    init(tmp.path());
    assert!(git(tmp.path(), &["init", "-q"]));
    assert!(git(tmp.path(), &["add", "."]));
    assert!(git(tmp.path(), &["commit", "-q", "-m", "base"]));
    write(tmp.path(), "contracts/hello.veld", BREAKING);
    let before = tree_excluding(tmp.path(), &[".git"]);
    veld(tmp.path())
        .args(["diff", "--against", "HEAD"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains("greet"));
    veld(tmp.path())
        .args(["diff", "--against", "HEAD", "--strict"])
        .assert()
        .code(1);
    assert_eq!(
        before,
        tree_excluding(tmp.path(), &[".git"]),
        "the working tree is untouched"
    );
    assert!(git(tmp.path(), &["status", "--porcelain"]));
    veld(tmp.path())
        .args(["diff", "--against", "no-such-ref"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not a git"));
}

// --- rule-aware diff ---------------------------------------------------------------------

const RULED: &str = "model Booking {\n  start: int\n  end: int\n  rule order: end > start\n}\n\nservice Bookings at \"/b\" {\n  post \"/\" book(body: Booking) -> 204\n}\n";

#[test]
fn diff_reports_a_tightened_invariant_with_both_expressions_and_fails_strict() {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    write(tmp.path(), "contracts/booking.veld", RULED);
    veld(tmp.path()).arg("generate").assert().success();
    veld(tmp.path())
        .args(["diff", "--strict"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no changes"));

    write(
        tmp.path(),
        "contracts/booking.veld",
        &RULED.replace("end > start", "end > start + 1"),
    );
    veld(tmp.path())
        .arg("diff")
        .assert()
        .success()
        .stdout(predicate::str::contains("Possibly breaking changes"))
        .stdout(predicate::str::contains(
            "rule `order` changed from `end > start` to `end > start + 1`",
        ));
    veld(tmp.path())
        .args(["diff", "--strict"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("`end > start`"))
        .stdout(predicate::str::contains("`end > start + 1`"));
}

// --- export ------------------------------------------------------------------------------

#[test]
fn export_openapi_and_ir() {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    let out = veld(tmp.path())
        .args(["export", "openapi"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(doc["openapi"], "3.1.0");
    veld(tmp.path())
        .args(["export", "openapi", "-o", "api.json", "--service", "Hello"])
        .assert()
        .success();
    let file: serde_json::Value =
        serde_json::from_slice(&fs::read(tmp.path().join("api.json")).unwrap()).unwrap();
    assert_eq!(file["openapi"], "3.1.0");
    veld(tmp.path())
        .args(["export", "openapi", "--service", "Nope"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("known services"));
    let ir = veld(tmp.path())
        .args(["export", "ir"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ir: serde_json::Value = serde_json::from_slice(&ir).unwrap();
    assert!(ir["services"].is_array());
    let debug = veld(tmp.path())
        .args(["debug", "ir"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        ir,
        serde_json::from_slice::<serde_json::Value>(&debug).unwrap()
    );
}

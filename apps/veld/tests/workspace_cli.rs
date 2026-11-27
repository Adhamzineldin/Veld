//! The workspace layout through the real CLI: members, `-p`, `--workspace`, the current member,
//! `--affected --since`, `owners`, and the error for the earlier layout (`spec/workspace.md`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::Path;
use std::process::Command;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

const ROOT: &str = "[workspace]\nname = \"shop\"\nmembers = [\"services/*\", \"apps/*\"]\n";
const ORDERS_TOML: &str = "[service]\nconsumes = [\"payments\"]\nowners = [\"team-orders\"]\n\n[server]\nemitter = \"node\"\n";
const PAYMENTS_TOML: &str = "[service]\nowners = [\"team-pay\"]\n\n[server]\nemitter = \"node\"\n";
const WEB_TOML: &str =
    "[app]\nowners = [\"team-web\"]\nuses = [\"orders\"]\n\n[client]\nemitter = \"typescript\"\n";
const ORDERS: &str = "model Order {\n  id: uuid\n}\n\n@auth(none)\nservice Orders at \"/orders\" {\n  get \"/{id}\" getOrder(id: uuid) -> Order | NotFound\n}\n";
const PAYMENTS: &str = "model Payment {\n  id: uuid\n  amount: int\n}\n\n@auth(none)\nservice Payments at \"/payments\" {\n  get \"/{id}\" getPayment(id: uuid) -> Payment | NotFound\n}\n";

fn shop(dir: &Path) {
    write(dir, "veld.toml", ROOT);
    write(dir, "services/orders/veld.toml", ORDERS_TOML);
    write(dir, "services/payments/veld.toml", PAYMENTS_TOML);
    write(dir, "apps/web/veld.toml", WEB_TOML);
    write(dir, "services/orders/contracts/orders.veld", ORDERS);
    write(dir, "services/payments/contracts/payments.veld", PAYMENTS);
}

fn generated(dir: &Path, member: &str) -> bool {
    dir.join(member).join("src/generated/index.ts").is_file()
}

#[test]
fn a_full_run_writes_every_member_and_the_ir_lock() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(tmp.path()).arg("generate").assert().success();
    for m in ["services/orders", "services/payments", "apps/web"] {
        assert!(generated(tmp.path(), m), "{m}");
    }
    assert!(tmp.path().join("veld.lock.ir.json").is_file());
}

#[test]
fn a_run_inside_a_member_works_on_that_member_and_writes_no_lock() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    let payments = tmp.path().join("services/payments");
    veld(&payments).arg("generate").assert().success();
    assert!(generated(tmp.path(), "services/payments"));
    assert!(!generated(tmp.path(), "services/orders"));
    assert!(!generated(tmp.path(), "apps/web"));
    assert!(
        !tmp.path().join("veld.lock.ir.json").exists(),
        "a partial run never writes the IR snapshot"
    );
    // `--workspace` takes every member, also from inside one.
    veld(&payments)
        .args(["generate", "--workspace"])
        .assert()
        .success();
    assert!(generated(tmp.path(), "apps/web"));
    assert!(tmp.path().join("veld.lock.ir.json").is_file());
}

#[test]
fn package_flags_select_members_whatever_their_kind() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(tmp.path())
        .args(["generate", "-p", "web", "--package", "payments"])
        .assert()
        .success();
    assert!(generated(tmp.path(), "apps/web"));
    assert!(generated(tmp.path(), "services/payments"));
    assert!(!generated(tmp.path(), "services/orders"));
    assert!(!tmp.path().join("veld.lock.ir.json").exists());
}

#[test]
fn an_unknown_member_is_e0803_and_lists_the_members() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(tmp.path())
        .args(["generate", "-p", "nope"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0803"))
        .stderr(predicate::str::contains("web, orders, payments"));
    assert!(!generated(tmp.path(), "services/orders"));
}

#[test]
fn a_target_of_a_member_that_is_not_selected_is_e0803() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(&tmp.path().join("services/orders"))
        .args(["generate", "payments"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0803"))
        .stderr(predicate::str::contains("not selected"));
    veld(&tmp.path().join("services/orders"))
        .args(["generate", "orders"])
        .assert()
        .success();
}

#[test]
fn the_selection_flags_exclude_each_other() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(tmp.path())
        .args(["generate", "-p", "web", "--workspace"])
        .assert()
        .code(2);
    veld(tmp.path())
        .args(["generate", "--affected"])
        .assert()
        .code(2);
    veld(tmp.path())
        .args(["generate", "--since", "main"])
        .assert()
        .code(2);
}

#[test]
fn clean_and_setup_follow_the_selection() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(tmp.path()).arg("generate").assert().success();
    veld(tmp.path())
        .args(["clean", "-p", "payments"])
        .assert()
        .success();
    assert!(!generated(tmp.path(), "services/payments"));
    assert!(generated(tmp.path(), "services/orders"));
    assert!(tmp.path().join("veld.lock.ir.json").is_file());
    veld(tmp.path())
        .args(["clean", "-p", "payments", "--lock"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("whole workspace"));
}

#[test]
fn the_earlier_layout_is_e0709_and_says_what_to_write() {
    let tmp = tempdir().unwrap();
    write(
        tmp.path(),
        "veld.toml",
        "[project]\nname = \"x\"\n\n[[targets]]\nrole = \"client\"\nemitter = \"typescript\"\nout = \"gen\"\n",
    );
    veld(tmp.path())
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0709"))
        .stderr(predicate::str::contains("`[project]`"))
        .stderr(predicate::str::contains("`[[targets]]`"))
        .stderr(predicate::str::contains("`[workspace] name`"))
        .stderr(predicate::str::contains("`[client]` in an `[app]` member"));
}

#[test]
fn check_graph_json_carries_owners_only_when_there_are_some() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    let json = |dir: &Path| {
        let out = veld(dir)
            .args(["check", "--graph=json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<serde_json::Value>(&out).unwrap()
    };
    let v = json(tmp.path());
    let owners: Vec<_> = v["services"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["name"].as_str().unwrap().to_string(), s["owners"].clone()))
        .collect();
    assert_eq!(
        owners[0],
        ("orders".to_string(), serde_json::json!(["team-orders"]))
    );
    assert_eq!(
        owners[1],
        ("payments".to_string(), serde_json::json!(["team-pay"]))
    );
    // Without owners the nodes are what they were.
    write(
        tmp.path(),
        "services/orders/veld.toml",
        &ORDERS_TOML.replace("owners = [\"team-orders\"]\n", ""),
    );
    write(
        tmp.path(),
        "services/payments/veld.toml",
        &PAYMENTS_TOML.replace("owners = [\"team-pay\"]\n", ""),
    );
    let v = json(tmp.path());
    assert!(v["services"][0].get("owners").is_none(), "{v}");
}

#[test]
fn diff_names_the_owners_and_who_depends_on_the_changed_package() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(tmp.path()).arg("generate").assert().success();
    // Removing a field is a breaking change in `payments`, which `orders` consumes.
    write(
        tmp.path(),
        "services/payments/contracts/payments.veld",
        &PAYMENTS.replace("  amount: int\n", ""),
    );
    veld(tmp.path())
        .arg("diff")
        .assert()
        .success()
        .stdout(predicate::str::contains("Who is affected:"))
        .stdout(predicate::str::contains("owners: team-pay"))
        .stdout(predicate::str::contains("affects orders (team-orders)"));
    // A workspace with no owners prints the report it always did.
    write(
        tmp.path(),
        "services/orders/veld.toml",
        &ORDERS_TOML.replace("owners = [\"team-orders\"]\n", ""),
    );
    write(
        tmp.path(),
        "services/payments/veld.toml",
        &PAYMENTS_TOML.replace("owners = [\"team-pay\"]\n", ""),
    );
    write(
        tmp.path(),
        "apps/web/veld.toml",
        &WEB_TOML.replace("owners = [\"team-web\"]\n", ""),
    );
    veld(tmp.path())
        .arg("diff")
        .assert()
        .success()
        .stdout(predicate::str::contains("Who is affected:").not());
}

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(args)
        .current_dir(dir)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn affected_members_come_from_git_and_say_why() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    shop(dir);
    if !git(dir, &["init", "-q"]) {
        eprintln!("skipped: git is not available");
        return;
    }
    assert!(git(dir, &["add", "."]));
    assert!(git(dir, &["commit", "-q", "-m", "base"]));
    // Nothing changed: nothing is affected, and the run is a success that writes nothing.
    veld(dir)
        .args(["generate", "--affected", "--since", "HEAD"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing is affected since HEAD"));
    assert!(!generated(dir, "services/orders"));
    // A contract of `payments` changes (not committed): payments, orders (consumes) and web
    // (uses orders) are affected, with one reason each.
    write(
        dir,
        "services/payments/contracts/payments.veld",
        &PAYMENTS.replace("amount: int", "amount: int\n  note?: string"),
    );
    let out = veld(dir)
        .args([
            "generate",
            "--affected",
            "--since",
            "HEAD",
            "--accept-breaking",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("affected since HEAD:"), "{text}");
    for line in [
        "payments   contracts changed",
        "orders     consumes payments",
        "web        uses orders",
    ] {
        assert!(text.lines().any(|l| l.trim() == line), "{line}\n{text}");
    }
    assert!(generated(dir, "services/payments") && generated(dir, "services/orders"));
    assert!(generated(dir, "apps/web"));
    // Every member is affected here, so this run selected every member: a full run.
    assert!(dir.join("veld.lock.ir.json").is_file());
}

#[test]
fn a_partial_affected_run_writes_no_lock() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    shop(dir);
    if !git(dir, &["init", "-q"]) {
        eprintln!("skipped: git is not available");
        return;
    }
    assert!(git(dir, &["add", "."]));
    assert!(git(dir, &["commit", "-q", "-m", "base"]));
    // Only `web` changes, and nothing depends on an app.
    write(
        dir,
        "apps/web/veld.toml",
        &format!("{WEB_TOML}\n# touched\n"),
    );
    veld(dir)
        .args(["generate", "--affected", "--since", "HEAD"])
        .assert()
        .success()
        .stdout(predicate::str::contains("web   veld.toml changed"));
    assert!(generated(dir, "apps/web"));
    assert!(!generated(dir, "services/orders"));
    assert!(!dir.join("veld.lock.ir.json").exists());
}

#[test]
fn affected_needs_git_and_a_known_ref() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    shop(dir);
    // The temporary directory is not inside a git work tree (unless the machine's temp folder is
    // in one, which we cannot rule out): only assert on the shape of the error.
    let out = veld(dir)
        .args([
            "generate",
            "--affected",
            "--since",
            "no-such-ref",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("E0712"), "{err}");
    assert!(!generated(dir, "services/orders"));
}

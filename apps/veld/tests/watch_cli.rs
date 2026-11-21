//! `veld watch` end to end: regenerate on edit, show diagnostics without dying, recover.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use common::*;
use tempfile::tempdir;
use veld_test_support::ProcessTree;

fn log_of(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn wait_until(what: &str, log: &Path, ok: impl Fn(&str) -> bool) -> String {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let text = log_of(log);
        if ok(&text) {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}; log:\n{text}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn any_file_contains(dir: &Path, needle: &str) -> bool {
    tree(dir)
        .values()
        .any(|b| String::from_utf8_lossy(b).contains(needle))
}

fn count(text: &str, needle: &str) -> usize {
    text.matches(needle).count()
}

#[test]
fn watch_regenerates_reports_errors_inline_and_recovers() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    veld(dir)
        .args(["init", "--name", "demo", "--client", "none"])
        .assert()
        .success();
    // The log lives outside the project so it cannot be mistaken for a contract.
    let log_dir = tempdir().unwrap();
    let log = log_dir.path().join("watch.log");

    let exe = assert_cmd::cargo::cargo_bin("veld");
    let mut cmd = Command::new(exe);
    cmd.current_dir(dir)
        .env("NO_COLOR", "1")
        .args(["watch", "--debounce", "100"]);
    let mut tree_guard = ProcessTree::spawn_logged(&mut cmd, &log).unwrap();

    // The first run is announced, with the watched paths in the header.
    let text = wait_until("the first run", &log, |t| t.contains("ready in"));
    assert!(
        text.contains("veld.toml") && text.contains("contracts"),
        "{text}"
    );
    assert!(text.contains("watching for changes"), "{text}");
    assert!(any_file_contains(&dir.join("generated"), "Greeting"));

    // An edit regenerates, with a timestamp and the changed file.
    let contract = dir.join("contracts/hello.veld");
    let original = fs::read_to_string(&contract).unwrap();
    fs::write(
        &contract,
        original.replace("message: string", "message: string\n  shoutedAt: string"),
    )
    .unwrap();
    let text = wait_until("the regeneration", &log, |t| t.contains("regenerated in"));
    assert!(text.contains("changed contracts"), "{text}");
    assert!(text.contains("hello.veld"), "{text}");
    assert!(any_file_contains(&dir.join("generated"), "shoutedAt"));

    // An error shows its diagnostic and the process stays alive.
    fs::write(
        &contract,
        original.replace("message: string", "message: NoSuchType"),
    )
    .unwrap();
    let text = wait_until("the diagnostic", &log, |t| t.contains("failed after"));
    assert!(text.contains("error[E"), "{text}");
    assert!(
        tree_guard.child_mut().try_wait().unwrap().is_none(),
        "watch must survive a broken contract"
    );

    // Fixing it regenerates again.
    let before = count(&log_of(&log), "regenerated in");
    fs::write(&contract, &original).unwrap();
    wait_until("the recovery", &log, |t| {
        count(t, "regenerated in") > before
    });
    assert!(!any_file_contains(&dir.join("generated"), "shoutedAt"));

    tree_guard.kill_tree();
}

/// `generate --watch` is the canonical spelling; breaking edits do not stop a watch.
#[test]
fn generate_watch_regenerates_and_does_not_block_on_breaking_changes() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    veld(dir)
        .args(["init", "--name", "demo", "--client", "none"])
        .assert()
        .success();
    let log_dir = tempdir().unwrap();
    let log = log_dir.path().join("watch.log");
    let exe = assert_cmd::cargo::cargo_bin("veld");
    let mut cmd = Command::new(exe);
    cmd.current_dir(dir)
        .env("NO_COLOR", "1")
        .args(["generate", "--watch"]);
    let mut guard = ProcessTree::spawn_logged(&mut cmd, &log).unwrap();
    wait_until("the first run", &log, |t| t.contains("ready in"));
    assert!(any_file_contains(&dir.join("generated"), "Greeting"));

    // Renaming the path parameter is a breaking change: `generate` would refuse, a watch goes on.
    let contract = dir.join("contracts/hello.veld");
    let original = fs::read_to_string(&contract).unwrap();
    fs::write(
        &contract,
        original
            .replace("{name}", "{who}")
            .replace("greet(name:", "greet(who:"),
    )
    .unwrap();
    let text = wait_until("the regeneration", &log, |t| t.contains("regenerated in"));
    assert!(text.contains("breaking changes"), "{text}");
    assert!(any_file_contains(&dir.join("generated"), "who"));
    guard.kill_tree();
}

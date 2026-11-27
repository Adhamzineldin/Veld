//! Output style: plain without a terminal, no colour with NO_COLOR, headers and hints.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn no_ansi() -> impl Predicate<str> {
    predicate::str::contains('\x1b').not()
}

#[test]
fn init_yes_prints_plain_output_with_next_steps() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--yes", "--name", "demo"])
        .assert()
        .success()
        .stdout(no_ansi())
        .stderr(no_ansi())
        .stdout(predicate::str::contains("veld"))
        .stdout(predicate::str::contains("Next steps"))
        .stdout(predicate::str::contains("veld generate"))
        .stdout(predicate::str::contains("veld setup"))
        .stdout(predicate::str::contains("npm install"));
}

#[test]
fn bare_init_without_a_terminal_never_prompts() {
    let tmp = tempdir().unwrap();
    // stdin is not a terminal here: the flow must not wait for answers.
    veld(tmp.path())
        .arg("init")
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success()
        .stdout(no_ansi());
    assert!(tmp.path().join("veld.toml").is_file());
}

#[test]
fn init_flags_cover_every_guided_answer() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args([
            "init",
            "--name",
            "shop",
            "--workspace",
            "users,orders",
            "--server",
            "java:spring",
            "--client",
            "typescript:react,java",
            "--out",
            "web=ui/generated",
            "--description",
            "Shop APIs",
        ])
        .assert()
        .success();
    let toml = std::fs::read_to_string(tmp.path().join("veld.toml")).unwrap();
    assert!(toml.contains("description = \"Shop APIs\""), "{toml}");
    // A target writes `out` relative to its member's directory.
    let web = std::fs::read_to_string(tmp.path().join("web/veld.toml")).unwrap();
    assert!(web.contains("out = \"../ui/generated\""), "{web}");
    let java = std::fs::read_to_string(tmp.path().join("clients/java/veld.toml")).unwrap();
    assert!(java.contains("java-client"), "{java}");
    veld(tmp.path()).arg("check").assert().success();
}

#[test]
fn init_dry_run_lists_files_and_writes_nothing() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("would write: veld.toml"));
    assert!(!tmp.path().join("veld.toml").exists());
}

#[test]
fn generate_output_has_a_header_summary_and_timing_without_ansi() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--yes", "--name", "demo"])
        .assert()
        .success();
    veld(tmp.path())
        .arg("generate")
        .assert()
        .success()
        .stdout(no_ansi())
        .stderr(no_ansi())
        .stdout(predicate::str::contains("generate"))
        .stdout(predicate::str::contains("created"))
        .stdout(predicate::str::contains("2 targets"))
        .stdout(predicate::str::contains(" in "));
}

#[test]
fn colour_never_appears_when_output_is_piped_or_no_color_is_set() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--yes", "--name", "demo"])
        .assert()
        .success();
    // Even when asked for colour by the environment, a pipe stays plain.
    veld(tmp.path())
        .env_remove("NO_COLOR")
        .env("CLICOLOR_FORCE", "1")
        .arg("check")
        .assert()
        .success()
        .stdout(no_ansi());
    veld(tmp.path())
        .args(["--no-color", "check"])
        .assert()
        .success()
        .stdout(no_ansi());
}

#[test]
fn a_failing_check_keeps_the_rustc_style_diagnostic() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--yes", "--name", "demo"])
        .assert()
        .success();
    write(
        tmp.path(),
        "contracts/hello.veld",
        "model A {\n  b: Missing\n}\n",
    );
    veld(tmp.path())
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error[E"))
        .stderr(predicate::str::contains("hello.veld"))
        .stderr(predicate::str::contains("✗"))
        .stderr(no_ansi());
}

#[test]
fn watch_is_a_hidden_alias_that_still_works() {
    let out = veld(tempdir().unwrap().path())
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(
        !help.lines().any(|l| l.trim_start().starts_with("watch")),
        "`watch` must not be listed:\n{help}"
    );
    veld(tempdir().unwrap().path())
        .args(["watch", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--debounce"));
}

//! `veld check --prove` and `veld diff --prove` through the real binary (`spec/rules.md`
//! section 18).
//!
//! The tests that need a solver look for one the way the command does (`VELD_SOLVER`, then `z3`
//! and `cvc5` on `PATH`). Without one they say so and pass, unless `VELD_REQUIRE_SOLVER=1` is set.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::Path;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

const CONTRADICTION: &str = r#"model Booking {
  start: datetime
  end: datetime
  guests: int @min(1) @max(10)
  rule endAfterStart: end > start
  rule endBeforeStart: end < start
}

model Account {
  id: uuid
  age: int
}

principal Account

policy Nobody(user: Account) {
  permit when user.age > 10 && user.age < 5
}

@auth(bearer)
service Bookings at "/bookings" {
  post "/" create(body: Booking) -> 201 Booking {
    require fine: body.guests > 2
  }
}
"#;

const SOUND: &str = r#"model Booking {
  guests: int @min(1) @max(10)
  rule few: guests < 8 && guests > 2
}

model Account {
  id: uuid
  age: int
}

principal Account

policy Adults(user: Account) {
  permit when user.age >= 18
}

@auth(bearer)
service Bookings at "/bookings" {
  post "/" create(body: Booking) -> 201 Booking {
    allow Adults(user)
    require fine: body.guests > 2
  }
}
"#;

fn project(contract: &str) -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    write(tmp.path(), "contracts/hello.veld", contract);
    tmp
}

/// True when a solver is available; otherwise says so, or fails under `VELD_REQUIRE_SOLVER=1`.
fn have_solver(test: &str) -> bool {
    match veld_prove::find_in_environment(None) {
        Ok(_) => true,
        Err(e) => {
            assert!(
                std::env::var("VELD_REQUIRE_SOLVER").as_deref() != Ok("1"),
                "{test}: VELD_REQUIRE_SOLVER=1 but {e}"
            );
            eprintln!("{test}: skipped, {e}");
            false
        }
    }
}

fn without_solver(dir: &Path) -> assert_cmd::Command {
    let mut cmd = veld(dir);
    cmd.env("PATH", "").env_remove("VELD_SOLVER");
    cmd
}

// ---- no solver ----------------------------------------------------------------------------

#[test]
fn without_a_solver_prove_says_so_in_one_line_and_check_goes_on() {
    let tmp = project(CONTRADICTION);
    let out = without_solver(tmp.path())
        .args(["check", "--prove", "--no-lint"])
        .assert()
        .success()
        .stderr(predicate::str::contains("no SMT solver found"))
        .stderr(predicate::str::contains("E0970").not())
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr.matches("--prove:").count(),
        1,
        "one line, not more: {stderr}"
    );
}

#[test]
fn the_exit_code_without_a_solver_is_that_of_check() {
    let tmp = project("model Broken {\n a: nope\n}\n");
    without_solver(tmp.path())
        .args(["check", "--prove"])
        .assert()
        .code(1);
}

#[test]
fn a_solver_that_is_named_and_missing_is_an_error_line_not_a_search() {
    let tmp = project(CONTRADICTION);
    without_solver(tmp.path())
        .args(["check", "--prove", "--no-lint", "--solver", "no/such/z3"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "the solver `no/such/z3` does not exist",
        ));
}

#[test]
fn the_prove_options_need_prove() {
    let tmp = project(SOUND);
    veld(tmp.path())
        .args(["check", "--solver", "z3"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--prove"));
    veld(tmp.path())
        .args(["diff", "--prove-timeout", "3"])
        .assert()
        .failure();
}

#[test]
fn the_help_documents_prove_and_never_says_verified_or_correct() {
    let tmp = project(SOUND);
    for command in ["check", "diff"] {
        let out = veld(tmp.path())
            .args([command, "--help"])
            .assert()
            .success();
        let text = String::from_utf8_lossy(&out.get_output().stdout).to_lowercase();
        for flag in ["--prove", "--solver", "--prove-timeout"] {
            assert!(
                text.contains(flag),
                "{command} --help lacks {flag}:\n{text}"
            );
        }
        assert!(
            !text.contains("verified") && !text.contains("correct"),
            "{text}"
        );
    }
}

// ---- with a solver ------------------------------------------------------------------------

#[test]
fn check_prove_finds_the_contradictions_and_fails() {
    if !have_solver("check_prove_finds_the_contradictions_and_fails") {
        return;
    }
    let tmp = project(CONTRADICTION);
    let out = veld(tmp.path())
        .args(["check", "--prove", "--no-lint"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "error[E0970]: no value of `Booking` can exist",
        ))
        .stderr(predicate::str::contains("rule `endAfterStart`"))
        .stderr(predicate::str::contains("rule `endBeforeStart`"))
        .stderr(predicate::str::contains(
            "the policy `Nobody` permits nobody",
        ))
        .stdout(predicate::str::contains("prove: "))
        .get_output()
        .clone();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .to_lowercase();
    assert!(
        !all.contains("verified") && !all.contains("correct"),
        "{all}"
    );
    assert!(all.contains("proved"), "{all}");
}

#[test]
fn check_prove_passes_a_contract_with_consistent_rules() {
    if !have_solver("check_prove_passes_a_contract_with_consistent_rules") {
        return;
    }
    let tmp = project(SOUND);
    veld(tmp.path())
        .args(["check", "--prove", "--no-lint"])
        .assert()
        .success()
        .stderr(predicate::str::contains("E0970").not())
        .stdout(predicate::str::contains(
            "3 questions: 0 proved, 3 counterexample",
        ));
}

#[test]
fn a_finding_outside_the_scope_of_the_paths_is_not_shown() {
    if !have_solver("a_finding_outside_the_scope_of_the_paths_is_not_shown") {
        return;
    }
    let tmp = project(CONTRADICTION);
    write(
        tmp.path(),
        "contracts/other.veld",
        "model Other {\n n: int\n}\n",
    );
    veld(tmp.path())
        .args(["check", "--prove", "--no-lint", "contracts/other.veld"])
        .assert()
        .success()
        .stderr(predicate::str::contains("E0970").not());
}

#[test]
fn diff_prove_says_which_rules_got_tighter_and_leaves_the_rest_alone() {
    if !have_solver("diff_prove_says_which_rules_got_tighter_and_leaves_the_rest_alone") {
        return;
    }
    let tmp = project(SOUND);
    veld(tmp.path()).arg("generate").assert().success();
    let tighter = SOUND
        .replace("guests < 8", "guests < 5")
        .replace("body.guests > 2", "body.guests > 4");
    write(tmp.path(), "contracts/hello.veld", &tighter);
    // Without --prove nothing changes: both are possibly breaking.
    let plain = veld(tmp.path())
        .arg("diff")
        .assert()
        .success()
        .get_output()
        .clone();
    let plain = String::from_utf8_lossy(&plain.stdout).into_owned();
    assert!(plain.contains("Possibly breaking changes"), "{plain}");
    assert!(!plain.contains("tightened"), "{plain}");
    // With it, each is judged.
    let proved = veld(tmp.path())
        .args(["diff", "--prove"])
        .assert()
        .success()
        .get_output()
        .clone();
    let text = String::from_utf8_lossy(&proved.stdout).into_owned();
    assert!(text.contains("Breaking changes (2)"), "{text}");
    assert!(text.contains("rule `few` changed from `guests < 8 && guests > 2` to `guests < 5 && guests > 2`; tightened"), "{text}");
    assert!(
        text.contains("the old rule accepted {\"guests\":5}"),
        "{text}"
    );
    assert!(
        text.contains(
            "require `fine` changed from `body.guests > 2` to `body.guests > 4`; tightened"
        ),
        "{text}"
    );
    assert!(
        text.contains("prove: 2 questions: 0 proved, 2 counterexample"),
        "{text}"
    );
    // `--strict` now fails on the proved breaking change.
    veld(tmp.path())
        .args(["diff", "--prove", "--strict"])
        .assert()
        .code(1);
}

#[test]
fn diff_prove_proves_a_loosened_rule_is_not_tightened() {
    if !have_solver("diff_prove_proves_a_loosened_rule_is_not_tightened") {
        return;
    }
    let tmp = project(SOUND);
    veld(tmp.path()).arg("generate").assert().success();
    write(
        tmp.path(),
        "contracts/hello.veld",
        &SOUND.replace("body.guests > 2", "body.guests > 1"),
    );
    veld(tmp.path())
        .args(["diff", "--prove"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "not tightened: every value the old rule accepted is still accepted",
        ))
        .stdout(predicate::str::contains("Non-breaking changes"))
        .stdout(predicate::str::contains("1 proved, 0 counterexample"));
}

#[test]
fn diff_prove_without_changed_rules_asks_nothing() {
    let tmp = project(SOUND);
    veld(tmp.path()).arg("generate").assert().success();
    // No solver is needed, and none is looked for, when no rule changed.
    without_solver(tmp.path())
        .args(["diff", "--prove"])
        .assert()
        .success()
        .stderr(predicate::str::contains("no SMT solver").not());
}

#[test]
fn the_contracts_are_left_alone() {
    let tmp = project(SOUND);
    let before = tree(tmp.path());
    without_solver(tmp.path())
        .args(["check", "--prove", "--no-lint"])
        .assert()
        .success();
    assert_same_tree(&before, &tree(tmp.path()), "veld check --prove");
}

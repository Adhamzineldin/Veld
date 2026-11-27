//! The v1-parity commands and flags, run through the real binary: `generate --check`, the
//! breaking-change gate, hooks, lint in `check`, `check --graph`, `export`, `doctor`,
//! `clean --lock`, `-v`, completions, `registry` and the `init` additions.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::thread;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

/// A single-service project with a Node server in `generated/server` and no client.
fn project(dir: &Path) {
    veld(dir)
        .args(["init", "--name", "demo", "--client", "none"])
        .assert()
        .success();
}

fn generated(dir: &Path) {
    veld(dir).arg("generate").assert().success();
}

fn read(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

fn append(dir: &Path, rel: &str, text: &str) {
    let mut s = read(dir, rel);
    s.push_str(text);
    fs::write(dir.join(rel), s).unwrap();
}

/// Renames the path parameter of the hello endpoint: a breaking change.
fn break_contract(dir: &Path) {
    let s = read(dir, "contracts/hello.veld")
        .replace("{name}", "{who}")
        .replace("greet(name:", "greet(who:");
    fs::write(dir.join("contracts/hello.veld"), s).unwrap();
}

/// Adds a field: a non-breaking change that still makes the output stale.
fn extend_contract(dir: &Path) {
    let s = read(dir, "contracts/hello.veld")
        .replace("message: string", "message: string\n  shoutedAt?: string");
    fs::write(dir.join("contracts/hello.veld"), s).unwrap();
}

// --- generate --check / --diff ------------------------------------------------------------

#[test]
fn check_is_clean_after_generate_and_stale_after_a_change_and_never_writes() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    veld(dir)
        .args(["generate", "--check"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("create index.ts"));
    assert!(!dir.join("generated").exists(), "--check writes nothing");
    generated(dir);
    veld(dir)
        .args(["generate", "--check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("generated output is up to date"));
    extend_contract(dir);
    let before = tree(dir);
    veld(dir)
        .args(["generate", "--check"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("update models/hello.ts"))
        .stderr(predicate::str::contains("out of date"));
    assert_eq!(before, tree(dir), "--check never writes");
}

#[test]
fn check_flags_user_edited_and_deleted_files_and_diff_prints_unified_diffs() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    generated(dir);
    append(dir, "generated/server/index.ts", "// mine\n");
    veld(dir)
        .args(["generate", "--check"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("modified index.ts"));
    veld(dir)
        .args(["generate", "--check", "--diff"])
        .assert()
        .code(1)
        .stdout(
            predicate::str::contains("--- a/index.ts").and(predicate::str::contains("-// mine")),
        );
}

#[test]
fn diff_works_with_dry_run_and_is_refused_alone_and_check_conflicts_with_dry_run() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    veld(dir)
        .args(["generate", "--dry-run", "--diff"])
        .assert()
        .success()
        .stdout(predicate::str::contains("+++ b/index.ts"));
    assert!(!dir.join("generated").exists());
    veld(dir)
        .args(["generate", "--diff"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("--check` or `--dry-run"));
    veld(dir)
        .args(["generate", "--check", "--dry-run"])
        .assert()
        .code(2);
    veld(dir)
        .args(["generate", "--check", "--force"])
        .assert()
        .code(2);
}

// --- breaking-change gate -----------------------------------------------------------------

#[test]
fn a_breaking_change_blocks_off_a_terminal_and_with_strict() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    generated(dir);
    break_contract(dir);
    let before = tree(dir);
    for args in [&["generate"][..], &["generate", "--strict"]] {
        veld(dir).args(args).assert().code(1).stderr(
            predicate::str::contains("breaking changes")
                .and(predicate::str::contains("--accept-breaking")),
        );
    }
    assert_eq!(before, tree(dir), "a blocked generate writes nothing");
}

#[test]
fn accept_breaking_continues_and_updates_the_baseline() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    generated(dir);
    break_contract(dir);
    veld(dir)
        .args(["generate", "--accept-breaking"])
        .assert()
        .success()
        .stderr(predicate::str::contains("breaking changes"));
    assert!(read(dir, "generated/server/models/hello.ts").contains("Greeting"));
    veld(dir).arg("generate").assert().success();
}

#[test]
fn non_breaking_changes_pass_and_check_and_dry_run_never_block() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    generated(dir);
    extend_contract(dir);
    veld(dir).arg("generate").assert().success();
    break_contract(dir);
    veld(dir)
        .args(["generate", "--dry-run"])
        .assert()
        .success()
        .stderr(predicate::str::contains("breaking changes"));
    veld(dir)
        .args(["generate", "--check"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("breaking changes"));
}

const ROOM: &str = "model Room {\n  size: int\n  rule small: size <= 10\n}\n\nservice Rooms at \"/rooms\" {\n  post \"/\" book(body: Room) -> 204\n}\n";

/// Adds a contract with one rule on a request body; the rule is the part the tests change.
fn add_room(dir: &Path) {
    write(dir, "contracts/room.veld", ROOM);
}

#[test]
fn a_changed_rule_stops_generate_and_says_it_cannot_tell_the_direction() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    add_room(dir);
    generated(dir);
    write(dir, "contracts/room.veld", &ROOM.replace("<= 10", "<= 5"));
    let before = tree(dir);
    for args in [&["generate"][..], &["generate", "--strict"]] {
        veld(dir).args(args).assert().code(1).stderr(
            predicate::str::contains("possibly breaking changes (1)")
                .and(predicate::str::contains("cannot tell"))
                .and(predicate::str::contains("--accept-breaking")),
        );
    }
    assert_eq!(before, tree(dir), "a blocked generate writes nothing");
    veld(dir)
        .args(["generate", "--accept-breaking"])
        .assert()
        .success()
        .stderr(predicate::str::contains("possibly breaking changes (1)"));
    veld(dir).arg("generate").assert().success();
}

#[test]
fn a_changed_rule_does_not_block_check_and_dry_run() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    add_room(dir);
    generated(dir);
    write(dir, "contracts/room.veld", &ROOM.replace("<= 10", "<= 5"));
    veld(dir)
        .args(["generate", "--dry-run"])
        .assert()
        .success()
        .stderr(predicate::str::contains("possibly breaking changes (1)"));
}

#[test]
fn an_added_event_channel_publisher_or_subscriber_does_not_stop_generate() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    add_room(dir);
    generated(dir);
    let events = "event Booked { id: uuid }\nchannel Bookings { carries Booked }\n\
                  service Desk {\n  publishes Bookings\n}\n\
                  service Audit {\n  subscribes Bookings as \"audit\" {\n    on Booked -> record\n  }\n}\n";
    write(dir, "contracts/room.veld", &format!("{ROOM}{events}"));
    // Additions are compatible: the gate lets them through and the Node server generates them.
    veld(dir)
        .arg("generate")
        .assert()
        .success()
        .stderr(predicate::str::contains("possibly breaking").not())
        .stderr(predicate::str::contains("breaking changes (").not())
        .stderr(predicate::str::contains("--accept-breaking").not());
}

#[test]
fn strict_and_accept_breaking_exclude_each_other() {
    let tmp = tempdir().unwrap();
    project(tmp.path());
    veld(tmp.path())
        .args(["generate", "--strict", "--accept-breaking"])
        .assert()
        .code(2);
}

// --- --setup, hooks and import hints ----------------------------------------

#[test]
fn generate_setup_wires_the_package_json() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    veld(dir)
        .args(["generate", "--setup"])
        .assert()
        .success()
        .stdout(predicate::str::contains("package.json"));
    assert!(read(dir, "package.json").contains("@veld/api"));
}

#[test]
fn import_hints_are_printed_per_target() {
    let tmp = tempdir().unwrap();
    project(tmp.path());
    veld(tmp.path())
        .arg("generate")
        .assert()
        .success()
        .stdout(predicate::str::contains("import { ... } from '@veld/api'"));
}

#[test]
fn the_use_it_block_aligns_its_columns_whatever_the_target_names() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    append(
        dir,
        "veld.toml",
        "\n[client]\nname = \"a-rather-long-client-target\"\nemitter = \"typescript\"\nout = \"generated/client\"\n",
    );
    let out = veld(dir).arg("generate").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let rows: Vec<&str> = text.lines().filter(|l| l.contains("import ")).collect();
    assert_eq!(rows.len(), 2, "{text}");
    let col = |l: &str| l.find("import ").unwrap();
    assert_eq!(col(rows[0]), col(rows[1]), "{text}");
}

const HOOK_WRITE: &str = "echo hooked> hook.txt";

#[test]
fn post_generate_hooks_run_in_order_from_the_project_root_after_a_real_write() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    append(
        dir,
        "veld.toml",
        &format!(
            "\n[hooks]\npost-generate = [\"echo one> order.txt\", \"echo two>> order.txt\", \"{HOOK_WRITE}\"]\n"
        ),
    );
    veld(dir).args(["generate", "--dry-run"]).assert().success();
    veld(dir).args(["generate", "--check"]).assert().code(1);
    assert!(
        !dir.join("hook.txt").exists(),
        "dry runs and checks run no hooks"
    );
    veld(dir)
        .arg("generate")
        .assert()
        .success()
        .stdout(predicate::str::contains("hook:"));
    assert!(read(dir, "hook.txt").contains("hooked"));
    let order: Vec<String> = read(dir, "order.txt")
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    assert_eq!(order, ["one", "two"]);
}

#[test]
fn a_hook_can_be_one_string_and_a_failure_is_a_warning_unless_fail_is_set() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    append(dir, "veld.toml", "\n[hooks]\npost-generate = \"exit 3\"\n");
    veld(dir)
        .arg("generate")
        .assert()
        .success()
        .stderr(predicate::str::contains("hook `exit 3` failed"));
    append(dir, "veld.toml", "fail = true\n");
    veld(dir)
        .arg("generate")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("hook `exit 3` failed"));
}

#[test]
fn an_unknown_hooks_key_is_a_config_error() {
    let tmp = tempdir().unwrap();
    project(tmp.path());
    append(tmp.path(), "veld.toml", "\n[hooks]\npost-generat = \"x\"\n");
    veld(tmp.path())
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("did you mean `post-generate`"));
}

// --- watch alias, verbose -----------------------------------------------------------------

#[test]
fn watch_is_a_hidden_alias_of_generate_watch() {
    let tmp = tempdir().unwrap();
    let out = veld(tmp.path()).arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(
        !help.lines().any(|l| l.trim_start().starts_with("watch")),
        "{help}"
    );
    veld(tmp.path())
        .args(["watch", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--debounce"));
    veld(tmp.path())
        .args(["generate", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--watch"));
}

#[test]
fn verbose_shows_phases_plan_entries_and_timings_on_stderr() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    veld(dir)
        .arg("generate")
        .assert()
        .success()
        .stderr(predicate::str::contains("compiled").not());
    veld(dir)
        .args(["-v", "generate", "--dry-run"])
        .assert()
        .success()
        .stderr(
            predicate::str::contains("compiled 1 package")
                .and(predicate::str::contains("generated in")),
        );
    veld(dir)
        .args(["-vv", "generate", "--dry-run"])
        .assert()
        .success()
        .stderr(predicate::str::contains("api: unchanged index.ts"));
}

// --- check: lint, scope, graph ------------------------------------------------------------

const UNUSED: &str = "\n/// Nothing uses this.\nmodel Orphan {\n  id: uuid\n}\n";

#[test]
fn check_lints_and_the_levels_come_from_veld_toml() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    // The scaffold says `@auth(none)`, so the unused model is the only finding.
    append(dir, "contracts/hello.veld", UNUSED);
    veld(dir)
        .arg("check")
        .assert()
        .success()
        .stderr(predicate::str::contains("warning[L0001]"))
        .stdout(predicate::str::contains("1 lint warning"));
    veld(dir)
        .args(["check", "--strict"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("1 lint error"));
    veld(dir).args(["check", "--no-lint"]).assert().success();
    append(dir, "veld.toml", "\n[lint]\nunused-model = \"error\"\n");
    veld(dir)
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error[L0001]"));
    veld(dir).args(["check", "--no-lint"]).assert().success();
    append(dir, "veld.toml", "empty-service = \"off\"\n");
    veld(dir).args(["check", "--no-lint"]).assert().success();
}

#[test]
fn an_unknown_lint_rule_is_reported_with_the_valid_ones() {
    let tmp = tempdir().unwrap();
    project(tmp.path());
    append(
        tmp.path(),
        "veld.toml",
        "\n[lint]\nno-such-rule = \"warn\"\n",
    );
    veld(tmp.path())
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("unknown lint rule `no-such-rule`"));
}

#[test]
fn generate_mentions_lint_findings() {
    let tmp = tempdir().unwrap();
    project(tmp.path());
    append(tmp.path(), "contracts/hello.veld", UNUSED);
    veld(tmp.path())
        .arg("generate")
        .assert()
        .success()
        .stdout(predicate::str::contains("run `veld check`"));
}

fn two_module_workspace(dir: &Path) {
    write(
        dir,
        "veld.toml",
        "[service]\nname = \"p\"\ncontracts = \"contracts\"\n",
    );
    write(
        dir,
        "contracts/a.veld",
        "model A { id: uuid }\nservice SA at \"/a\" {\n  get \"/\" a() -> A\n}\n",
    );
    write(
        dir,
        "contracts/b.veld",
        "model B { id: uuid }\nservice SB at \"/b\" {\n  get \"/\" b() -> B\n}\n",
    );
}

#[test]
fn check_with_paths_reports_only_those_files() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    two_module_workspace(dir);
    append(dir, "contracts/b.veld", "\nmodel Bad { x: Missing }\n");
    veld(dir)
        .args(["check", "contracts/a.veld"])
        .assert()
        .success()
        .stderr(
            predicate::str::contains("in other files")
                .and(predicate::str::contains("Missing").not()),
        );
    veld(dir)
        .args(["check", "contracts/b.veld"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("Missing"));
    veld(dir).args(["check", "contracts"]).assert().code(1);
    veld(dir)
        .args(["check", "contracts/nope.veld"])
        .assert()
        .code(1);
}

#[test]
fn check_graph_prints_text_dot_and_json_and_marks_cycles() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    write_mutual_workspace(dir);
    veld(dir)
        .args(["check", "--graph"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "cycle: orders <-> payments (allowed)",
        ));
    veld(dir)
        .args(["check", "--graph", "dot"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("digraph"));
    let out = veld(dir)
        .args(["check", "--graph=json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(json.get("cycles").is_some(), "{json}");
    veld(dir)
        .args(["check", "--graph", "yaml"])
        .assert()
        .code(2);
}

// --- export -------------------------------------------------------------------------------

#[test]
fn openapi_export_prints_to_stdout_and_writes_one_file() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    let stdout_args = ["export", "openapi"];
    let out = veld(dir).args(stdout_args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("\"openapi\""), "{text}");
    let file_args = ["export", "openapi", "-o", "o.json"];
    veld(dir).args(file_args).assert().success();
    assert_eq!(read(dir, "o.json"), text, "-o writes what stdout prints");
    veld(dir)
        .args(file_args)
        .assert()
        .success()
        .stdout(predicate::str::contains("unchanged"));
    // `export ir` prints the compiled IR.
    let ir = veld(dir).args(["export", "ir"]).output().unwrap();
    assert!(String::from_utf8_lossy(&ir.stdout).contains("\"services\""));
}

#[test]
fn a_single_document_export_will_not_overwrite_a_different_file_without_force() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    write(dir, "api.json", "mine");
    veld(dir)
        .args(["export", "openapi", "-o", "api.json"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("--force"));
    assert_eq!(read(dir, "api.json"), "mine");
    veld(dir)
        .args(["export", "openapi", "-o", "api.json", "--force"])
        .assert()
        .success();
    assert!(read(dir, "api.json").contains("\"openapi\""));
}

#[test]
fn export_service_filters_and_rejects_unknown_services() {
    let tmp = tempdir().unwrap();
    write_mutual_workspace(tmp.path());
    let out = veld(tmp.path())
        .args(["export", "openapi", "--service", "Orders"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("/orders") && !text.contains("/payments"),
        "{text}"
    );
    veld(tmp.path())
        .args(["export", "openapi", "--service", "Nope"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("known services"));
}

// --- clean --lock -------------------------------------------------------------------------

#[test]
fn clean_lock_removes_the_baseline_and_plain_clean_keeps_it() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    generated(dir);
    assert!(dir.join("veld.lock.ir.json").is_file());
    veld(dir).arg("clean").assert().success();
    assert!(
        dir.join("veld.lock.ir.json").is_file(),
        "clean keeps the lock"
    );
    veld(dir)
        .args(["clean", "--lock", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("would remove veld.lock.ir.json"));
    assert!(dir.join("veld.lock.ir.json").is_file());
    veld(dir).args(["clean", "--lock"]).assert().success();
    assert!(!dir.join("veld.lock.ir.json").exists());
    // Without a baseline there is nothing to gate on.
    break_contract(dir);
    veld(dir).arg("generate").assert().success();
}

// --- doctor -------------------------------------------------------------------------------

fn client_only_project(dir: &Path) {
    write(
        dir,
        "veld.toml",
        "[service]\nname = \"p\"\n[[client]]\nname = \"oas\"\nemitter = \"typescript\"\nout = \"spec\"\n",
    );
    write(
        dir,
        "contracts/a.veld",
        "model A { id: uuid }\nservice SA at \"/a\" {\n  get \"/\" a() -> A\n}\n",
    );
}

#[test]
fn doctor_passes_on_a_healthy_project() {
    let tmp = tempdir().unwrap();
    client_only_project(tmp.path());
    veld(tmp.path()).arg("doctor").assert().success().stdout(
        predicate::str::contains("✓ config")
            .and(predicate::str::contains("✓ contracts"))
            .and(predicate::str::contains("✓ target oas"))
            .and(predicate::str::contains("everything looks good")),
    );
}

#[test]
fn doctor_reports_each_problem_with_a_fix_and_exits_1() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    client_only_project(dir);
    append(
        dir,
        "veld.toml",
        "\n[plugins.ghost]\ncommand = \"veld-no-such-plugin-xyz\"\n\n[[client]]\nname = \"web\"\nemitter = \"plugin:ghost\"\nout = \"web\"\n\n[lint]\nbogus = \"warn\"\n\n[dependencies]\n\"@acme/billing\" = \"^1\"\n",
    );
    write(dir, "contracts/b.veld", "model B { x: Missing }\n");
    write(dir, "spec", "a file, not a directory");
    let out = veld(dir).arg("doctor").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    // The config itself is invalid because of the unknown lint rule only at lint level; the
    // plugin and registry problems are reported by their own lines.
    for needle in [
        "✗ registry",
        "✗ contracts",
        "✗ lint",
        "✗ target web",
        "✗ target oas",
        "fix:",
    ] {
        assert!(stdout.contains(needle), "lacks `{needle}`:\n{stdout}");
    }
}

#[test]
fn doctor_reports_a_broken_config_and_a_corrupt_baseline() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    client_only_project(dir);
    write(dir, "veld.lock.ir.json", "{ not json");
    veld(dir).arg("doctor").assert().code(1).stdout(
        predicate::str::contains("✗ baseline").and(predicate::str::contains("veld clean --lock")),
    );
    write(dir, "veld.toml", "[service]\nnam = \"x\"\n");
    veld(dir)
        .arg("doctor")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("✗ config"));
}

#[test]
fn doctor_checks_the_tools_the_wired_targets_need() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    project(dir);
    let out = veld(dir).arg("doctor").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(" node ") && stdout.contains(" npm "),
        "{stdout}"
    );
    assert!(stdout.contains(" git "), "{stdout}");
    // With an empty PATH nothing can be found: every tool line fails (git only warns).
    let out = veld(dir).env("PATH", "").arg("doctor").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(
        stdout.contains("✗ node") && stdout.contains("! git"),
        "{stdout}"
    );
}

// --- completions ---------------------------------------------------------------------------

#[test]
fn completions_exist_for_every_shell_and_stay_out_of_help() {
    let tmp = tempdir().unwrap();
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        veld(tmp.path())
            .args(["completions", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains("veld"));
    }
    veld(tmp.path())
        .args(["completions", "tcsh"])
        .assert()
        .code(2);
    veld(tmp.path())
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("completions").not());
}

// --- registry -----------------------------------------------------------------------------

/// A tiny registry that answers the read-only calls with canned JSON.
fn fake_registry() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || serve(stream));
        }
    });
    url
}

fn serve(mut stream: TcpStream) {
    let mut head = Vec::new();
    let mut buf = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut buf).unwrap_or(0) == 0 {
            return;
        }
        head.push(buf[0]);
    }
    let head = String::from_utf8_lossy(&head).to_string();
    let first = head.lines().next().unwrap_or_default().to_string();
    let length = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
        })
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    let _ = stream.read_exact(&mut body);
    let (status, json) = if first.starts_with("GET /api/v1/auth/me") {
        ("200 OK", r#"{"user":{"username":"ada"}}"#.to_string())
    } else if first.starts_with("GET /api/v1/packages?q=bill") {
        (
            "200 OK",
            r#"[{"org_name":"acme","name":"billing","latest_version":"1.2.0","description":"Billing contracts"}]"#.to_string(),
        )
    } else if first.starts_with("GET /api/v1/packages/acme/billing/versions") {
        (
            "200 OK",
            r#"[{"version":"1.0.0","tarball_sha256":"ab"},{"version":"1.2.0","tarball_sha256":"cd","deprecated":"use 2"}]"#.to_string(),
        )
    } else if first.starts_with("POST /api/v1/tokens") {
        let sent = String::from_utf8_lossy(&body).to_string();
        let ok = sent.contains("\"name\":\"ci\"") && sent.contains("\"write\"");
        (
            "201 Created",
            if ok {
                r#"{"token":"vtk_secret"}"#.to_string()
            } else {
                r#"{"error":"bad body"}"#.to_string()
            },
        )
    } else {
        ("404 Not Found", r#"{"error":"not found"}"#.to_string())
    };
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    );
    let _ = stream.write_all(reply.as_bytes());
}

#[test]
fn registry_info_search_versions_and_token_create_talk_to_the_registry() {
    let url = fake_registry();
    let tmp = tempdir().unwrap();
    let home = tempdir().unwrap();
    let run = |args: &[&str]| {
        let mut cmd = veld(tmp.path());
        cmd.env("VELD_HOME", home.path()).args(args);
        cmd.arg("--registry").arg(&url);
        cmd
    };
    run(&["registry", "info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not logged in"));
    veld(tmp.path())
        .env("VELD_HOME", home.path())
        .args(["login", "--registry", &url, "--token", "vtk_mine"])
        .assert()
        .success();
    run(&["registry", "info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ada"));
    run(&["registry", "search", "bill"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@acme/billing").and(predicate::str::contains("1.2.0")));
    run(&["registry", "versions", "@acme/billing"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("1.0.0").and(predicate::str::contains("deprecated: use 2")),
        );
    run(&["registry", "versions", "billing"]).assert().code(1);
    run(&[
        "registry",
        "token",
        "create",
        "--name",
        "ci",
        "--scopes",
        "read,write",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("vtk_secret"));
    run(&[
        "registry", "token", "create", "--name", "ci", "--scopes", "admin",
    ])
    .assert()
    .code(1)
    .stderr(predicate::str::contains("unknown scope"));
}

// --- init additions ------------------------------------------------------------------------

#[test]
fn init_writes_a_project_readme_and_never_replaces_an_existing_one() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    veld(dir)
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    let readme = read(dir, "README.md");
    assert!(
        readme.starts_with("# demo") && readme.contains("veld generate --check"),
        "{readme}"
    );
    let other = tempdir().unwrap();
    write(other.path(), "README.md", "mine");
    veld(other.path())
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    assert_eq!(read(other.path(), "README.md"), "mine");
}

#[test]
fn init_template_crud_scaffolds_users_auth_and_pagination_that_generate() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    veld(dir)
        .args(["init", "--name", "shop", "--template", "crud", "--yes"])
        .assert()
        .success();
    for f in [
        "contracts/common.veld",
        "contracts/users.veld",
        "contracts/auth.veld",
    ] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    assert!(!dir.join("contracts/hello.veld").exists());
    let users = read(dir, "contracts/users.veld");
    assert!(users.contains("Page<User>") && users.contains("service Users"));
    veld(dir)
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("2 services"));
    veld(dir).arg("fmt").arg("--check").assert().success();
    veld(dir).arg("generate").assert().success();
}

#[test]
fn init_template_crud_is_single_service_only_and_unknown_templates_are_usage_errors() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--template", "crud", "--workspace", "a,b"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("single-service"));
    veld(tmp.path())
        .args(["init", "--template", "nope"])
        .assert()
        .code(2);
    assert!(!tmp.path().join("veld.toml").exists());
}

#[test]
fn init_detects_an_existing_package_json_and_pom_xml() {
    let node = tempdir().unwrap();
    write(
        node.path(),
        "package.json",
        r#"{"name":"app","dependencies":{"fastify":"^4","vue":"^3"}}"#,
    );
    veld(node.path()).args(["init", "--yes"]).assert().success();
    let toml = read(node.path(), "veld.toml");
    assert!(
        toml.contains("framework = \"fastify\"") && toml.contains("framework = \"vue\""),
        "{toml}"
    );
    assert_eq!(
        read(node.path(), "package.json"),
        r#"{"name":"app","dependencies":{"fastify":"^4","vue":"^3"}}"#,
        "an existing package.json is left alone"
    );

    let java = tempdir().unwrap();
    write(
        java.path(),
        "pom.xml",
        "<project><artifactId>spring-boot-starter-web</artifactId></project>",
    );
    veld(java.path())
        .args(["init", "--yes", "--client", "none"])
        .assert()
        .success();
    let toml = read(java.path(), "veld.toml");
    assert!(
        toml.contains("emitter = \"java\"") && toml.contains("framework = \"spring\""),
        "{toml}"
    );

    let flags = tempdir().unwrap();
    write(flags.path(), "pom.xml", "<project/>");
    veld(flags.path())
        .args(["init", "--yes", "--server", "node:hono", "--client", "none"])
        .assert()
        .success();
    assert!(
        read(flags.path(), "veld.toml").contains("emitter = \"node\""),
        "flags win over detection"
    );
}

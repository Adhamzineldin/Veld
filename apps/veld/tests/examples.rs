//! Every project under `examples/` must compile, and its committed output must be up to date.
//!
//! The example is copied to a temporary directory first, so the test never touches the checkout.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::*;
use tempfile::tempdir;

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// The example directories: every direct child of `examples/` that holds a `veld.toml`.
fn examples() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(examples_dir())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("veld.toml").is_file())
        .collect();
    dirs.sort();
    dirs
}

/// Every file of an example except installed dependencies and build output.
fn source_tree(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    tree_excluding(dir, &["node_modules", "dist", "target", ".veld"])
}

fn copy_tree(from: &Path, to: &Path) {
    for (rel, bytes) in source_tree(from) {
        let target = to.join(&rel);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
}

#[test]
fn the_expected_examples_exist() {
    let names: Vec<String> = examples()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    for want in [
        "bookings",
        "hello",
        "library-package",
        "microservices",
        "rust-catalog",
    ] {
        assert!(names.iter().any(|n| n == want), "missing example {want}");
    }
}

#[test]
fn every_example_checks() {
    for example in examples() {
        let work = tempdir().unwrap();
        copy_tree(&example, work.path());
        veld(work.path()).arg("check").assert().success();
    }
}

#[test]
fn committed_output_of_every_example_is_up_to_date() {
    for example in examples() {
        let work = tempdir().unwrap();
        copy_tree(&example, work.path());
        let before = tree(work.path());

        let out = veld(work.path())
            .args(["generate", "--dry-run"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let text = String::from_utf8(out).unwrap();
        let name = example.file_name().unwrap().to_string_lossy();
        assert!(text.contains("would create"), "{name}: no dry-run report");
        for line in text.lines().filter(|l| l.contains("would create")) {
            assert!(
                line.contains("would create 0, update 0, delete 0"),
                "{name}: committed output is stale, run `veld generate`: {line}"
            );
        }
        assert_same_tree(&before, &tree(work.path()), &format!("{name}: dry run"));
    }
}

#[test]
fn generating_twice_from_scratch_gives_the_committed_bytes() {
    for example in examples() {
        let work = tempdir().unwrap();
        copy_tree(&example, work.path());
        veld(work.path()).arg("clean").assert().success();
        veld(work.path()).arg("generate").assert().success();
        assert_same_tree(
            &source_tree(&example),
            &source_tree(work.path()),
            &format!("{}: regeneration", example.display()),
        );
    }
}

/// The `example` blocks of the bookings contract are documentation that runs: all of them pass offline.
#[test]
fn bookings_examples_pass_offline() {
    let work = tempdir().unwrap();
    copy_tree(&examples_dir().join("bookings"), work.path());
    let out = veld(work.path())
        .arg("test")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    // Every `example` block of the contract ran and passed (the count comes from the file, so a
    // new example needs no change here, and a block that silently did not run is noticed).
    let contract =
        fs::read_to_string(examples_dir().join("bookings/contracts/bookings.veld")).unwrap();
    let count = contract
        .lines()
        .filter(|l| l.starts_with("example \""))
        .count();
    assert!(
        count > 30,
        "the contract should state the state machine and policy examples too"
    );
    assert!(
        text.contains(&format!(" {count} passed")),
        "expected {count} passed: {text}"
    );
    assert!(!text.contains("FAIL") && !text.contains("skip"), "{text}");
}

/// The README's "Using it in your code" section quotes the hello example verbatim.
#[test]
fn readme_quotes_the_hello_example_exactly() {
    // Line endings are not part of the quote (a Windows checkout may convert them).
    let lf = |t: String| t.replace("\r\n", "\n");
    let readme = lf(fs::read_to_string(repo_root().join("README.md")).unwrap());
    for file in [
        "server/src/main.ts",
        "web/src/App.tsx",
        "web/src/Greeter.tsx",
    ] {
        let code = lf(fs::read_to_string(examples_dir().join("hello").join(file)).unwrap());
        assert!(
            readme.contains(code.trim_end()),
            "README.md no longer matches examples/hello/{file}; update its \"Using it in your code\" section"
        );
    }
}

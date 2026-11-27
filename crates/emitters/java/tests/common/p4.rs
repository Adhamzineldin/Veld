//! Helpers of the tests of platform phase P4 (`spec/targets/java.md` section 18): generate a
//! contract in a role with extra target options, compile one or two generated trees with a driver of
//! `tests/toolchain/java/platform_p4/`, run it and hand back its working directory.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

use super::live::{java_main, javac_ok, write_files};
use super::{options, repo_root, run};

/// The folder of the drivers.
pub const DRIVERS: &str = "tests/toolchain/java/platform_p4";

/// The options of `package` with the entries of `extra` added (an object).
pub fn options_with(package: &str, extra: &serde_json::Value) -> serde_json::Value {
    let mut value = options(package, Some("17"));
    if let (Some(map), Some(more)) = (value.as_object_mut(), extra.as_object()) {
        for (k, v) in more {
            map.insert(k.clone(), v.clone());
        }
    }
    value
}

/// The files of every service of `ir` in `role`, with extra target options. The name of the target
/// is the package with `-` for `.` (`gen.b` is `gen-b`) unless `extra` has a `target-name`.
pub fn generate(ir: &Ir, role: Role, package: &str, extra: &serde_json::Value) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], None, options_with(package, extra))
}

/// The path of a driver: a name in the folder of the drivers of phase P4, or a path below
/// `tests/toolchain/java` (`platform_p1/AuditRedaction`).
pub fn driver(name: &str) -> PathBuf {
    if name.contains('/') {
        repo_root()
            .join("tests/toolchain/java")
            .join(format!("{name}.java"))
    } else {
        repo_root().join(DRIVERS).join(format!("{name}.java"))
    }
}
/// Compiles `trees` (generated files, one per package root) with the driver `name` for every
/// release, runs `veldtest.<name>` and checks it printed `OK`. `copy` lists files of the repository
/// to copy into the working directory (`(repository path, name there)`). Returns the working
/// directory (a driver may write files there).
pub fn run_driver(
    trees: &[&FileSet],
    name: &str,
    copy: &[(&str, &str)],
    args: &[&str],
) -> tempfile::TempDir {
    run_driver_with(trees, name, copy, &[], args)
}

/// Like [`run_driver`], and `write` lists files to create in the working directory (`(name there,
/// contents)`).
pub fn run_driver_with(
    trees: &[&FileSet],
    name: &str,
    copy: &[(&str, &str)],
    write: &[(&str, &str)],
    args: &[&str],
) -> tempfile::TempDir {
    let work = tempfile::tempdir().unwrap();
    for (to, text) in write {
        std::fs::write(work.path().join(to), text).unwrap();
    }
    let mut sources = Vec::new();
    for tree in trees {
        sources.extend(write_files(work.path(), tree));
    }
    sources.push(driver(name));
    for (from, to) in copy {
        std::fs::copy(repo_root().join(from), work.path().join(to)).unwrap();
    }
    for release in super::releases() {
        javac_ok(work.path(), &sources, release);
    }
    let class = name.rsplit('/').next().unwrap_or(name);
    let out = java_main(work.path(), &format!("veldtest.{class}"), args);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "{name} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    work
}

/// Compiles `trees` for every release without a driver.
pub fn compiles(trees: &[&FileSet]) {
    let work = tempfile::tempdir().unwrap();
    let mut sources = Vec::new();
    for tree in trees {
        sources.extend(write_files(work.path(), tree));
    }
    for release in super::releases() {
        javac_ok(work.path(), &sources, release);
    }
}

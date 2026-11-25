//! Shared by the tests of state machines and policies: compile a contract by path, build the
//! generated server and client with `tsc --strict`, serve the generated test handler and send it
//! every example of a contract through the shared live runner.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_project::output::{WriteOptions, write};

use super::{TARGETS, gen_client_with, gen_server_with, repo_root, typescript_option};

/// Compiles the contract at `path` (relative to the repository root) as the package `bookings`.
pub fn compile_path(path: &str) -> Ir {
    let text = std::fs::read_to_string(repo_root().join(path)).unwrap();
    compile_text(text)
}

/// Compiles `text` as the package `bookings`.
pub fn compile_text(text: String) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "bookings.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "the contract failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

pub fn toolchain_dir() -> PathBuf {
    repo_root().join("tests/toolchain/ts")
}

/// The `tsc` of the toolchain directory, or `None` (the test is skipped) without `node` or
/// `node_modules`, unless `VELD_REQUIRE_TOOLCHAINS=1`.
pub fn tsc_path() -> Option<PathBuf> {
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    let tsc = toolchain_dir().join("node_modules/typescript/bin/tsc");
    if node_ok && tsc.exists() {
        return Some(tsc);
    }
    assert!(
        std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
        "toolchain required but unavailable"
    );
    eprintln!("SKIPPED toolchain test");
    None
}

pub fn run(cmd: &mut Command, what: &str) {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{what} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write_config(work: &Path, name: &str, value: &serde_json::Value) {
    std::fs::write(
        work.join(name),
        serde_json::to_string_pretty(value).unwrap(),
    )
    .unwrap();
}

/// A scratch directory under `tests/toolchain/ts/.work`, removed when dropped.
pub fn scratch(prefix: &str) -> tempfile::TempDir {
    std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(toolchain_dir().join(".work"))
        .unwrap()
}

/// Generates the server (with the test handler) and the client of `ir` into `work`, copies the
/// `drivers`, and compiles all of it: the generated packages alone under `--strict` with no types
/// packages, then the drivers with the Node types.
pub fn build(work: &Path, tsc: &Path, ir: &Ir, drivers: &[&str], edition: &str) {
    let target = TARGETS
        .iter()
        .find(|(e, _)| *e == edition)
        .map_or("ES2020", |(_, t)| *t);
    let opts = WriteOptions::default();
    let option = json!({ "typescript": edition, "testing": true });
    write(
        &work.join("server"),
        &gen_server_with(ir, "Bookings", &[], &option),
        &opts,
    )
    .unwrap();
    write(
        &work.join("client"),
        &gen_client_with(ir, &typescript_option(edition)),
        &opts,
    )
    .unwrap();
    for driver in drivers {
        std::fs::copy(toolchain_dir().join(driver), work.join(driver)).unwrap();
    }
    write_config(
        work,
        "tsconfig.generated.json",
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target], "types": [],
                "module": "commonjs", "moduleResolution": "node10", "noEmit": true,
                "skipLibCheck": false, "forceConsistentCasingInFileNames": true
            },
            "include": ["server/**/*.ts", "client/**/*.ts"]
        }),
    );
    run(
        Command::new("node")
            .arg(tsc)
            .args(["-p", "tsconfig.generated.json"])
            .current_dir(work),
        "tsc generated",
    );
    write_config(
        work,
        "tsconfig.json",
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target, "dom"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "server/**/*.ts", "client/**/*.ts"]
        }),
    );
    run(
        Command::new("node")
            .arg(tsc)
            .args(["-p", "tsconfig.json"])
            .current_dir(work),
        "tsc drivers",
    );
}

/// A running driver that prints `PORT=<n>` and serves until its stdin closes; killed when dropped.
pub struct Live {
    tree: veld_test_support::ProcessTree,
    pub url: String,
}

impl Live {
    /// Starts `node out/<script>` in `work`.
    pub fn start(work: &Path, script: &str) -> Live {
        let mut cmd = Command::new("node");
        cmd.arg(format!("out/{script}"))
            .current_dir(work)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut tree =
            veld_test_support::ProcessTree::spawn(&mut cmd).expect("cannot start the server");
        let mut line = String::new();
        BufReader::new(tree.child_mut().stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port = line
            .trim()
            .strip_prefix("PORT=")
            .unwrap_or_else(|| panic!("the server did not report a port: {line:?}"));
        Live {
            url: format!("http://127.0.0.1:{port}"),
            tree,
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.tree.kill_tree();
    }
}

/// Sends every example of `ir` to the test handler at `url`, in dry-run mode, through the shared
/// live runner. Returns how many passed.
pub fn run_examples(url: &str, ir: &Ir) -> usize {
    let live = veld_testing::Live::connect(url, true).unwrap();
    let (mut passed, mut failures) = (0, Vec::new());
    for ex in &ir.examples {
        match live.run_example(ir, ex) {
            Ok(report) if report.passed() => passed += 1,
            Ok(report) => failures.push(format!("\"{}\": {report}", ex.name)),
            Err(e) => failures.push(format!("\"{}\": {e}", ex.name)),
        }
    }
    assert!(
        failures.is_empty(),
        "{} examples failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    passed
}

//! `veld plugin new` end to end: scaffold, compile with the pinned tsc, conformance-test it,
//! then use it as a `plugin:` target of a real project.
//!
//! Without node and the TypeScript harness the test is skipped with a message, unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::PathBuf;
use std::process::Command;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn harness() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/toolchain/ts/node_modules")
}

fn toolchain_ready() -> bool {
    let node = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    let ok = node
        && harness().join("typescript/bin/tsc").is_file()
        && harness().join("@types/node").is_dir();
    if !ok {
        assert!(
            std::env::var("VELD_REQUIRE_TOOLCHAINS").is_err(),
            "VELD_REQUIRE_TOOLCHAINS is set but node or the TypeScript harness is missing"
        );
    }
    ok
}

fn node_output(args: &[&str]) -> String {
    let out = Command::new("node").args(args).output().unwrap();
    assert!(
        out.status.success(),
        "node {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn sha256_of_node() -> String {
    let script = "const c=require('crypto'),f=require('fs');console.log(c.createHash('sha256').update(f.readFileSync(process.execPath)).digest('hex'))";
    node_output(&["-e", script])
}

fn compile(dir: &std::path::Path) {
    let harness = harness().canonicalize().unwrap();
    let clean = |p: PathBuf| p.to_string_lossy().trim_start_matches(r"\?\").to_string();
    let tsc = clean(harness.join("typescript/bin/tsc"));
    let types = clean(harness.join("@types"));
    let out = Command::new("node")
        .args([&tsc, "-p", "demo", "--typeRoots", &types, "--types", "node"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "tsc failed:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn scaffolded_plugin_builds_passes_conformance_and_generates() {
    if !toolchain_ready() {
        eprintln!("skipped: node or the TypeScript harness is not available");
        return;
    }
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    veld(dir).args(["plugin", "new", "demo"]).assert().success();
    let files = tree(&dir.join("demo"));
    for f in [
        "package.json",
        "tsconfig.json",
        "README.md",
        "src/protocol.ts",
        "src/generate.ts",
        "src/main.ts",
        "src/ir.d.ts",
    ] {
        assert!(files.contains_key(f), "missing {f}: {:?}", files.keys());
    }
    assert_eq!(
        String::from_utf8_lossy(&files["src/ir.d.ts"]),
        veld_ir::schema::typescript()
    );
    veld(dir)
        .args(["plugin", "new", "demo"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("already exists"));
    veld(dir)
        .args(["plugin", "new", "Bad Name"])
        .assert()
        .code(1);

    compile(dir);
    assert!(dir.join("demo/dist/main.js").is_file());
    veld(dir)
        .args(["plugin", "test", "node", "demo/dist/main.js"])
        .assert()
        .success()
        .stdout(predicate::str::contains("handshake"));
    veld(dir)
        .args(["plugin", "test", "node", "demo/no-such.js"])
        .assert()
        .code(1);

    write(
        dir,
        "contracts/hello.veld",
        "model Greeting {\n  message: string\n}\n\nservice Hello at \"/hello\" {\n  get \"/{name}\" greet(name: string) -> Greeting | NotFound\n}\n",
    );
    write(
        dir,
        "veld.toml",
        &format!(
            "[project]\nname = \"hello\"\n\n[plugins.demo]\ncommand = \"node demo/dist/main.js\"\nsha256 = \"{}\"\n\n\
             [[targets]]\nname = \"demo\"\nrole = \"client\"\nemitter = \"plugin:demo\"\nout = \"gen/demo\"\n",
            sha256_of_node()
        ),
    );
    veld(dir).arg("generate").assert().success();
    let out = tree(&dir.join("gen/demo"));
    let types = out
        .iter()
        .find(|(p, _)| p.starts_with("types/") && p.contains("Greeting"))
        .expect("a type file");
    assert!(
        String::from_utf8_lossy(types.1).contains("sig: "),
        "signature embedded"
    );
    assert!(out.keys().any(|p| p.starts_with("services/")));
}

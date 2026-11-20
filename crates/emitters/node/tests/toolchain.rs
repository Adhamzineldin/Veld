//! Behaviour tests against a real TypeScript toolchain (`tests/toolchain/ts`).
//!
//! Generates the contract, compiles it with `tsc --strict`, then runs the round trip,
//! cross-package equality and wire-golden scripts under Node.
//!
//! Without `node` or `node_modules` the test is skipped with a message, unless
//! `VELD_REQUIRE_TOOLCHAINS=1`, in which case it fails.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use serde_json::{Map, Value, json};
use veld_ir::Ir;
use veld_project::output::{WriteOptions, write};

fn toolchain_dir() -> PathBuf {
    repo_root().join("tests/toolchain/ts")
}

/// The `tsc` entry point, or why the toolchain is unavailable.
fn locate_toolchain() -> Result<PathBuf, String> {
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !node_ok {
        return Err("`node` is not on PATH".into());
    }
    let tsc = toolchain_dir().join("node_modules/typescript/bin/tsc");
    if !tsc.exists() {
        return Err(format!(
            "{} is missing; run `npm ci` in tests/toolchain/ts",
            tsc.display()
        ));
    }
    Ok(tsc)
}

fn sig_map(ir: &Ir) -> Value {
    let mut m = Map::new();
    for s in &ir.services {
        for ep in &s.endpoints {
            m.insert(format!("{}.{}", s.name, ep.name), json!(ep.sig));
        }
    }
    for d in &ir.decls {
        m.insert(format!("decl:{}", d.qualified), json!(d.sig));
    }
    Value::Object(m)
}

fn run(cmd: &mut Command, what: &str) {
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{what}: cannot start: {e}"));
    print!("{}", String::from_utf8_lossy(&out.stdout));
    assert!(
        out.status.success(),
        "{what} failed ({})\nstdout:\n{}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn tsc(tsc: &Path, work: &Path, project: &str) {
    run(
        Command::new("node")
            .arg(tsc)
            .args(["-p", project])
            .current_dir(work),
        &format!("tsc -p {project}"),
    );
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

fn generated_only_config(dirs: &[&str], module: &str) -> Value {
    let (module_resolution, module) = if module == "nodenext" {
        ("nodenext", "nodenext")
    } else {
        ("node10", "commonjs")
    };
    json!({
        "compilerOptions": {
            "strict": true, "target": "ES2022", "lib": ["ES2022"], "types": [],
            "module": module, "moduleResolution": module_resolution,
            "noEmit": true, "skipLibCheck": false,
            "noUnusedLocals": false, "forceConsistentCasingInFileNames": true
        },
        "include": dirs.iter().map(|d| format!("{d}/**/*.ts")).collect::<Vec<_>>()
    })
}

#[test]
fn generated_typescript_compiles_and_behaves() {
    let tsc_path = match locate_toolchain() {
        Ok(p) => p,
        Err(why) => {
            assert!(
                std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
                "toolchain required but unavailable: {why}"
            );
            eprintln!("SKIPPED toolchain test: {why}");
            return;
        }
    };

    let a = standard_ir();
    let c = compile_contract("orders", &|s| {
        s.replace(
            "error OutOfStock 409 {\n  sku: string\n}",
            "error OutOfStock 409 {\n  sku: string\n  warehouse: string\n}",
        )
    });
    let d = compile_contract("warehouse", &|s| s.to_string());

    std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
    let work_guard = tempfile::Builder::new()
        .prefix("run-")
        .tempdir_in(toolchain_dir().join(".work"))
        .unwrap();
    let work = work_guard.path();
    let opts = WriteOptions::default();
    let put = |dir: &str, files: &veld_codegen::FileSet| {
        write(&work.join(dir), files, &opts).unwrap();
    };
    put("server", &gen_server(&a, "Orders", &["Iam"]));
    put("server_iam", &gen_server(&a, "Iam", &[]));
    put("copy_a", &gen_client(&a));
    put("copy_b", &gen_client(&a));
    put("copy_c", &gen_client(&c));
    put("copy_d", &gen_client(&d));

    write_json(
        &work.join("sigs.json"),
        &json!({ "a": sig_map(&a), "c": sig_map(&c), "d": sig_map(&d) }),
    );
    for script in ["roundtrip.ts", "equality.ts", "wire.ts"] {
        std::fs::copy(toolchain_dir().join(script), work.join(script)).unwrap();
    }

    // 1. Each generated target compiles on its own: strict, no types packages, both module modes.
    let dirs = [
        "server",
        "server_iam",
        "copy_a",
        "copy_b",
        "copy_c",
        "copy_d",
    ];
    write_json(
        &work.join("tsconfig.commonjs.json"),
        &generated_only_config(&dirs, "commonjs"),
    );
    write_json(
        &work.join("tsconfig.nodenext.json"),
        &generated_only_config(&dirs, "nodenext"),
    );
    tsc(&tsc_path, work, "tsconfig.commonjs.json");
    tsc(&tsc_path, work, "tsconfig.nodenext.json");

    // 2. Behaviour scripts compile (with Node and Express types) and run.
    write_json(
        &work.join("tsconfig.json"),
        &json!({
            "compilerOptions": {
                "strict": true, "target": "ES2022", "lib": ["ES2022"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "server/**/*.ts", "server_iam/**/*.ts", "copy_a/**/*.ts", "copy_b/**/*.ts", "copy_c/**/*.ts", "copy_d/**/*.ts"]
        }),
    );
    tsc(&tsc_path, work, "tsconfig.json");
    for script in ["roundtrip", "equality", "wire"] {
        let mut cmd = Command::new("node");
        cmd.arg(format!("out/{script}.js"))
            .current_dir(work)
            .env("WIRE_DIR", repo_root().join("tests/wire"));
        run(&mut cmd, &format!("node {script}.js"));
    }
}

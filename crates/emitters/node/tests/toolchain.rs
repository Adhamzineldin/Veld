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

/// Runs `body` once per language version of the matrix (`typescript = "es2020"|"es2022"` with the
/// matching `tsc --target`), or skips when the toolchain is missing.
fn matrix(body: fn(&Path, &str, &str)) {
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
    for (edition, target) in TARGETS {
        body(&tsc_path, edition, target);
    }
}

fn generated_only_config(dirs: &[&str], module: &str, target: &str) -> Value {
    let (module_resolution, module) = if module == "nodenext" {
        ("nodenext", "nodenext")
    } else {
        ("node10", "commonjs")
    };
    json!({
        "compilerOptions": {
            "strict": true, "target": target, "lib": [target], "types": [],
            "module": module, "moduleResolution": module_resolution,
            "noEmit": true, "skipLibCheck": false,
            "noUnusedLocals": false, "forceConsistentCasingInFileNames": true
        },
        "include": dirs.iter().map(|d| format!("{d}/**/*.ts")).collect::<Vec<_>>()
    })
}

fn generated_typescript_compiles_and_behaves(tsc_path: &Path, edition: &str, target: &str) {
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
    put(
        "server",
        &gen_server_with(&a, "Orders", &["Iam"], &typescript_option(edition)),
    );
    put(
        "server_iam",
        &gen_server_with(&a, "Iam", &[], &typescript_option(edition)),
    );
    put("copy_a", &gen_client_with(&a, &typescript_option(edition)));
    put("copy_b", &gen_client_with(&a, &typescript_option(edition)));
    put("copy_c", &gen_client_with(&c, &typescript_option(edition)));
    put("copy_d", &gen_client_with(&d, &typescript_option(edition)));

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
        &generated_only_config(&dirs, "commonjs", target),
    );
    write_json(
        &work.join("tsconfig.nodenext.json"),
        &generated_only_config(&dirs, "nodenext", target),
    );
    tsc(tsc_path, work, "tsconfig.commonjs.json");
    tsc(tsc_path, work, "tsconfig.nodenext.json");

    // 2. Behaviour scripts compile (with Node and Express types) and run.
    write_json(
        &work.join("tsconfig.json"),
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "server/**/*.ts", "server_iam/**/*.ts", "copy_a/**/*.ts", "copy_b/**/*.ts", "copy_c/**/*.ts", "copy_d/**/*.ts"]
        }),
    );
    tsc(tsc_path, work, "tsconfig.json");
    for script in ["roundtrip", "equality", "wire"] {
        let mut cmd = Command::new("node");
        cmd.arg(format!("out/{script}.js"))
            .current_dir(work)
            .env("WIRE_DIR", repo_root().join("tests/wire"));
        run(&mut cmd, &format!("node {script}.js"));
    }
}

#[test]
fn generated_typescript_compiles_and_behaves_per_version() {
    matrix(generated_typescript_compiles_and_behaves);
}

/// Endpoints, fields, models and enum variants named with every hazardous identifier must
/// still compile under `tsc --strict`, on the server and the client.
fn hazardous_identifiers_compile_strict(tsc_path: &Path, edition: &str, target: &str) {
    let ir = hazards_ir();
    std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
    let work_guard = tempfile::Builder::new()
        .prefix("hazards-")
        .tempdir_in(toolchain_dir().join(".work"))
        .unwrap();
    let work = work_guard.path();
    let opts = WriteOptions::default();
    write(
        &work.join("server"),
        &gen_server_with(&ir, "Weird", &[], &typescript_option(edition)),
        &opts,
    )
    .unwrap();
    write(
        &work.join("client"),
        &gen_client_with(&ir, &typescript_option(edition)),
        &opts,
    )
    .unwrap();
    for module in ["commonjs", "nodenext"] {
        write_json(
            &work.join(format!("tsconfig.{module}.json")),
            &generated_only_config(&["server", "client"], module, target),
        );
        tsc(tsc_path, work, &format!("tsconfig.{module}.json"));
    }
}

#[test]
fn hazardous_identifiers_compile_strict_per_version() {
    matrix(hazardous_identifiers_compile_strict);
}

/// Errors raised by two separately generated servers over real HTTP and decoded by clients from
/// two separately generated packages: `same()` holds for the same service, op and payload, and
/// fails for a different payload and for the other service's otherwise identical error.
fn equality_holds_over_http_across_packages(tsc_path: &Path, edition: &str, target: &str) {
    let ir = equality_ir();
    std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
    let work_guard = tempfile::Builder::new()
        .prefix("http-eq-")
        .tempdir_in(toolchain_dir().join(".work"))
        .unwrap();
    let work = work_guard.path();
    let opts = WriteOptions::default();
    write(
        &work.join("srv_alpha"),
        &gen_server_with(&ir, "Alpha", &[], &typescript_option(edition)),
        &opts,
    )
    .unwrap();
    write(
        &work.join("srv_beta"),
        &gen_server_with(&ir, "Beta", &[], &typescript_option(edition)),
        &opts,
    )
    .unwrap();
    write(
        &work.join("cli_1"),
        &gen_client_with(&ir, &typescript_option(edition)),
        &opts,
    )
    .unwrap();
    write(
        &work.join("cli_2"),
        &gen_client_with(&ir, &typescript_option(edition)),
        &opts,
    )
    .unwrap();
    std::fs::copy(
        toolchain_dir().join("http_equality.ts"),
        work.join("http_equality.ts"),
    )
    .unwrap();
    write_json(
        &work.join("tsconfig.json"),
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "srv_alpha/**/*.ts", "srv_beta/**/*.ts", "cli_1/**/*.ts", "cli_2/**/*.ts"]
        }),
    );
    tsc(tsc_path, work, "tsconfig.json");
    let mut cmd = Command::new("node");
    cmd.arg("out/http_equality.js").current_dir(work);
    run(&mut cmd, "node http_equality.js");
}

#[test]
fn equality_holds_over_http_across_packages_per_version() {
    matrix(equality_holds_over_http_across_packages);
}

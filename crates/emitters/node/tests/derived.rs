//! Derived fields in the generated TypeScript (`spec/targets/typescript.md` section 5i,
//! `spec/rules.md` section 17.2).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;
use veld_ir::Ir;

const ENDPOINTS: &str = "tests/rules/endpoints/derived.veld.pending";

fn shop_ir() -> Ir {
    compile_path(ENDPOINTS)
}

fn testing() -> serde_json::Value {
    json!({ "testing": true })
}

fn examples_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = shop_ir();
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Shop", &["derived_serve.ts"], edition);
    let server = Live::start(work, "derived_serve.js");
    let passed = run_examples(&server.url, &ir);
    assert_eq!(passed, 9);
    assert_eq!(ir.examples.len(), 9, "every example ran, none was skipped");
}

#[test]
fn the_derived_examples_compile_strict_and_pass_es2020() {
    examples_run("es2020", "derived-2020-");
}

#[test]
fn the_derived_examples_compile_strict_and_pass_es2022() {
    examples_run("es2022", "derived-2022-");
}

#[allow(dead_code)]
fn unused() {
    let _ = (gen_server_with(&shop_ir(), "Shop", &[], &testing()),);
}

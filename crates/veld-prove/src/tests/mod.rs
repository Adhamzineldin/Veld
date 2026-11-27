//! Tests of the encoder, the solver glue and the confirmation step.
//!
//! The tests that need a real solver look for one the way the CLI does (`VELD_SOLVER`, then
//! `z3` and `cvc5` on `PATH`). Without one they say so and pass, unless `VELD_REQUIRE_SOLVER=1`
//! is set, which makes them fail.

use std::collections::BTreeMap;

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::Ir;

use crate::solver::{Solver, find_in_environment};

/// Compiles `text` as the package `t`, as the vector contracts under `tests/rules/` are.
pub fn compile_text(text: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: veld_ir::PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.to_owned(),
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
    out.ir.unwrap()
}

/// A solver for a test, or `None` after saying why the test is skipped.
pub fn solver(test: &str) -> Option<Solver> {
    match find_in_environment(None) {
        Ok(s) => Some(s),
        Err(e) => {
            assert!(
                std::env::var("VELD_REQUIRE_SOLVER").as_deref() != Ok("1"),
                "{test}: VELD_REQUIRE_SOLVER=1 but {e}"
            );
            eprintln!("{test}: skipped, {e}");
            None
        }
    }
}
mod canned;
mod encoder;
mod pin;
mod questions;
mod soundness;
mod units;

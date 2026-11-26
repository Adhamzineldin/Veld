//! Veld tools: the canonical formatter, the OpenAPI exporter, lint, the dependency graph and
//! breaking-change detection.
//!
//! Every tool is a pure function over source text or the IR. None of them
//! touches the file system.

pub mod deps;
pub mod diff;
pub mod fmt;
pub mod gate;
pub mod lint;
pub mod openapi;
mod retention_export;

pub use deps::{DepGraph, DepsFormat, deps, deps_with_owners};
pub use diff::{
    Change, ChangeKind, ChangedRule, RuleSite, Severity, changed_rules, diff, has_breaking,
};
pub use fmt::format;
pub use gate::{GateDecision, GatePolicy, GateResult, breaking_gate};
pub use openapi::openapi;
pub use retention_export::retention_report;

#[cfg(test)]
mod retention_export_tests;

#[cfg(test)]
pub(crate) mod testutil {
    use std::collections::BTreeMap;

    use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
    use veld_ir::{Ir, PackageKind};

    /// Compiles `src` as the single module `app.veld` of package `app`.
    pub fn ir(src: &str) -> Ir {
        let out = compile(&CompileInput {
            packages: vec![PackageInput {
                name: "app".into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: "app.veld".into(),
                    text: src.into(),
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        });
        match out.ir {
            Some(ir) => ir,
            None => panic!("test source does not compile:\n{}", out.render()),
        }
    }

    /// A contract that uses every feature the exporters map.
    pub const SHOP: &str = include_str!("testdata/shop.veld");

    /// The IR of [`SHOP`].
    pub fn shop() -> Ir {
        ir(SHOP)
    }
}

//! Veld tools: the canonical formatter, the OpenAPI exporter, breaking-change
//! detection and the v1 to v2 migration.
//!
//! Every tool is a pure function over source text or the IR. None of them
//! touches the file system.

pub mod diff;
pub mod fmt;
pub mod migrate;
pub mod openapi;

pub use diff::{Change, ChangeKind, Severity, diff, has_breaking};
pub use fmt::format;
pub use migrate::{MigrateResult, migrate_v1};
pub use openapi::openapi;

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
}

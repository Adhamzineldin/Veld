//! Shared helpers for unit tests.

use std::collections::BTreeMap;

use veld_compiler::index::{Index, index};
use veld_compiler::{CompileInput, PackageInput, SourceFile};
use veld_ir::PackageKind;

/// Indexes (package, path, text) triples; every package is a service.
pub fn build(files: &[(&str, &str, &str)]) -> Index {
    let mut by: BTreeMap<&str, Vec<SourceFile>> = BTreeMap::new();
    for (p, path, text) in files {
        by.entry(p).or_default().push(SourceFile {
            path: (*path).into(),
            text: (*text).into(),
        });
    }
    index(&CompileInput {
        packages: by
            .into_iter()
            .map(|(name, files)| PackageInput {
                name: name.into(),
                kind: PackageKind::Service,
                files,
                base_url: None,
            })
            .collect(),
        consumes: BTreeMap::new(),
    })
}

//! The embedded runtime. One real `.ts` file, byte-identical in every role.

use veld_codegen::{FileSet, GenDiagnostic};

use crate::util::add_file;

/// Source of `runtime/veld.ts`.
pub const SOURCE: &str = include_str!("../runtime/veld.ts");

/// Root-relative module path of the runtime, without extension.
pub const MODULE: &str = "runtime/veld";

/// Adds `runtime/veld.ts` to `files`.
pub fn emit(files: &mut FileSet) -> Result<(), Vec<GenDiagnostic>> {
    add_file(files, "runtime/veld.ts", SOURCE.to_string())
}

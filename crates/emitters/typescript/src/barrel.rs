//! `index.ts` barrels.

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};

use crate::util::{HEADER, add_file, rel_import};

/// Runtime names re-exported by every top-level barrel.
const RUNTIME_EXPORTS: &str = "export { VeldError, VeldHttpError, VeldTransportError, canonical, canonicalError, matchesError, same } from './runtime/veld';";

/// Writes `<dir>index.ts` re-exporting `modules` (root-relative, no extension) and any
/// `extra` lines. `top_level` adds the runtime exports.
pub fn write(
    files: &mut FileSet,
    dir: &str,
    modules: &[String],
    extra: &[String],
    top_level: bool,
) -> Result<(), Vec<GenDiagnostic>> {
    let from = format!("{dir}index.ts");
    let mut w = CodeWriter::new();
    w.line(HEADER);
    w.blank();
    if top_level {
        w.line("export * as veld from './runtime/veld';");
        w.line(RUNTIME_EXPORTS);
    }
    for m in modules {
        w.line(format!("export * from '{}';", rel_import(&from, m)));
    }
    for e in extra {
        w.line(e);
    }
    add_file(files, &from, w.finish())
}

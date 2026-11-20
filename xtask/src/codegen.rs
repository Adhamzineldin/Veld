//! `cargo xtask codegen [--check]`: files generated from the Rust sources.
//!
//! - `editors/veld-language.json`: the language table, for editor grammars
//! - `spec/ir.d.ts`: the TypeScript types of the IR and the plugin messages

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// `(path relative to the repository root, contents)` of every generated file.
pub fn outputs() -> Result<Vec<(&'static str, String)>> {
    let mut language =
        serde_json::to_string_pretty(&veld_compiler::language::spec()).context("cannot serialize the language table")?;
    language.push('\n');
    Ok(vec![("editors/veld-language.json", language), ("spec/ir.d.ts", veld_ir::schema::typescript())])
}

/// Writes the files, or with `check` only compares them. Returns the paths that differ (or were written).
pub fn run(root: &Path, check: bool) -> Result<Vec<String>> {
    let mut drift = Vec::new();
    for (rel, text) in outputs()? {
        let path = root.join(rel);
        let current = fs::read_to_string(&path).ok();
        if current.as_deref() == Some(text.as_str()) {
            continue;
        }
        drift.push(rel.to_string());
        if !check {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).with_context(|| format!("cannot create `{}`", dir.display()))?;
            }
            fs::write(&path, text).with_context(|| format!("cannot write `{}`", path.display()))?;
        }
    }
    if check && !drift.is_empty() {
        bail!("generated files are out of date: {}\nrun `cargo xtask codegen` and commit the result", drift.join(", "));
    }
    Ok(drift)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_check_passes_and_drift_is_detected() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(run(tmp.path(), true).is_err(), "nothing is generated yet");
        let written = run(tmp.path(), false).unwrap();
        assert_eq!(written.len(), 2);
        assert!(run(tmp.path(), true).unwrap().is_empty());
        fs::write(tmp.path().join("editors/veld-language.json"), "{}").unwrap();
        let err = run(tmp.path(), true).unwrap_err().to_string();
        assert!(err.contains("editors/veld-language.json"), "{err}");
    }

    /// The committed files must equal what the Rust sources generate. Fix with `cargo xtask codegen`.
    #[test]
    fn committed_generated_files_are_current() {
        run(&crate::new_emitter::repo_root(), true).unwrap();
    }

    #[test]
    fn language_json_lists_the_declaration_keywords() {
        let text = &outputs().unwrap()[0].1;
        assert!(text.contains("\"model\"") && text.contains("\"service\""), "{text}");
    }
}

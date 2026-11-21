//! `cargo xtask codegen [--check]`: files generated from the Rust sources.
//!
//! - `editors/veld-language.json`: the language table, for editor grammars
//! - `editors/vscode/syntaxes/veld.tmLanguage.json`: the TextMate grammar, from the table above
//! - `editors/jetbrains/src/main/resources/textmate/veld/syntaxes/veld.tmLanguage.json`: the same grammar,
//!   bundled into the JetBrains plugin
//! - `spec/diagnostics.md`: every diagnostic code, from `veld_compiler::codes`
//! - `spec/ir.d.ts`: the TypeScript types of the IR and the plugin messages

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// `(path relative to the repository root, contents)` of every generated file.
pub fn outputs() -> Result<Vec<(&'static str, String)>> {
    let mut language = serde_json::to_string_pretty(&veld_compiler::language::spec())
        .context("cannot serialize the language table")?;
    language.push('\n');
    let grammar = crate::grammar::tm_language(&language)?;
    Ok(vec![
        ("editors/veld-language.json", language),
        (
            "editors/vscode/syntaxes/veld.tmLanguage.json",
            grammar.clone(),
        ),
        (
            "editors/jetbrains/src/main/resources/textmate/veld/syntaxes/veld.tmLanguage.json",
            grammar,
        ),
        ("spec/diagnostics.md", veld_compiler::codes::markdown()),
        ("spec/ir.d.ts", veld_ir::schema::typescript()),
    ])
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
                fs::create_dir_all(dir)
                    .with_context(|| format!("cannot create `{}`", dir.display()))?;
            }
            fs::write(&path, text).with_context(|| format!("cannot write `{}`", path.display()))?;
        }
    }
    if check && !drift.is_empty() {
        bail!(
            "generated files are out of date: {}\nrun `cargo xtask codegen` and commit the result",
            drift.join(", ")
        );
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
        assert_eq!(written.len(), 5);
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
        assert!(
            text.contains("\"model\"") && text.contains("\"service\""),
            "{text}"
        );
    }
}

#[cfg(test)]
mod code_table_tests {
    use std::fs;
    use std::path::Path;

    fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// Every `E####` / `W####` token in `text`.
    fn codes_in(text: &str) -> Vec<String> {
        let b = text.as_bytes();
        let mut found = Vec::new();
        for i in 0..b.len().saturating_sub(4) {
            let boundary = i == 0 || !b[i - 1].is_ascii_alphanumeric();
            if boundary
                && matches!(b[i], b'E' | b'W')
                && b[i + 1..i + 5].iter().all(u8::is_ascii_digit)
                && b.get(i + 5).is_none_or(|c| !c.is_ascii_alphanumeric())
            {
                found.push(text[i..i + 5].to_string());
            }
        }
        found
    }

    /// A code used anywhere in the workspace sources must be in the code table, so it appears in
    /// `spec/diagnostics.md`.
    #[test]
    fn every_code_in_the_sources_is_documented() {
        let root = crate::new_emitter::repo_root();
        let mut files = Vec::new();
        for dir in ["crates", "apps", "xtask"] {
            rust_files(&root.join(dir), &mut files);
        }
        assert!(files.len() > 50, "found only {} sources", files.len());
        let mut missing = Vec::new();
        for file in files {
            if file.ends_with("veld-compiler/src/codes.rs") {
                continue;
            }
            let text = fs::read_to_string(&file).unwrap_or_default();
            for code in codes_in(&text) {
                if veld_compiler::codes::lookup(&code).is_none() {
                    missing.push(format!("{code} in {}", file.display()));
                }
            }
        }
        assert!(missing.is_empty(), "undocumented codes: {missing:#?}");
    }

    #[test]
    fn code_scanner_finds_codes() {
        assert_eq!(
            codes_in("a \"E0303\" b W0001: x E00012 XE0001"),
            ["E0303", "W0001"]
        );
    }
}

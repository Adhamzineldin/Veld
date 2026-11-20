//! v1 to v2 migration.
//!
//! `migrate_v1` reads a set of v1 contract files (one package) and returns the
//! equivalent v2 files plus a list of warnings. Nothing is dropped silently:
//! whatever cannot be mapped is named in a warning and, where it has a place
//! in the file, in a `// TODO(veld migrate): ...` comment.

mod conv;
mod ctx;
mod models;
mod names;
mod services;
pub mod v1;

use std::collections::{BTreeMap, BTreeSet};

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;

use conv::{FileConv, comment_block, todo_comment};
use ctx::{Ctx, Source};
use v1::ast::{Item, V1File};

/// The migrated package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateResult {
    /// New `.veld` paths with their formatted v2 source.
    pub files: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

/// Migrates the v1 files of one package. Paths are relative to the package
/// root; the module path of each output file follows from its path.
#[must_use]
pub fn migrate_v1(files: &[(String, String)]) -> MigrateResult {
    let mut warnings = Vec::new();
    let mut sorted: Vec<&(String, String)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let sources: Vec<Source> = sorted.iter().map(|(p, t)| parse_source(p, t, &mut warnings)).collect();
    let ctx = Ctx::collect(&sources);
    warnings.extend(ctx.warnings.iter().cloned());
    let mut out = Vec::new();
    for (idx, s) in sources.iter().enumerate() {
        if let Some(text) = convert_source(&ctx, &sources, idx, &mut warnings) {
            out.push((s.out_path.clone(), text));
        }
    }
    check_imports(&sources, &mut warnings);
    check_compiles(&out, &mut warnings);
    MigrateResult { files: out, warnings }
}

fn parse_source(rel: &str, text: &str, warnings: &mut Vec<String>) -> Source {
    let (_, _, renamed) = names::module_of(rel);
    if renamed {
        warnings.push(format!("{rel}: path segments are not valid identifiers; the module path was sanitized"));
    }
    let parsed = match v1::parse(text) {
        Ok(p) => {
            warnings.extend(p.warnings.into_iter().map(|w| format!("{rel}: {w}")));
            Ok(p.file)
        }
        Err(e) => {
            warnings.push(format!("{rel}: not migrated, v1 syntax error: {e}"));
            Err(e)
        }
    };
    Source::new(rel, text, parsed)
}

fn convert_source(ctx: &Ctx, sources: &[Source], idx: usize, warnings: &mut Vec<String>) -> Option<String> {
    let s = &sources[idx];
    let file = match &s.parsed {
        Ok(f) => f,
        Err(e) => return Some(unparsed(s, e)),
    };
    if file.items.is_empty() && file.trailing.is_empty() {
        warnings.push(format!("{}: only contains imports or settings; no v2 file is needed", s.rel));
        return None;
    }
    let mut c = FileConv::new(ctx, &s.rel);
    let mut parts = convert_items(&mut c, file);
    c.generated_errors(idx);
    parts.append(&mut c.generated);
    if !file.trailing.is_empty() {
        parts.push(comment_block(&file.trailing).join("\n"));
    }
    if file.prefix.is_some() && !file.items.iter().any(|i| matches!(i, Item::Module(_))) {
        c.warn("top-level `prefix` has no module to apply to and was dropped");
    }
    let uses = use_lines(&c, sources, idx);
    let mut text = String::new();
    if !uses.is_empty() {
        text.push_str(&uses.join("\n"));
        text.push_str("\n\n");
    }
    text.push_str(&parts.join("\n\n"));
    text.push('\n');
    warnings.append(&mut c.warnings);
    Some(finish(&s.rel, text, warnings))
}

fn convert_items(c: &mut FileConv<'_>, file: &V1File) -> Vec<String> {
    file.items
        .iter()
        .map(|item| match item {
            Item::Model(m) => c.model(m),
            Item::Enum(e) => c.enum_decl(e),
            Item::Constants(g) => c.constants(g),
            Item::Module(m) => c.module(m, file),
        })
        .collect()
}

/// Passes the text through the formatter; on failure keeps it as written.
fn finish(rel: &str, text: String, warnings: &mut Vec<String>) -> String {
    match crate::fmt::format(&text) {
        Ok(t) => t,
        Err(diags) => {
            let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
            warnings.push(format!("{rel}: the migrated source could not be formatted ({})", codes.join(", ")));
            text
        }
    }
}

/// A file that failed to parse is kept as comments so no text is lost.
fn unparsed(s: &Source, error: &str) -> String {
    let mut out = format!("{}\n", todo_comment(&format!("this v1 file could not be parsed ({error}); the original follows")));
    for l in s.raw.lines() {
        out.push_str(&format!("// {l}\n"));
    }
    out
}

/// `use module.{A, B}` lines for the names this file takes from other files.
fn use_lines(c: &FileConv<'_>, sources: &[Source], idx: usize) -> Vec<String> {
    let mut by_module: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for r in c.refs.iter().filter(|r| !c.locals.contains(*r)) {
        let target = c
            .ctx
            .decls
            .get(r)
            .copied()
            .or_else(|| c.ctx.errors.values().find(|g| &g.name == r).map(|g| g.file));
        match target {
            Some(t) if t != idx => {
                by_module.entry(sources[t].module.as_str()).or_default().insert(r.as_str());
            }
            _ => {}
        }
    }
    by_module
        .into_iter()
        .map(|(m, names)| format!("use {m}.{{{}}}", names.into_iter().collect::<Vec<_>>().join(", ")))
        .collect()
}

/// Imports are rebuilt from the names each file uses. Warn about explicit
/// imports that match no file, because they may have named something missing.
fn check_imports(sources: &[Source], warnings: &mut Vec<String>) {
    let stems: Vec<&str> = sources.iter().map(|s| s.rel.strip_suffix(".veld").unwrap_or(&s.rel)).collect();
    for s in sources {
        let Ok(f) = &s.parsed else { continue };
        for imp in f.imports.iter().filter(|i| !i.glob) {
            let hit = stems.iter().any(|st| *st == imp.text || st.ends_with(&format!("/{}", imp.text)) || st.ends_with(&imp.text));
            if !hit {
                warnings.push(format!("{}: import `{}` matches no file in this package", s.rel, imp.text));
            }
        }
    }
}

/// Compiles the output as one package and reports what still fails.
fn check_compiles(files: &[(String, String)], warnings: &mut Vec<String>) {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "migrated".to_string(),
            kind: PackageKind::Service,
            files: files
                .iter()
                .map(|(p, t)| SourceFile { path: p.clone(), text: t.clone() })
                .collect(),
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    for d in out.diagnostics.iter().filter(|d| d.is_error()) {
        let loc = out
            .sources
            .line_col(d.primary.file, d.primary.start)
            .map(|(l, c)| format!("{}:{l}:{c}", out.sources.path(d.primary.file).unwrap_or("?")))
            .unwrap_or_default();
        warnings.push(format!("migrated output has an error {} at {loc}: {}", d.code, d.message));
    }
}

#[cfg(test)]
mod tests;

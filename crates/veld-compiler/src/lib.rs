//! The Veld compiler: `.veld` sources in, [`veld_ir::Ir`] and diagnostics out.
//!
//! Pipeline: lex, parse (error-tolerant), semantic analysis (module graph,
//! name resolution, inheritance, type checks, services, annotations), lower.
//! Semantic analysis is skipped when a file has syntax errors, so that a
//! broken parse does not produce a cascade of misleading follow-up errors.

use std::collections::BTreeMap;

pub mod access;
pub mod codes;
pub mod diagnostics;
pub mod index;
pub mod language;
pub mod lower;
pub mod messages;
pub mod semantic;
pub mod signature;
pub mod syntax;

#[cfg(test)]
mod messages_tests;

pub use access::AccessRules;
pub use diagnostics::{Diagnostic, FileId, Severity, SourceMap, Span};
pub use syntax::lexer::Comment;

use diagnostics::has_errors;
use semantic::{ParsedFile, ParsedPackage};
use veld_ir::PackageKind;

/// One source file; `path` is relative to the package root with `/` separators.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct PackageInput {
    pub name: String,
    pub kind: PackageKind,
    pub files: Vec<SourceFile>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CompileInput {
    pub packages: Vec<PackageInput>,
    /// service name -> consumed service names. Cycles are allowed.
    pub consumes: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub struct CompileOutput {
    /// `Some` only when there are no errors.
    pub ir: Option<veld_ir::Ir>,
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
}

impl CompileOutput {
    /// All diagnostics rendered as plain text.
    #[must_use]
    pub fn render(&self) -> String {
        self.sources.render_all(&self.diagnostics)
    }
}

/// Parses one file for tools such as the formatter and the language server.
#[must_use]
pub fn parse_file(text: &str, file: FileId) -> (syntax::ast::File, Vec<Comment>, Vec<Diagnostic>) {
    let p = syntax::parser::parse(text, file);
    (p.file, p.comments, p.diagnostics)
}

/// Compiles a whole workspace.
#[must_use]
pub fn compile(input: &CompileInput) -> CompileOutput {
    compile_with(input, None)
}

/// Compiles a whole workspace and, when `rules` is given, also checks that every `use` of a
/// registry package is allowed (`E0206`).
#[must_use]
pub fn compile_with(input: &CompileInput, rules: Option<&AccessRules>) -> CompileOutput {
    compile_prefixed(input, rules, &BTreeMap::new())
}

/// Like [`compile_with`], with a route prefix per package name (`[workspace] prefix` joined with
/// `[service] prefix`), prepended to every service prefix of that package so that all targets
/// agree on the full route. Prefixes must be absolute (`/api`); the project config validates them.
#[must_use]
pub fn compile_prefixed(
    input: &CompileInput,
    rules: Option<&AccessRules>,
    prefixes: &BTreeMap<String, String>,
) -> CompileOutput {
    let mut sources = SourceMap::default();
    let mut diags = Vec::new();
    let mut parsed = parse_packages(input, prefixes, &mut sources, &mut diags);
    parsed.push(parse_prelude(&mut sources, &mut diags));
    if let Some(rules) = rules {
        diags.extend(access::check(rules, &parsed));
    }
    if has_errors(&diags) {
        return finish(None, diags, sources);
    }
    let ir = semantic::analyze(&parsed, &input.consumes, &mut diags)
        .map(|prog| lower::lower(&prog, &sources, &input.consumes));
    finish(ir, diags, sources)
}

pub(crate) fn finish(
    ir: Option<veld_ir::Ir>,
    mut diags: Vec<Diagnostic>,
    sources: SourceMap,
) -> CompileOutput {
    diags.sort_by_key(|d| (d.primary.file, d.primary.start, d.code));
    CompileOutput {
        ir: if has_errors(&diags) { None } else { ir },
        diagnostics: diags,
        sources,
    }
}

pub(crate) fn parse_packages(
    input: &CompileInput,
    prefixes: &BTreeMap<String, String>,
    sources: &mut SourceMap,
    diags: &mut Vec<Diagnostic>,
) -> Vec<ParsedPackage> {
    let mut order: Vec<&PackageInput> = input.packages.iter().collect();
    order.sort_by(|a, b| a.name.cmp(&b.name));
    order
        .into_iter()
        .map(|pkg| {
            let mut files: Vec<&SourceFile> = pkg.files.iter().collect();
            files.sort_by(|a, b| a.path.cmp(&b.path));
            let files = files
                .into_iter()
                .map(|f| {
                    let id = sources.add(format!("{}/{}", pkg.name, f.path), f.text.clone());
                    let p = syntax::parser::parse(&f.text, id);
                    diags.extend(p.diagnostics);
                    ParsedFile {
                        file: id,
                        rel_path: f.path.clone(),
                        ast: p.file,
                    }
                })
                .collect();
            ParsedPackage {
                name: pkg.name.clone(),
                kind: pkg.kind,
                base_url: pkg.base_url.clone(),
                prefix: prefixes.get(&pkg.name).cloned().unwrap_or_default(),
                files,
            }
        })
        .collect()
}

pub(crate) fn parse_prelude(sources: &mut SourceMap, diags: &mut Vec<Diagnostic>) -> ParsedPackage {
    let id = sources.add(
        format!("{}/{}", language::PRELUDE_PACKAGE, language::PRELUDE_FILE),
        language::PRELUDE_SOURCE,
    );
    let p = syntax::parser::parse(language::PRELUDE_SOURCE, id);
    diags.extend(p.diagnostics);
    ParsedPackage {
        name: language::PRELUDE_PACKAGE.to_string(),
        kind: PackageKind::Prelude,
        base_url: None,
        prefix: String::new(),
        files: vec![ParsedFile {
            file: id,
            rel_path: language::PRELUDE_FILE.to_string(),
            ast: p.file,
        }],
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// Compiles `src` as the single module `app.veld` of package `app`.
    pub fn one(src: &str) -> CompileOutput {
        multi(&[("app", "app.veld", src)])
    }

    /// Compiles (package, path, text) triples; every package is a service.
    pub fn multi(files: &[(&str, &str, &str)]) -> CompileOutput {
        let mut packages: BTreeMap<&str, Vec<SourceFile>> = BTreeMap::new();
        for (pkg, path, text) in files {
            packages.entry(pkg).or_default().push(SourceFile {
                path: (*path).to_string(),
                text: (*text).to_string(),
            });
        }
        let packages = packages
            .into_iter()
            .map(|(name, files)| PackageInput {
                name: name.to_string(),
                kind: PackageKind::Service,
                files,
                base_url: None,
            })
            .collect();
        compile(&CompileInput {
            packages,
            consumes: BTreeMap::new(),
        })
    }

    /// Diagnostic codes of `src`, in output order.
    pub fn codes(src: &str) -> Vec<&'static str> {
        one(src).diagnostics.iter().map(|d| d.code).collect()
    }

    /// The IR of `src`; panics (test only) when compilation failed.
    pub fn ir(src: &str) -> veld_ir::Ir {
        let out = one(src);
        match out.ir {
            Some(ir) => ir,
            None => panic!("compile failed:\n{}", out.render()),
        }
    }
}

//! Document symbols (outline), semantic tokens and formatting.

use lsp_types::{
    DocumentSymbol, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend,
    SymbolKind, TextEdit,
};
use veld_compiler::index::{Index, RefTarget};
use veld_compiler::semantic::DKind;
use veld_compiler::syntax::ast::{self, DeclKind};
use veld_compiler::{FileId, Span};

use crate::text::LineIndex;
use crate::workspace::Analysis;

fn symbol(
    lines: &LineIndex,
    name: &str,
    detail: Option<String>,
    kind: SymbolKind,
    spans: (Span, Span),
    children: Vec<DocumentSymbol>,
) -> DocumentSymbol {
    let (full, sel) = spans;
    #[allow(deprecated)]
    DocumentSymbol {
        name: name.to_string(),
        detail,
        kind,
        tags: None,
        deprecated: None,
        range: lines.range(full.start as usize, full.end as usize),
        selection_range: lines.range(sel.start as usize, sel.end as usize),
        children: (!children.is_empty()).then_some(children),
    }
}

fn src(ix: &Index, span: Span) -> &str {
    ix.sources
        .text(span.file)
        .and_then(|t| t.get(span.start as usize..span.end as usize))
        .unwrap_or("")
}

/// The outline of one file.
#[must_use]
pub fn document_symbols(a: &Analysis, file: FileId) -> Vec<DocumentSymbol> {
    let (Some(f), Some(lines)) = (a.index.file(file), a.lines(file)) else {
        return Vec::new();
    };
    f.ast
        .decls
        .iter()
        .map(|d| decl_symbol(&a.index, lines, d))
        .collect()
}

fn fields(ix: &Index, lines: &LineIndex, fs: &[ast::Field]) -> Vec<DocumentSymbol> {
    fs.iter()
        .map(|f| {
            let opt = if f.optional { "?" } else { "" };
            let detail = format!("{opt}: {}", src(ix, f.ty.span));
            let spans = (f.span, f.name.span);
            symbol(
                lines,
                &f.name.name,
                Some(detail),
                SymbolKind::FIELD,
                spans,
                Vec::new(),
            )
        })
        .collect()
}

fn decl_symbol(ix: &Index, lines: &LineIndex, d: &ast::Decl) -> DocumentSymbol {
    let name = d.name();
    let spans = (d.span, name.span);
    let sym = |kind, detail, kids| symbol(lines, &name.name, detail, kind, spans, kids);
    match &d.kind {
        DeclKind::Model(m) => {
            let detail = m
                .extends
                .as_ref()
                .map(|e| format!("extends {}", src(ix, e.span)));
            sym(SymbolKind::STRUCT, detail, fields(ix, lines, &m.fields))
        }
        DeclKind::Error(e) => {
            let detail = e.status.map(|(s, _)| format!("error {s}"));
            sym(SymbolKind::STRUCT, detail, fields(ix, lines, &e.fields))
        }
        DeclKind::Event(e) => sym(SymbolKind::EVENT, None, fields(ix, lines, &e.fields)),
        DeclKind::Channel(_) => sym(
            SymbolKind::INTERFACE,
            Some("channel".to_string()),
            Vec::new(),
        ),
        DeclKind::Enum(e) => {
            let kids = e
                .variants
                .iter()
                .map(|v| {
                    let spans = (v.span, v.name.span);
                    symbol(
                        lines,
                        &v.name.name,
                        None,
                        SymbolKind::ENUM_MEMBER,
                        spans,
                        Vec::new(),
                    )
                })
                .collect();
            sym(SymbolKind::ENUM, None, kids)
        }
        DeclKind::Union(u) => {
            let kids = u
                .arms
                .iter()
                .map(|a| {
                    let detail = Some(src(ix, a.ty.span).to_string());
                    let spans = (a.span, a.label.span);
                    symbol(
                        lines,
                        &a.label.name,
                        detail,
                        SymbolKind::FIELD,
                        spans,
                        Vec::new(),
                    )
                })
                .collect();
            sym(SymbolKind::INTERFACE, None, kids)
        }
        DeclKind::Const(c) => sym(
            SymbolKind::CONSTANT,
            Some(src(ix, c.ty.span).to_string()),
            Vec::new(),
        ),
        DeclKind::Service(s) => {
            let kids = s
                .endpoints
                .iter()
                .map(|e| {
                    let m = src(ix, e.method_span).to_uppercase();
                    let detail = Some(format!("{m} {}", e.path.value));
                    let spans = (e.span, e.name.span);
                    symbol(
                        lines,
                        &e.name.name,
                        detail,
                        SymbolKind::METHOD,
                        spans,
                        Vec::new(),
                    )
                })
                .collect();
            let detail = s.prefix.as_ref().map(|p| format!("at \"{}\"", p.value));
            sym(SymbolKind::CLASS, detail, kids)
        }
    }
}

// ---- formatting ----------------------------------------------------------------

/// A single edit replacing the file with its canonical form; empty when already canonical.
/// `None` when the file has syntax errors.
#[must_use]
pub fn format_edits(a: &Analysis, file: FileId) -> Option<Vec<TextEdit>> {
    let lines = a.lines(file)?;
    let formatted = veld_tools::fmt::format(lines.text()).ok()?;
    if formatted == lines.text() {
        return Some(Vec::new());
    }
    Some(vec![TextEdit::new(lines.full_range(), formatted)])
}

// ---- semantic tokens -----------------------------------------------------------

const T_TYPE: u32 = 0;
const T_ENUM: u32 = 1;
const T_MEMBER: u32 = 2;
const T_PROPERTY: u32 = 3;
const T_FUNCTION: u32 = 4;
const T_PARAM: u32 = 5;
const T_DECORATOR: u32 = 6;
const T_NAMESPACE: u32 = 7;
const T_VARIABLE: u32 = 8;
const M_DECLARATION: u32 = 1;

#[must_use]
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::TYPE,
            SemanticTokenType::ENUM,
            SemanticTokenType::ENUM_MEMBER,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::DECORATOR,
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::VARIABLE,
        ],
        token_modifiers: vec![SemanticTokenModifier::DECLARATION],
    }
}

struct Raw {
    span: Span,
    ty: u32,
    mods: u32,
}

fn raw(span: Span, ty: u32, mods: u32) -> Raw {
    Raw { span, ty, mods }
}

/// Identifier tokens of a file: names, types, annotations and `use` paths. Keywords and
/// literals are left to the TextMate grammar.
#[must_use]
pub fn semantic_tokens(a: &Analysis, file: FileId) -> Vec<SemanticToken> {
    let (Some(f), Some(lines)) = (a.index.file(file), a.lines(file)) else {
        return Vec::new();
    };
    let ix = &a.index;
    let mut out: Vec<Raw> = Vec::new();
    for u in &f.ast.uses {
        out.extend(
            u.package
                .iter()
                .chain(&u.module)
                .map(|i| raw(i.span, T_NAMESPACE, 0)),
        );
    }
    for d in &f.ast.decls {
        decl_tokens(d, &mut out);
    }
    // Type references take their colour from the declaration they resolve to.
    for r in ix.refs.iter().filter(|r| r.span.file == file) {
        if let RefTarget::Decl(d) = &r.target {
            let is_enum = ix.decls.get(*d).is_some_and(|d| d.kind == DKind::Enum);
            out.push(raw(r.span, if is_enum { T_ENUM } else { T_TYPE }, 0));
        }
    }
    encode(lines, out)
}

fn annotations(anns: &[ast::Annotation], out: &mut Vec<Raw>) {
    out.extend(anns.iter().map(|an| raw(an.name.span, T_DECORATOR, 0)));
}

fn field_tokens(fs: &[ast::Field], out: &mut Vec<Raw>) {
    for f in fs {
        annotations(&f.annotations, out);
        out.push(raw(f.name.span, T_PROPERTY, M_DECLARATION));
    }
}

fn decl_tokens(d: &ast::Decl, out: &mut Vec<Raw>) {
    annotations(&d.annotations, out);
    let name = d.name().span;
    match &d.kind {
        DeclKind::Model(m) => {
            out.push(raw(name, T_TYPE, M_DECLARATION));
            field_tokens(&m.fields, out);
        }
        DeclKind::Error(e) => {
            out.push(raw(name, T_TYPE, M_DECLARATION));
            field_tokens(&e.fields, out);
        }
        DeclKind::Event(e) => {
            out.push(raw(name, T_TYPE, M_DECLARATION));
            field_tokens(&e.fields, out);
        }
        DeclKind::Channel(_) => out.push(raw(name, T_TYPE, M_DECLARATION)),
        DeclKind::Union(u) => {
            out.push(raw(name, T_TYPE, M_DECLARATION));
            out.extend(
                u.arms
                    .iter()
                    .map(|a| raw(a.label.span, T_PROPERTY, M_DECLARATION)),
            );
        }
        DeclKind::Enum(e) => {
            out.push(raw(name, T_ENUM, M_DECLARATION));
            for v in &e.variants {
                annotations(&v.annotations, out);
                out.push(raw(v.name.span, T_MEMBER, M_DECLARATION));
            }
        }
        DeclKind::Const(_) => out.push(raw(name, T_VARIABLE, M_DECLARATION)),
        DeclKind::Service(s) => {
            out.push(raw(name, T_TYPE, M_DECLARATION));
            for e in &s.endpoints {
                annotations(&e.annotations, out);
                out.push(raw(e.name.span, T_FUNCTION, M_DECLARATION));
                for p in &e.params {
                    annotations(&p.annotations, out);
                    out.push(raw(p.name.span, T_PARAM, M_DECLARATION));
                }
            }
        }
    }
}

fn encode(lines: &LineIndex, mut toks: Vec<Raw>) -> Vec<SemanticToken> {
    toks.sort_by_key(|r| (r.span.start, r.span.end));
    toks.dedup_by_key(|r| (r.span.start, r.span.end));
    let mut out = Vec::new();
    let (mut prev_line, mut prev_col) = (0u32, 0u32);
    for r in toks {
        let s = lines.position(r.span.start as usize);
        let e = lines.position(r.span.end as usize);
        if s.line != e.line || e.character <= s.character {
            continue;
        }
        let delta_line = s.line - prev_line;
        let delta_start = if delta_line == 0 {
            s.character.saturating_sub(prev_col)
        } else {
            s.character
        };
        out.push(SemanticToken {
            delta_line,
            delta_start,
            length: e.character - s.character,
            token_type: r.ty,
            token_modifiers_bitset: r.mods,
        });
        (prev_line, prev_col) = (s.line, s.character);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;
    use std::fs;

    fn analysis(src: &str) -> (tempfile::TempDir, Analysis, FileId) {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("veld.toml"), "[service]\nname = \"p\"\n").unwrap();
        fs::create_dir_all(tmp.path().join("contracts")).unwrap();
        let p = tmp.path().join("contracts/m.veld");
        fs::write(&p, src).unwrap();
        let a = Workspace::new(Some(tmp.path().to_path_buf())).analyze(None);
        let f = a.file_for(&p).unwrap();
        (tmp, a, f)
    }

    const SRC: &str = "enum Role { admin }\nmodel A {\n  r: Role\n  n: int @min(1)\n}\nservice S at \"/s\" {\n  get \"/\" list(q: A) -> A\n}\n";

    #[test]
    fn outline_nests_members_under_declarations() {
        let (_t, a, f) = analysis(SRC);
        let syms = document_symbols(&a, f);
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Role", "A", "S"]);
        assert_eq!(syms[0].children.as_ref().unwrap()[0].name, "admin");
        assert_eq!(syms[1].children.as_ref().unwrap().len(), 2);
        let ep = &syms[2].children.as_ref().unwrap()[0];
        assert_eq!(
            (ep.name.as_str(), ep.detail.as_deref()),
            ("list", Some("GET /"))
        );
        assert_eq!(syms[1].selection_range.start.line, 1);
    }

    #[test]
    fn formatting_returns_one_edit_or_none() {
        let (_t, a, f) = analysis("model A{x:int}\n");
        let edits = format_edits(&a, f).unwrap();
        assert_eq!(edits.len(), 1);
        assert!(edits[0].new_text.contains("model A {"));
        let (_t, a, f) = analysis(&edits[0].new_text.clone());
        assert!(format_edits(&a, f).unwrap().is_empty());
        let (_t, a, f) = analysis("model A {");
        assert!(format_edits(&a, f).is_none());
    }

    #[test]
    fn semantic_tokens_are_ordered_single_line_and_typed() {
        let (_t, a, f) = analysis(SRC);
        let toks = semantic_tokens(&a, f);
        assert!(!toks.is_empty());
        let mut abs = Vec::new();
        let (mut line, mut col) = (0, 0);
        for t in &toks {
            line += t.delta_line;
            col = if t.delta_line == 0 {
                col + t.delta_start
            } else {
                t.delta_start
            };
            abs.push((line, col, t.length, t.token_type));
        }
        let mut sorted = abs.clone();
        sorted.sort_unstable();
        assert_eq!(abs, sorted);
        assert_eq!(abs.iter().filter(|t| t.3 == T_ENUM).count(), 2);
        assert!(abs.iter().any(|t| t.3 == T_DECORATOR));
        assert!(abs.iter().any(|t| t.3 == T_FUNCTION && t.0 == 6));
    }
}

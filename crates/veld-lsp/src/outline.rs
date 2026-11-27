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
    let ix = &a.index;
    let mut all: Vec<DocumentSymbol> = f
        .ast
        .decls
        .iter()
        .map(|d| decl_symbol(ix, lines, d))
        .collect();
    all.extend(f.ast.policies.iter().map(|p| policy_symbol(ix, lines, p)));
    all.extend(
        f.ast
            .principals
            .iter()
            .map(|p| principal_symbol(ix, lines, p)),
    );
    all.extend(f.ast.examples.iter().map(|x| example_symbol(ix, lines, x)));
    all.sort_by_key(|s| (s.range.start.line, s.range.start.character));
    all
}

/// An expression as one line of text.
fn one_line(ix: &Index, span: Span) -> String {
    src(ix, span)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn word_span(start: u32, word: &str) -> Span {
    let end = start.saturating_add(u32::try_from(word.len()).unwrap_or(0));
    Span {
        file: FileId::NONE,
        start,
        end,
    }
}

/// The invariants of a model or error, as symbols.
fn rule_symbols(ix: &Index, lines: &LineIndex, rules: &[ast::Rule]) -> Vec<DocumentSymbol> {
    rules
        .iter()
        .map(|r| {
            let audited = r.annotations.iter().any(|a| a.name.name == "rollout");
            let text = one_line(ix, r.expr.span);
            let detail = if audited {
                format!("{text} (audit)")
            } else {
                text
            };
            let spans = (r.span, r.name.span);
            let kind = SymbolKind::BOOLEAN;
            symbol(lines, &r.name.name, Some(detail), kind, spans, Vec::new())
        })
        .collect()
}

/// A field and rule list in source order.
fn members(
    ix: &Index,
    lines: &LineIndex,
    fs: &[ast::Field],
    rules: &[ast::Rule],
) -> Vec<DocumentSymbol> {
    let mut all = fields(ix, lines, fs);
    all.extend(rule_symbols(ix, lines, rules));
    all.sort_by_key(|s| (s.range.start.line, s.range.start.character));
    all
}

fn statement_symbols(ix: &Index, lines: &LineIndex, ep: &ast::Endpoint) -> Vec<DocumentSymbol> {
    let Some(block) = &ep.block else {
        return Vec::new();
    };
    block
        .stmts
        .iter()
        .map(|st| match st {
            ast::Stmt::Fact(f) => {
                let detail = format!(
                    "{}: {}",
                    if f.optional { "?" } else { "" },
                    src(ix, f.ty.span)
                );
                let spans = (f.span, f.name.span);
                symbol(
                    lines,
                    &f.name.name,
                    Some(detail),
                    SymbolKind::VARIABLE,
                    spans,
                    Vec::new(),
                )
            }
            ast::Stmt::Require(r) => {
                let sel = Span {
                    file: r.span.file,
                    ..word_span(r.span.start, "require")
                };
                let name = r.name.as_ref().map_or("require", |n| n.name.as_str());
                let spans = (r.span, r.name.as_ref().map_or(sel, |n| n.span));
                let detail = Some(one_line(ix, r.expr.span));
                symbol(lines, name, detail, SymbolKind::BOOLEAN, spans, Vec::new())
            }
            ast::Stmt::Ensure(e) => {
                let sel = Span {
                    file: e.span.file,
                    ..word_span(e.span.start, "ensure")
                };
                let name = e.name.as_ref().map_or("ensure", |n| n.name.as_str());
                let spans = (e.span, e.name.as_ref().map_or(sel, |n| n.span));
                let detail = Some(one_line(ix, e.expr.span));
                symbol(lines, name, detail, SymbolKind::BOOLEAN, spans, Vec::new())
            }
            ast::Stmt::Transition(t) => {
                let path: Vec<&str> = t.path.iter().map(|p| p.name.as_str()).collect();
                let from: Vec<&str> = t.from.iter().map(|p| p.name.as_str()).collect();
                let name = format!("transition {}", path.join("."));
                let detail = format!("{} -> {}", from.join(", "), t.to.name);
                let sel = Span {
                    file: t.span.file,
                    ..word_span(t.span.start, "transition")
                };
                symbol(
                    lines,
                    &name,
                    Some(detail),
                    SymbolKind::OPERATOR,
                    (t.span, sel),
                    Vec::new(),
                )
            }
            ast::Stmt::Allow(a) => {
                let name = format!("allow {}", a.policy.name);
                let args: Vec<String> = a.args.iter().map(|x| one_line(ix, x.span)).collect();
                let sel = Span {
                    file: a.span.file,
                    ..word_span(a.span.start, "allow")
                };
                symbol(
                    lines,
                    &name,
                    Some(args.join(", ")),
                    SymbolKind::KEY,
                    (a.span, sel),
                    Vec::new(),
                )
            }
        })
        .collect()
}

fn policy_symbol(ix: &Index, lines: &LineIndex, p: &ast::PolicyDecl) -> DocumentSymbol {
    let params: Vec<String> = p
        .params
        .iter()
        .map(|x| format!("{}: {}", x.name.name, src(ix, x.ty.span)))
        .collect();
    let kids = p
        .clauses
        .iter()
        .map(|c| {
            let word = if c.forbid { "forbid" } else { "permit" };
            let sel = Span {
                file: c.span.file,
                ..word_span(c.span.start, word)
            };
            let detail = Some(one_line(ix, c.expr.span));
            symbol(
                lines,
                word,
                detail,
                SymbolKind::BOOLEAN,
                (c.span, sel),
                Vec::new(),
            )
        })
        .collect();
    let detail = Some(format!("({})", params.join(", ")));
    symbol(
        lines,
        &p.name.name,
        detail,
        SymbolKind::FUNCTION,
        (p.span, p.name.span),
        kids,
    )
}

fn principal_symbol(ix: &Index, lines: &LineIndex, p: &ast::PrincipalDecl) -> DocumentSymbol {
    let detail = match &p.name {
        Some(n) => format!("{} as {}", src(ix, p.ty.span), n.name),
        None => src(ix, p.ty.span).to_owned(),
    };
    let sel = Span {
        file: p.span.file,
        ..word_span(p.span.start, "principal")
    };
    symbol(
        lines,
        "principal",
        Some(detail),
        SymbolKind::OBJECT,
        (p.span, sel),
        Vec::new(),
    )
}

fn example_symbol(ix: &Index, lines: &LineIndex, x: &ast::Example) -> DocumentSymbol {
    let name = if x.name.value.is_empty() {
        "example"
    } else {
        x.name.value.as_str()
    };
    let detail = x
        .body
        .as_call()
        .map(|c| format!("{} {}", c.endpoint.name, one_line(ix, c.outcome.span)));
    symbol(
        lines,
        name,
        detail,
        SymbolKind::STRING,
        (x.span, x.name.span),
        Vec::new(),
    )
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
            sym(
                SymbolKind::STRUCT,
                detail,
                members(ix, lines, &m.fields, &m.rules),
            )
        }
        DeclKind::Error(e) => {
            let detail = e.status.map(|(s, _)| format!("error {s}"));
            sym(
                SymbolKind::STRUCT,
                detail,
                members(ix, lines, &e.fields, &e.rules),
            )
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
                    let kids = statement_symbols(ix, lines, e);
                    symbol(lines, &e.name.name, detail, SymbolKind::METHOD, spans, kids)
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
const T_KEYWORD: u32 = 9;
const M_DECLARATION: u32 = 1;
const M_DEFAULT_LIBRARY: u32 = 2;

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
            SemanticTokenType::KEYWORD,
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::DEFAULT_LIBRARY,
        ],
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
    rules_tokens(a, file, &mut out);
    encode(lines, out)
}

/// The words of the rules language: the words that start a statement, the names inside
/// expressions and the built-in functions.
fn rules_tokens(a: &Analysis, file: FileId, out: &mut Vec<Raw>) {
    use crate::rfind::What;
    use crate::scope::LocalKind;
    let ix = &a.index;
    for f in crate::rfind::collect(a, file, None) {
        let decl = if f.def == Some(f.span) {
            M_DECLARATION
        } else {
            0
        };
        let (ty, mods) = match &f.what {
            What::Keyword(_) => (T_KEYWORD, 0),
            What::Rule { .. } | What::Require { .. } | What::Ensure { .. } => {
                let is_word = matches!(src(ix, f.span), "rule" | "require" | "ensure");
                if is_word {
                    (T_KEYWORD, 0)
                } else {
                    (T_VARIABLE, M_DECLARATION)
                }
            }
            What::Local(l) => match l.kind {
                LocalKind::Field { .. } => (T_PROPERTY, decl),
                LocalKind::Param { .. } | LocalKind::PolicyParam { .. } => (T_PARAM, decl),
                LocalKind::Fact { .. } | LocalKind::Principal { .. } => (T_VARIABLE, decl),
                LocalKind::Now | LocalKind::Result => (T_VARIABLE, M_DEFAULT_LIBRARY),
            },
            What::Lambda(_) | What::Param { .. } => (T_PARAM, decl),
            What::Member { .. } | What::PageMember(_) => (T_PROPERTY, 0),
            What::Variant { .. } | What::State { .. } => (T_MEMBER, 0),
            What::Const(_) | What::Principal { .. } => (T_VARIABLE, decl),
            What::Builtin(_) => (T_FUNCTION, M_DEFAULT_LIBRARY),
            What::Policy { .. } | What::Endpoint { .. } => (T_FUNCTION, decl),
            What::Decl(d) => {
                let is_enum = ix.decls.get(*d).is_some_and(|e| e.kind == DKind::Enum);
                (if is_enum { T_ENUM } else { T_TYPE }, 0)
            }
            What::Example { .. } => continue,
        };
        out.push(raw(f.span, ty, mods));
    }
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

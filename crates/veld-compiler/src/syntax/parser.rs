//! Recursive-descent parser with error recovery.
//!
//! Recovery points: the end of a declaration, the start of the next field,
//! variant, arm or endpoint. One file can therefore report several errors.

#![allow(
    clippy::wildcard_enum_match_arm,
    clippy::match_wildcard_for_single_variants,
    reason = "token scanning: only a few tokens matter, the rest are skipped"
)]

#[cfg(test)]
mod derived_tests;
mod endpoint;
#[cfg(test)]
mod endpoint_tests;
mod event;
#[cfg(test)]
mod event_tests;
mod example;
mod expr;
mod flags;
#[cfg(test)]
mod flags_tests;
#[cfg(test)]
mod idempotency_tests;
mod machine;
#[cfg(test)]
mod platform_p1_tests;
mod policy;
#[cfg(test)]
mod policy_tests;
#[cfg(test)]
mod rule_tests;
mod saga;
#[cfg(test)]
mod saga_tests;
#[cfg(test)]
mod timed_tests;

use super::ast::{
    Annotation, Arg, ArgValue, ConstDecl, Decl, DeclKind, Doc, Endpoint, EnumDecl, ErrorDecl,
    Example, Expr, Field, File, FinalStmt, FlowStmt, HttpMethod, Ident, Import, LitKind, Literal,
    ModelDecl, Param, PolicyDecl, PrincipalDecl, RespItem, Responses, Rule, ServiceDecl, StrLit,
    TypeExpr, TypeExprKind, UnionArm, UnionDecl, Use, Variant,
};
use super::lexer::{Comment, Tok, Token, lex};
use crate::diagnostics::{Diagnostic, FileId, Span};
use crate::language;

/// The top-level items that are not declarations of the module's name table.
#[derive(Default)]
struct Items {
    examples: Vec<Example>,
    policies: Vec<PolicyDecl>,
    principals: Vec<PrincipalDecl>,
}

impl Items {
    fn any(&self) -> bool {
        !(self.examples.is_empty() && self.policies.is_empty() && self.principals.is_empty())
    }
}

/// Result of parsing one file.
#[derive(Debug, Clone)]
pub struct Parsed {
    pub file: File,
    pub comments: Vec<Comment>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses `text`. Never panics; syntax problems become diagnostics.
#[must_use]
pub fn parse(text: &str, file: FileId) -> Parsed {
    let lexed = lex(text, file);
    let mut p = Parser {
        text,
        toks: lexed.tokens,
        pos: 0,
        file,
        diags: lexed.diagnostics,
        depth: 0,
    };
    let ast = p.parse_file();
    Parsed {
        file: ast,
        comments: lexed.comments,
        diagnostics: p.diags,
    }
}

struct Parser<'a> {
    text: &'a str,
    toks: Vec<Token>,
    pos: usize,
    file: FileId,
    diags: Vec<Diagnostic>,
    /// Current nesting of types, annotation arguments and expressions (see [`MAX_NESTING`]).
    depth: usize,
}

/// The deepest nesting of types (`Map<string, Map<...>>`, `int[][]`) and annotation call
/// arguments the parser accepts. Every later pass recurses over these, so the limit keeps the
/// stack bounded for any input.
pub const MAX_NESTING: usize = 64;

/// The deepest rule expression (`spec/rules.md` section 3.1): 32 levels above the leaves. The
/// parser counts the leaf too (a bare `a` is depth 1, `a && b` depth 2, a chain of 33 terms
/// depth 33), and every operator, call, list and parenthesis adds one, also in a
/// left-associative chain. The limit keeps the IR JSON of any rule readable by a JSON reader
/// with the default recursion limit of 128 (see `semantic::rules::depth`).
pub const MAX_EXPR_LEVELS: usize = 32;

/// [`MAX_EXPR_LEVELS`] plus the leaf, in the unit of the parser's depth counter.
pub const MAX_EXPR_NESTING: usize = MAX_EXPR_LEVELS + 1;

const V1_HELP: &str = "this is v1 syntax; see spec/language.md";

impl Parser<'_> {
    // ---- token helpers -------------------------------------------------

    fn tok_at(&self, n: usize) -> &Tok {
        let i = (self.pos + n).min(self.toks.len().saturating_sub(1));
        &self.toks[i].tok
    }

    fn peek(&self) -> &Tok {
        self.tok_at(0)
    }

    fn span_at(&self, n: usize) -> Span {
        let i = (self.pos + n).min(self.toks.len().saturating_sub(1));
        self.toks[i].span
    }

    fn span(&self) -> Span {
        self.span_at(0)
    }

    fn prev_end(&self) -> u32 {
        self.pos
            .checked_sub(1)
            .and_then(|i| self.toks.get(i))
            .map_or(0, |t| t.span.end)
    }

    fn advance(&mut self) -> Token {
        let t = self.toks[self.pos.min(self.toks.len() - 1)].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }

    fn at_word(&self, w: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s == w)
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == t {
            self.advance();
            true
        } else {
            false
        }
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.at_word(w) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn newline_between(&self, from: u32, to: u32) -> bool {
        self.text
            .get(from as usize..to as usize)
            .is_some_and(|s| s.contains('\n'))
    }

    fn err(&mut self, code: &'static str, msg: impl Into<String>, span: Span) {
        self.diags.push(Diagnostic::error(code, msg, span));
    }

    fn unexpected(&mut self, expected: &str) {
        let found = self.peek().describe();
        let span = self.span();
        self.err("E0003", format!("expected {expected}, found {found}"), span);
    }

    fn expect(&mut self, t: &Tok, what: &str) -> Option<Span> {
        if self.peek() == t {
            return Some(self.advance().span);
        }
        self.unexpected(what);
        None
    }

    /// Expects `>`. A `>=` token is split, so `Map<string, int>= ...` keeps its old meaning.
    fn expect_gt(&mut self, what: &str) -> Option<Span> {
        if self.peek() == &Tok::GtEq {
            let at = self.span();
            let i = self.pos.min(self.toks.len() - 1);
            self.toks[i] = Token {
                tok: Tok::Eq,
                span: Span::new(self.file, at.start as usize + 1, at.end as usize),
            };
            return Some(Span::new(
                self.file,
                at.start as usize,
                at.start as usize + 1,
            ));
        }
        self.expect(&Tok::Gt, what)
    }

    fn expect_ident(&mut self, what: &str) -> Option<Ident> {
        if let Tok::Ident(name) = self.peek().clone() {
            let span = self.advance().span;
            if language::is_reserved(&name) {
                self.err(
                    "E0004",
                    format!("`{name}` is a reserved word and cannot be used as {what}"),
                    span,
                );
            }
            return Some(Ident { name, span });
        }
        self.unexpected(what);
        None
    }

    fn expect_str(&mut self, what: &str) -> Option<StrLit> {
        if let Tok::Str(value) = self.peek().clone() {
            let span = self.advance().span;
            return Some(StrLit { value, span });
        }
        self.unexpected(what);
        None
    }

    fn is_field_start_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Ident(_))
            && (matches!(self.tok_at(n + 1), Tok::Colon)
                || (matches!(self.tok_at(n + 1), Tok::Question)
                    && matches!(self.tok_at(n + 2), Tok::Colon)))
    }

    fn is_endpoint_start_at(&self, n: usize) -> bool {
        (matches!(self.tok_at(n), Tok::Ident(w) if language::is_method_word(w))
            && matches!(self.tok_at(n + 1), Tok::Str(_)))
            || self.is_saga_start_at(n)
    }

    // ---- file ----------------------------------------------------------

    fn parse_file(&mut self) -> File {
        let start = self.span();
        let mut uses = Vec::new();
        let mut decls: Vec<Decl> = Vec::new();
        let mut items = Items::default();
        while !self.at_eof() {
            let before = self.pos;
            self.parse_top_item(&mut uses, &mut decls, &mut items);
            if self.pos == before {
                self.advance();
            }
        }
        File {
            uses,
            decls,
            examples: items.examples,
            policies: items.policies,
            principals: items.principals,
            span: start.to(self.span()),
        }
    }

    fn parse_top_item(&mut self, uses: &mut Vec<Use>, decls: &mut Vec<Decl>, items: &mut Items) {
        let item_start = self.pos;
        let docs = self.docs();
        let anns = self.annotations_before();
        match self.peek().clone() {
            Tok::Eof => self.dangling(&docs, &anns),
            Tok::Ident(w) if w == "example" && self.is_example_start_at(0) => {
                self.no_annotations(&anns, "an example");
                if let Some(e) = self.parse_example(docs) {
                    items.examples.push(e);
                } else {
                    self.recover_decl(item_start);
                }
            }
            Tok::Ident(w) if w == "policy" && self.is_policy_start_at(0) => {
                self.no_annotations(&anns, "a policy");
                match self.parse_policy(docs) {
                    Some(p) => items.policies.push(p),
                    None => self.recover_decl(item_start),
                }
            }
            Tok::Ident(w) if w == "principal" && self.is_principal_start_at(0) => {
                self.no_annotations(&anns, "a principal");
                match self.parse_principal(docs) {
                    Some(p) => items.principals.push(p),
                    None => self.recover_decl(item_start),
                }
            }
            Tok::Ident(w) if w == "use" && docs.is_empty() && anns.is_empty() => {
                if !decls.is_empty() || items.any() {
                    let span = self.span();
                    self.err(
                        "E0003",
                        "`use` declarations must come before all other declarations",
                        span,
                    );
                }
                if let Some(u) = self.parse_use() {
                    uses.push(u);
                } else {
                    self.skip_to_top_level();
                }
            }
            Tok::Ident(w) if language::V1_KEYWORDS.contains(&w.as_str()) => self.v1_syntax(&w),
            Tok::Ident(w) if language::is_decl_keyword(&w) || self.is_event_decl_start() => {
                self.warn_dangling_docs_if_any(&docs, false);
                if let Some(d) = self.parse_decl(docs, anns) {
                    decls.push(d);
                } else {
                    self.recover_decl(item_start);
                }
            }
            other => {
                let span = self.span();
                self.err("E0003", format!("expected a declaration (`model`, `enum`, `error`, `union`, `service`, `const`), found {}", other.describe()), span);
                self.skip_to_top_level();
            }
        }
    }

    fn dangling(&mut self, docs: &[Doc], anns: &[Annotation]) {
        self.warn_dangling_docs_if_any(docs, true);
        if let Some(a) = anns.first() {
            self.err(
                "E0003",
                "annotation is not followed by a declaration",
                a.span,
            );
        }
    }

    fn warn_dangling_docs_if_any(&mut self, docs: &[Doc], dangling: bool) {
        if dangling && let Some(d) = docs.first() {
            self.diags.push(Diagnostic::warning(
                "W0001",
                "doc comment is not attached to anything",
                d.span,
            ));
        }
    }

    fn v1_syntax(&mut self, word: &str) {
        let span = self.advance().span;
        self.diags.push(
            Diagnostic::error(
                "E0001",
                format!("`{word}` is Veld v1 syntax and is not valid in v2"),
                span,
            )
            .with_help(V1_HELP),
        );
        if word == "import" {
            let mut end = span.end;
            while !self.at_eof() && !self.newline_between(end, self.span().start) {
                end = self.advance().span.end;
            }
        } else {
            self.advance();
            if matches!(self.peek(), Tok::LBrace) {
                self.skip_balanced_block();
            }
        }
    }

    fn skip_balanced_block(&mut self) {
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.advance().tok {
                Tok::LBrace => depth += 1,
                Tok::RBrace => {
                    depth -= 1;
                    if depth <= 0 {
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    /// Skips to the next token that can start a top-level item.
    fn skip_to_top_level(&mut self) {
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.peek() {
                Tok::LBrace => depth += 1,
                Tok::RBrace => depth = (depth - 1).max(0),
                Tok::Doc(_) | Tok::At if depth == 0 => return,
                Tok::Ident(w)
                    if depth == 0
                        && (language::is_decl_keyword(w)
                            || w == "use"
                            || self.is_rule_decl_start_at(0)
                            || language::V1_KEYWORDS.contains(&w.as_str())) =>
                {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    /// Recovery after a failed declaration that began at token `start`.
    fn recover_decl(&mut self, start: usize) {
        let mut depth: i32 = 0;
        for t in &self.toks[start..self.pos] {
            match t.tok {
                Tok::LBrace => depth += 1,
                Tok::RBrace => depth -= 1,
                _ => {}
            }
        }
        let mut depth = depth.max(0);
        while !self.at_eof() {
            match self.peek() {
                Tok::LBrace => depth += 1,
                Tok::RBrace => {
                    depth -= 1;
                    if depth <= 0 {
                        self.advance();
                        return;
                    }
                }
                Tok::Doc(_) | Tok::At if depth == 0 => return,
                Tok::Ident(w)
                    if depth == 0
                        && self.pos > start
                        && (language::is_decl_keyword(w)
                            || w == "use"
                            || self.is_rule_decl_start_at(0)) =>
                {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    // ---- use -----------------------------------------------------------

    fn parse_use(&mut self) -> Option<Use> {
        let start = self.advance().span;
        let package = self.parse_use_package();
        let mut module = vec![self.expect_ident("a module name")?];
        while matches!(self.peek(), Tok::Dot) && !matches!(self.tok_at(1), Tok::LBrace | Tok::Star)
        {
            self.advance();
            module.push(self.expect_ident("a module name")?);
        }
        self.expect(&Tok::Dot, "`.` before the import list")?;
        if matches!(self.peek(), Tok::Star) {
            let span = self.span();
            self.diags.push(
                Diagnostic::error("E0003", "glob imports do not exist", span)
                    .with_help("list the imported names: `use mod.{A, B}`"),
            );
            return None;
        }
        self.expect(&Tok::LBrace, "`{` to open the import list")?;
        let imports = self.parse_imports()?;
        let end = self.expect(&Tok::RBrace, "`}` to close the import list")?;
        Some(Use {
            package,
            module,
            imports,
            span: start.to(end),
        })
    }

    fn parse_use_package(&mut self) -> Option<Ident> {
        if matches!(self.peek(), Tok::At) {
            let at = self.advance().span;
            let org = self.expect_ident("an organisation name")?;
            self.expect(&Tok::Slash, "`/` in the package name")?;
            let name = self.expect_ident("a package name")?;
            self.expect(&Tok::Colon, "`:` after the package name")?;
            return Some(Ident {
                name: format!("@{}/{}", org.name, name.name),
                span: at.to(name.span),
            });
        }
        if matches!(self.tok_at(1), Tok::Colon) && matches!(self.peek(), Tok::Ident(_)) {
            let id = self.expect_ident("a package name")?;
            self.advance();
            return Some(id);
        }
        None
    }

    fn parse_imports(&mut self) -> Option<Vec<Import>> {
        let mut imports = Vec::new();
        loop {
            if matches!(self.peek(), Tok::RBrace) && !imports.is_empty() {
                break;
            }
            let name = self.expect_ident("an imported name")?;
            let mut span = name.span;
            let alias = if self.eat_word("as") {
                let a = self.expect_ident("an alias")?;
                span = span.to(a.span);
                Some(a)
            } else {
                None
            };
            imports.push(Import { name, alias, span });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        Some(imports)
    }

    // ---- docs and annotations -------------------------------------------

    fn docs(&mut self) -> Vec<Doc> {
        let mut docs = Vec::new();
        while let Tok::Doc(text) = self.peek().clone() {
            let span = self.advance().span;
            docs.push(Doc { text, span });
        }
        docs
    }

    fn annotations_before(&mut self) -> Vec<Annotation> {
        let mut anns = Vec::new();
        while matches!(self.peek(), Tok::At) {
            match self.parse_annotation() {
                Some(a) => anns.push(a),
                None => break,
            }
        }
        anns
    }

    fn parse_annotation(&mut self) -> Option<Annotation> {
        let at = self.advance().span;
        let name = self.expect_ident("an annotation name")?;
        let mut end = name.span;
        let mut when = None;
        let args = if matches!(self.peek(), Tok::LParen) {
            self.advance();
            if self.at_when_argument(&name) {
                self.advance();
                self.advance();
                when = Some(self.parse_expr()?);
                end = self.expect(&Tok::RParen, "`)` to close the annotation arguments")?;
                None
            } else {
                let args = self.parse_args()?;
                end = self.expect(&Tok::RParen, "`)` to close the annotation arguments")?;
                Some(args)
            }
        } else {
            None
        };
        Some(Annotation {
            name,
            args,
            when,
            span: at.to(end),
        })
    }

    /// True after the `(` of `@visible` when `when:` follows: the argument is an expression
    /// (`spec/platform.md` section 2). `when` stays an ordinary name everywhere else.
    fn at_when_argument(&self, name: &Ident) -> bool {
        name.name == "visible"
            && matches!(self.peek(), Tok::Ident(w) if w == "when")
            && matches!(self.tok_at(1), Tok::Colon)
    }

    fn parse_args(&mut self) -> Option<Vec<Arg>> {
        let mut args = Vec::new();
        while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
            args.push(self.parse_arg()?);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        Some(args)
    }

    fn parse_arg(&mut self) -> Option<Arg> {
        let start = self.span();
        let name = if matches!(self.peek(), Tok::Ident(_)) && matches!(self.tok_at(1), Tok::Colon) {
            let id = self.expect_ident("an argument name")?;
            self.advance();
            Some(id)
        } else {
            None
        };
        let value = self.parse_arg_value()?;
        let end = Span {
            file: self.file,
            start: self.prev_end(),
            end: self.prev_end(),
        };
        Some(Arg {
            name,
            value,
            span: start.to(end),
        })
    }

    /// Enters one nesting level; reports E0005 and returns false past [`MAX_NESTING`].
    fn enter(&mut self, levels: usize) -> bool {
        if self.depth + levels > MAX_NESTING {
            let span = self.span();
            self.err(
                "E0005",
                format!("nesting is too deep (the limit is {MAX_NESTING} levels)"),
                span,
            );
            return false;
        }
        self.depth += levels;
        true
    }

    /// Enters one expression level; reports E0005 past [`MAX_EXPR_NESTING`].
    fn enter_expr(&mut self) -> bool {
        if self.depth + 1 > MAX_EXPR_NESTING {
            let span = self.span();
            self.err(
                "E0005",
                format!("nesting is too deep (the limit is {MAX_EXPR_LEVELS} levels)"),
                span,
            );
            return false;
        }
        self.depth += 1;
        true
    }

    fn parse_arg_value(&mut self) -> Option<ArgValue> {
        if let Tok::Ident(w) = self.peek().clone()
            && !matches!(w.as_str(), "true" | "false" | "null")
        {
            let id = self.expect_ident("an argument")?;
            if matches!(self.peek(), Tok::LParen) {
                self.advance();
                if !self.enter(1) {
                    return None;
                }
                let args = self.parse_args();
                self.depth -= 1;
                let args = args?;
                self.expect(&Tok::RParen, "`)` to close the arguments")?;
                return Some(ArgValue::Call { name: id, args });
            }
            return Some(ArgValue::Ident(id));
        }
        self.parse_literal().map(ArgValue::Literal)
    }

    // ---- literals and types -----------------------------------------------

    fn parse_literal(&mut self) -> Option<Literal> {
        let span = self.span();
        let kind = match self.peek().clone() {
            Tok::Str(s) => LitKind::Str(s),
            Tok::Int(n) => LitKind::Int(n),
            Tok::Float(f) => LitKind::Float(f),
            Tok::Ident(w) if w == "true" => LitKind::Bool(true),
            Tok::Ident(w) if w == "false" => LitKind::Bool(false),
            Tok::Ident(w) if w == "null" => LitKind::Null,
            Tok::Dot => {
                self.advance();
                let id = self.expect_ident("an enum variant name")?;
                return Some(Literal {
                    kind: LitKind::Variant(id.name),
                    span: span.to(id.span),
                });
            }
            Tok::LBracket => {
                self.advance();
                let end = self.expect(
                    &Tok::RBracket,
                    "`]` (only the empty list `[]` is a literal)",
                )?;
                return Some(Literal {
                    kind: LitKind::EmptyList,
                    span: span.to(end),
                });
            }
            _ => {
                self.unexpected("a literal");
                return None;
            }
        };
        self.advance();
        Some(Literal { kind, span })
    }

    fn parse_typeref(&mut self) -> Option<TypeExpr> {
        let name = self.expect_ident("a type name")?;
        let mut span = name.span;
        let mut args = Vec::new();
        if matches!(self.peek(), Tok::Lt) {
            self.advance();
            if !self.enter(1) {
                return None;
            }
            let parsed = self.parse_type_args(&mut args);
            self.depth -= 1;
            parsed?;
            span = span.to(self.expect_gt("`>` to close the type arguments")?);
        }
        Some(TypeExpr {
            kind: TypeExprKind::Named { name, args },
            span,
        })
    }

    fn parse_type_args(&mut self, args: &mut Vec<TypeExpr>) -> Option<()> {
        loop {
            args.push(self.parse_type()?);
            if !self.eat(&Tok::Comma) {
                return Some(());
            }
        }
    }

    fn parse_type(&mut self) -> Option<TypeExpr> {
        if self.is_field_start_at(0) && self.newline_between(self.prev_end(), self.span().start) {
            self.unexpected("a type");
            return None;
        }
        let mut ty = self.parse_typeref()?;
        let base = self.depth;
        while matches!(self.peek(), Tok::LBracket) && matches!(self.tok_at(1), Tok::RBracket) {
            if !self.enter(1) {
                self.depth = base;
                return None;
            }
            self.advance();
            let end = self.advance().span;
            let span = ty.span.to(end);
            ty = TypeExpr {
                kind: TypeExprKind::List(Box::new(ty)),
                span,
            };
        }
        self.depth = base;
        Some(ty)
    }

    // ---- declarations -------------------------------------------------------

    fn parse_decl(&mut self, docs: Vec<Doc>, annotations: Vec<Annotation>) -> Option<Decl> {
        let start = annotations.first().map_or(self.span(), |a| a.span);
        let kw_tok = self.advance();
        let Tok::Ident(kw) = kw_tok.tok else {
            return None;
        };
        let kind = match kw.as_str() {
            "model" => DeclKind::Model(self.parse_model()?),
            "error" => DeclKind::Error(self.parse_error_decl()?),
            "enum" => DeclKind::Enum(self.parse_enum()?),
            "union" => DeclKind::Union(self.parse_union()?),
            "service" => DeclKind::Service(self.parse_service()?),
            "const" => DeclKind::Const(self.parse_const()?),
            "event" => DeclKind::Event(self.parse_event()?),
            "channel" => DeclKind::Channel(self.parse_channel(kw_tok.span)?),
            _ => return None,
        };
        let end = Span {
            file: self.file,
            start: self.prev_end(),
            end: self.prev_end(),
        };
        Some(Decl {
            docs,
            annotations,
            kind,
            span: start.to(end),
        })
    }

    fn parse_model(&mut self) -> Option<ModelDecl> {
        let name = self.expect_ident("a model name")?;
        let extends = if self.eat_word("extends") {
            Some(self.parse_typeref()?)
        } else {
            None
        };
        self.expect(&Tok::LBrace, "`{` to open the model body")?;
        let (fields, rules) = self.parse_members();
        Some(ModelDecl {
            name,
            extends,
            fields,
            rules,
        })
    }

    fn parse_error_decl(&mut self) -> Option<ErrorDecl> {
        let name = self.expect_ident("an error name")?;
        let status = if let Tok::Int(n) = self.peek().clone() {
            let span = self.advance().span;
            Some((n, span))
        } else {
            None
        };
        self.expect(&Tok::LBrace, "`{` to open the error body")?;
        let (fields, rules) = self.parse_members();
        Some(ErrorDecl {
            name,
            status,
            fields,
            rules,
        })
    }

    /// Parses fields and rules up to and including the closing brace.
    fn parse_members(&mut self) -> (Vec<Field>, Vec<Rule>) {
        let mut fields = Vec::new();
        let mut rules = Vec::new();
        loop {
            while self.eat(&Tok::Comma) {}
            if self.at_eof() {
                self.unexpected("`}` to close the body");
                break;
            }
            let before = self.pos;
            let docs = self.docs();
            let pre = self.annotations_before();
            if matches!(self.peek(), Tok::RBrace) {
                self.warn_dangling_docs_if_any(&docs, true);
                self.advance();
                break;
            }
            let parsed = if self.is_rule_start_at(0) {
                self.parse_rule(docs, pre).map(|r| rules.push(r))
            } else {
                self.parse_field(docs, pre).map(|f| fields.push(f))
            };
            match parsed {
                Some(()) => {}
                None => {
                    self.recover_field();
                    if self.pos == before {
                        self.advance();
                    }
                }
            }
        }
        (fields, rules)
    }

    fn parse_field(&mut self, docs: Vec<Doc>, mut annotations: Vec<Annotation>) -> Option<Field> {
        let name = self.expect_ident("a field name")?;
        let optional = self.eat(&Tok::Question);
        self.expect(&Tok::Colon, "`:` after the field name")?;
        let ty = self.parse_type()?;
        let (default, derived) = self.parse_default_or_derived()?;
        while matches!(self.peek(), Tok::At)
            && !self.newline_between(self.prev_end(), self.span().start)
        {
            annotations.push(self.parse_annotation()?);
        }
        let end = Span {
            file: self.file,
            start: self.prev_end(),
            end: self.prev_end(),
        };
        Some(Field {
            docs,
            annotations,
            span: name.span.to(end),
            name,
            optional,
            ty,
            default,
            derived,
        })
    }

    /// What follows `=` in a field: a literal is a default, any other expression makes the field
    /// derived (`spec/rules.md` section 17.2). A literal is tried first and kept only when no
    /// operator or `.name` continues it, so every default parses and fails as it always did.
    fn parse_default_or_derived(&mut self) -> Option<(Option<Literal>, Option<Expr>)> {
        if !self.eat(&Tok::Eq) {
            return Some((None, None));
        }
        if !self.starts_expression_not_literal() {
            let (pos, diags, depth) = (self.pos, self.diags.len(), self.depth);
            let lit = self.parse_literal()?;
            if !self.continues_expression() {
                return Some((Some(lit), None));
            }
            self.pos = pos;
            self.diags.truncate(diags);
            self.depth = depth;
        }
        Some((None, Some(self.parse_expr()?)))
    }

    /// A name that is not `true`, `false` or `null`, a parenthesis, `!`, a `-` that is not the
    /// sign of a literal, a list that is not empty: these start an expression, not a literal.
    fn starts_expression_not_literal(&self) -> bool {
        match self.peek() {
            Tok::Ident(w) => !matches!(w.as_str(), "true" | "false" | "null"),
            Tok::LParen | Tok::Bang | Tok::Minus => true,
            // Only the empty list `[]` is a literal.
            Tok::LBracket => !matches!(self.tok_at(1), Tok::RBracket),
            _ => false,
        }
    }

    /// True when the token after a literal continues it as an expression: a binary operator, or
    /// `.name` on the same line.
    fn continues_expression(&self) -> bool {
        self.binary_op().is_some()
            || (matches!(self.peek(), Tok::Dot)
                && matches!(self.tok_at(1), Tok::Ident(_))
                && !self.newline_between(self.prev_end(), self.span().start))
    }

    fn recover_field(&mut self) {
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.peek() {
                Tok::RBrace if depth == 0 => return,
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => depth = (depth - 1).max(0),
                Tok::Doc(_) | Tok::At if depth == 0 => return,
                Tok::Ident(_)
                    if depth == 0 && (self.is_rule_start_at(0) || self.is_field_start_at(0)) =>
                {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    fn parse_enum(&mut self) -> Option<EnumDecl> {
        let name = self.expect_ident("an enum name")?;
        self.expect(&Tok::LBrace, "`{` to open the enum body")?;
        let mut variants = Vec::new();
        let mut flows = Vec::new();
        let mut finals = Vec::new();
        loop {
            while self.eat(&Tok::Comma) {}
            if self.at_eof() {
                self.unexpected("`}` to close the enum");
                break;
            }
            let before = self.pos;
            let docs = self.docs();
            let annotations = self.annotations_before();
            if matches!(self.peek(), Tok::RBrace) {
                self.advance();
                break;
            }
            if self.at_flow_stmt() || self.at_final_stmt() {
                self.enum_statement(&docs, &annotations, &mut flows, &mut finals);
                continue;
            }
            match self.parse_variant(docs, annotations) {
                Some(v) => variants.push(v),
                None => {
                    self.recover_list_item();
                    if self.pos == before {
                        self.advance();
                    }
                }
            }
        }
        Some(EnumDecl {
            name,
            variants,
            flows,
            finals,
        })
    }

    /// A `flow` or `final` statement of an enum body; docs and annotations do not belong to it.
    fn enum_statement(
        &mut self,
        docs: &[Doc],
        annotations: &[Annotation],
        flows: &mut Vec<FlowStmt>,
        finals: &mut Vec<FinalStmt>,
    ) {
        if let Some(d) = docs.first() {
            self.err(
                "E0003",
                "a doc comment documents a variant, not a `flow` or `final` statement",
                d.span,
            );
        }
        if let Some(a) = annotations.first() {
            self.err(
                "E0003",
                "an annotation belongs to a variant, not to a `flow` or `final` statement",
                a.span,
            );
        }
        let before = self.pos;
        if self.at_flow_stmt() {
            flows.extend(self.parse_flow());
        } else {
            finals.extend(self.parse_final());
        }
        if self.pos == before {
            self.advance();
        }
    }

    fn parse_variant(&mut self, docs: Vec<Doc>, annotations: Vec<Annotation>) -> Option<Variant> {
        let name = self.expect_ident("a variant name")?;
        let mut span = name.span;
        let wire = if self.eat(&Tok::Eq) {
            let s = self.expect_str("a string wire value")?;
            span = span.to(s.span);
            Some((s.value, s.span))
        } else {
            None
        };
        Some(Variant {
            docs,
            annotations,
            name,
            wire,
            span,
        })
    }

    /// Skips to the next comma or closing brace of a comma-separated list.
    fn recover_list_item(&mut self) {
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.peek() {
                Tok::RBrace | Tok::Comma if depth == 0 => return,
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => depth = (depth - 1).max(0),
                _ => {}
            }
            self.advance();
        }
    }

    fn parse_union(&mut self) -> Option<UnionDecl> {
        let name = self.expect_ident("a union name")?;
        self.expect(&Tok::LBrace, "`{` to open the union body")?;
        let mut arms = Vec::new();
        loop {
            while self.eat(&Tok::Comma) {}
            if self.at_eof() {
                self.unexpected("`}` to close the union");
                break;
            }
            if matches!(self.peek(), Tok::RBrace) {
                self.advance();
                break;
            }
            let before = self.pos;
            match self.parse_arm() {
                Some(a) => arms.push(a),
                None => {
                    self.recover_list_item();
                    if self.pos == before {
                        self.advance();
                    }
                }
            }
        }
        Some(UnionDecl { name, arms })
    }

    fn parse_arm(&mut self) -> Option<UnionArm> {
        let label = self.expect_ident("an arm label")?;
        self.expect(&Tok::Colon, "`:` after the arm label")?;
        let ty = self.parse_typeref()?;
        let span = label.span.to(ty.span);
        Some(UnionArm { label, ty, span })
    }

    fn parse_const(&mut self) -> Option<ConstDecl> {
        let name = self.expect_ident("a constant name")?;
        self.expect(&Tok::Colon, "`:` after the constant name")?;
        let ty = self.parse_type()?;
        self.expect(&Tok::Eq, "`=` and a value")?;
        let value = self.parse_literal()?;
        Some(ConstDecl { name, ty, value })
    }

    // ---- services -------------------------------------------------------------

    fn parse_service(&mut self) -> Option<ServiceDecl> {
        let name = self.expect_ident("a service name")?;
        let prefix = if matches!(self.peek(), Tok::LBrace) {
            None
        } else if self.eat_word("at") {
            Some(self.expect_str("a string path prefix")?)
        } else {
            self.unexpected("`at` and a path prefix");
            return None;
        };
        self.expect(&Tok::LBrace, "`{` to open the service body")?;
        let mut endpoints = Vec::new();
        let mut publishes = Vec::new();
        let mut subscribes = Vec::new();
        loop {
            if self.at_eof() {
                self.unexpected("`}` to close the service");
                break;
            }
            let before = self.pos;
            let docs = self.docs();
            let anns = self.annotations_before();
            if matches!(self.peek(), Tok::RBrace) {
                self.warn_dangling_docs_if_any(&docs, true);
                self.advance();
                break;
            }
            if let Tok::Ident(w) = self.peek().clone()
                && language::V1_KEYWORDS.contains(&w.as_str())
            {
                self.v1_syntax(&w);
                continue;
            }
            if self.is_event_line_start_at(0) {
                self.service_event_line(&docs, &anns, &mut publishes, &mut subscribes);
                if self.pos == before {
                    self.advance();
                }
                continue;
            }
            if self.is_bare_event_word() {
                self.bare_event_word();
                continue;
            }
            match self.parse_endpoint(docs, anns) {
                Some(e) => endpoints.push(e),
                None => {
                    self.recover_endpoint();
                    if self.pos == before {
                        self.advance();
                    }
                }
            }
        }
        Some(ServiceDecl {
            name,
            prefix,
            endpoints,
            publishes,
            subscribes,
        })
    }

    fn recover_endpoint(&mut self) {
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.peek() {
                Tok::RBrace if depth == 0 => return,
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => depth = (depth - 1).max(0),
                Tok::Doc(_) | Tok::At if depth == 0 => return,
                Tok::Ident(_) if depth == 0 && self.is_endpoint_start_at(0) => return,
                _ => {}
            }
            self.advance();
        }
    }

    fn parse_endpoint(&mut self, docs: Vec<Doc>, annotations: Vec<Annotation>) -> Option<Endpoint> {
        let saga_word = if self.is_saga_start_at(0) {
            Some(self.advance().span)
        } else {
            None
        };
        let method_span = self.span();
        let method = match self.peek().clone() {
            Tok::Ident(w) if HttpMethod::from_word(&w).is_some() => {
                self.advance();
                HttpMethod::from_word(&w)?
            }
            _ => {
                self.unexpected("an HTTP method (`get`, `post`, `put`, `patch`, `delete`, `head`, `options`, `ws`)");
                return None;
            }
        };
        let path = self.expect_str("a string path such as \"/{id}\"")?;
        let name = self.expect_ident("an endpoint name")?;
        self.expect(&Tok::LParen, "`(` to open the parameter list")?;
        let params = self.parse_params()?;
        self.expect(&Tok::RParen, "`)` to close the parameter list")?;
        let responses = if matches!(self.peek(), Tok::Arrow) {
            Some(self.parse_responses()?)
        } else {
            None
        };
        let (block, saga) = if let Some(word) = saga_word {
            if method != HttpMethod::Post {
                self.err("E1203", "a saga is a `post` endpoint", method_span);
            }
            (None, Some(self.parse_saga_block(word)?))
        } else if matches!(self.peek(), Tok::LBrace) {
            (Some(self.parse_block()?), None)
        } else {
            (None, None)
        };
        let end = Span {
            file: self.file,
            start: self.prev_end(),
            end: self.prev_end(),
        };
        Some(Endpoint {
            docs,
            annotations,
            method,
            method_span,
            path,
            name,
            params,
            responses,
            block,
            saga,
            span: saga_word.unwrap_or(method_span).to(end),
        })
    }

    fn parse_params(&mut self) -> Option<Vec<Param>> {
        let mut params = Vec::new();
        while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
            let annotations = self.annotations_before();
            let name = self.expect_ident("a parameter name")?;
            let optional = self.eat(&Tok::Question);
            self.expect(&Tok::Colon, "`:` after the parameter name")?;
            let stream = self.eat_word("stream");
            let ty = self.parse_type()?;
            let span = name.span.to(ty.span);
            params.push(Param {
                annotations,
                name,
                optional,
                stream,
                ty,
                span,
            });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        Some(params)
    }

    fn parse_responses(&mut self) -> Option<Responses> {
        let arrow = self.advance().span;
        let mut items = vec![self.parse_resp_item(true)?];
        while self.eat(&Tok::Pipe) {
            items.push(self.parse_resp_item(false)?);
        }
        let span = arrow.to(items.last().map_or(arrow, |i| i.span));
        Some(Responses { items, span })
    }

    fn parse_resp_item(&mut self, first: bool) -> Option<RespItem> {
        let start = self.span();
        let status = if let Tok::Int(n) = self.peek().clone() {
            let span = self.advance().span;
            Some((n, span))
        } else {
            None
        };
        let type_follows = match self.peek() {
            Tok::Ident(w) if w == "stream" => true,
            Tok::Ident(_) => {
                status.is_none()
                    || !(self.is_endpoint_start_at(0) || self.is_event_line_start_at(0))
            }
            _ => false,
        };
        if status.is_some() && !first && !type_follows {
            self.unexpected("an error type after the status");
            return None;
        }
        let (stream, ty) = if type_follows {
            let stream = self.eat_word("stream");
            (stream, Some(self.parse_type()?))
        } else {
            (false, None)
        };
        if status.is_none() && ty.is_none() {
            self.unexpected("a response type or status");
            return None;
        }
        let end = Span {
            file: self.file,
            start: self.prev_end(),
            end: self.prev_end(),
        };
        Some(RespItem {
            status,
            stream,
            ty,
            span: start.to(end),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::ast::DeclKind;

    fn p(src: &str) -> Parsed {
        parse(src, FileId(0))
    }

    fn codes(src: &str) -> Vec<&'static str> {
        p(src).diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn parses_use_forms() {
        let r = p(
            "use users.{User, Role as R}\nuse iam:a.b.{X}\nuse @acme/billing:invoices.{Invoice,}\n",
        );
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        let u = &r.file.uses;
        assert_eq!(u.len(), 3);
        assert!(u[0].package.is_none());
        assert_eq!(
            u[0].imports[1].alias.as_ref().map(|a| a.name.as_str()),
            Some("R")
        );
        assert_eq!(u[1].package.as_ref().map(|a| a.name.as_str()), Some("iam"));
        assert_eq!(u[1].module_path(), "a.b");
        assert_eq!(
            u[2].package.as_ref().map(|a| a.name.as_str()),
            Some("@acme/billing")
        );
    }

    #[test]
    fn parses_model_with_all_field_forms() {
        let r = p(
            "model A extends B {\n /// docs\n id: uuid @readonly\n n?: string = \"x\" @length(max: 5)\n @wire(\"t\") t: string[]\n m: Map<string, int[]>\n}",
        );
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        let DeclKind::Model(m) = &r.file.decls[0].kind else {
            panic!("unexpected declaration kind")
        };
        assert!(m.extends.is_some());
        assert_eq!(m.fields.len(), 4);
        assert_eq!(m.fields[0].docs.len(), 1);
        assert!(m.fields[1].optional && m.fields[1].default.is_some());
        assert_eq!(m.fields[1].annotations.len(), 1);
        assert_eq!(m.fields[2].annotations[0].name.name, "wire");
    }

    #[test]
    fn annotation_on_next_line_belongs_to_next_field() {
        let r = p("model A {\n a: int\n @readonly\n b: int\n}");
        let DeclKind::Model(m) = &r.file.decls[0].kind else {
            panic!("unexpected declaration kind")
        };
        assert!(m.fields[0].annotations.is_empty());
        assert_eq!(m.fields[1].annotations.len(), 1);
    }

    #[test]
    fn commas_between_fields_are_accepted() {
        assert!(codes("model V { a: string, b: int }").is_empty());
    }

    #[test]
    fn parses_enum_union_const_error() {
        let r = p(
            "enum S { a, b = \"B\", c }\nunion U { x: A, y: B }\nconst MAX: int = 3\nerror E 409 { sku: string }",
        );
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        assert_eq!(r.file.decls.len(), 4);
    }

    #[test]
    fn parses_service_with_responses() {
        let src = "service S at \"/s\" {\n get \"/{id}\" g(id: uuid) -> A | NotFound\n post \"/\" c(body: B) -> 201 A | 409 Out\n get \"/h\" h() -> 204\n get \"/x\" x()\n ws \"/w\" w(body: stream M) -> stream N\n}";
        let r = p(src);
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        let DeclKind::Service(s) = &r.file.decls[0].kind else {
            panic!("unexpected declaration kind")
        };
        assert_eq!(s.endpoints.len(), 5);
        let resp = s.endpoints[1].responses.as_ref().map(|r| r.items.len());
        assert_eq!(resp, Some(2));
        assert!(
            s.endpoints[2]
                .responses
                .as_ref()
                .is_some_and(|r| r.items[0].ty.is_none())
        );
        assert!(s.endpoints[4].params[0].stream);
    }

    #[test]
    fn bare_status_then_next_endpoint_is_not_a_type() {
        let r = p("service S at \"/\" {\n get \"/a\" a() -> 204\n get \"/b\" b() -> 200\n}");
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    }

    #[test]
    fn annotation_args_support_calls_and_named_args() {
        let r = p("@auth(apiKey(header: \"X\"))\nservice S at \"/\" {}");
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        let a = &r.file.decls[0].annotations[0];
        let args = a.args.as_ref();
        assert!(matches!(
            args.and_then(|v| v.first()).map(|a| &a.value),
            Some(ArgValue::Call { .. })
        ));
    }

    #[test]
    fn v1_syntax_reports_e0001_with_help() {
        let r =
            p("import \"./x.veld\"\nmodule M {\n action A { method: GET }\n}\nmodel Ok { a: int }");
        let e: Vec<_> = r.diagnostics.iter().filter(|d| d.code == "E0001").collect();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].help.as_deref(), Some(V1_HELP));
        assert_eq!(r.file.decls.len(), 1);
    }

    #[test]
    fn reserved_word_as_name_is_e0004_but_contextual_words_are_fine() {
        assert_eq!(codes("model use { a: int }"), vec!["E0004"]);
        assert!(codes("model A { get: string, body: int, query: int }").is_empty());
    }

    #[test]
    fn recovery_reports_errors_in_several_fields() {
        let r = p(
            "model A {\n a int\n b: string\n c: \n d: int\n}\nmodel B { e: }\nmodel C { f: int }",
        );
        let n = r.diagnostics.iter().filter(|d| d.code == "E0003").count();
        assert!(n >= 3, "{:?}", r.diagnostics);
        assert_eq!(r.file.decls.len(), 3, "later declarations still parse");
    }

    #[test]
    fn recovery_continues_after_a_broken_declaration() {
        let r = p("model { }\nenum E { a }\nmodel X extends { }\nerror Q { a: int }");
        assert!(r.diagnostics.len() >= 2);
        let names: Vec<_> = r.file.decls.iter().map(|d| d.name().name.clone()).collect();
        assert!(names.contains(&"E".to_string()) && names.contains(&"Q".to_string()));
    }

    #[test]
    fn recovery_in_services_reports_each_bad_endpoint() {
        let r = p(
            "service S at \"/\" {\n get \"/a\" a( -> X\n get \"/b\" b()\n put oops\n get \"/c\" c()\n}",
        );
        assert!(
            r.diagnostics.iter().filter(|d| d.code == "E0003").count() >= 2,
            "{:?}",
            r.diagnostics
        );
        let DeclKind::Service(s) = &r.file.decls[0].kind else {
            panic!("unexpected declaration kind")
        };
        assert!(s.endpoints.iter().any(|e| e.name.name == "c"));
    }

    #[test]
    fn missing_closing_brace_reports_once_and_terminates() {
        let r = p("model A { a: int");
        assert_eq!(r.diagnostics.len(), 1);
    }

    #[test]
    fn glob_import_is_rejected() {
        assert_eq!(codes("use a.*"), vec!["E0003"]);
    }

    #[test]
    fn use_after_decl_is_an_error_but_still_parsed() {
        let r = p("model A { a: int }\nuse b.{B}");
        assert_eq!(r.diagnostics.len(), 1);
        assert_eq!(r.file.uses.len(), 1);
    }

    #[test]
    fn dangling_doc_comment_is_a_warning() {
        let r = p("model A { a: int }\n/// nothing");
        assert_eq!(r.diagnostics.len(), 1);
        assert_eq!(r.diagnostics[0].code, "W0001");
    }

    #[test]
    fn parse_never_loops_on_garbage() {
        for src in [
            "}}}",
            "{ { {",
            "model",
            "service S at",
            "@",
            "@@@",
            "use",
            "enum E {",
            "union U { a: }",
            "get get get",
            "-> |",
            "\"abc",
        ] {
            let _ = p(src);
        }
    }

    #[test]
    fn literals_cover_every_kind() {
        let r = p(
            "model A { a: int = 1\n b: float = -2.5\n c: bool = true\n d: json = null\n e: S = .x\n f: int[] = []\n g: string = \"s\" }",
        );
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    }
}

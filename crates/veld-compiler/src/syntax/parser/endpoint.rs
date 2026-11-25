//! The block of endpoint rules (`spec/rules.md` section 9): `fact`, `transition`, `require` and `ensure`.
//!
//! `allow` belongs to a later phase. It is recognised only to say so, and the rest of the
//! statement is skipped. `transition` is parsed in `machine.rs`.

use super::Parser;
use crate::diagnostics::Span;
use crate::syntax::ast::{
    Doc, Else, EndpointBlock, EnsureStmt, FactStmt, Ident, PayloadField, RequireStmt, Stmt, StrLit,
};
use crate::syntax::lexer::Tok;

const STATEMENT_WORDS: [&str; 5] = ["fact", "require", "ensure", "allow", "transition"];

impl Parser<'_> {
    /// `{ stmt* }` at the cursor (the `{` is the current token).
    pub(super) fn parse_block(&mut self) -> Option<EndpointBlock> {
        let open = self.advance().span;
        let mut stmts = Vec::new();
        let close = loop {
            if self.at_eof() {
                self.unexpected("`}` to close the endpoint block");
                break self.span();
            }
            let before = self.pos;
            let docs = self.docs();
            if matches!(self.peek(), Tok::RBrace) {
                self.warn_dangling_docs_if_any(&docs, true);
                break self.advance().span;
            }
            if let Some(s) = self.parse_stmt(docs) {
                stmts.push(s);
            }
            if self.pos == before {
                self.advance();
            }
        };
        Some(EndpointBlock {
            stmts,
            span: open.to(close),
        })
    }

    fn parse_stmt(&mut self, docs: Vec<Doc>) -> Option<Stmt> {
        let word = match self.peek() {
            Tok::Ident(w) => w.clone(),
            _ => String::new(),
        };
        let parsed = match word.as_str() {
            "fact" => self.fact_stmt(&docs),
            "require" => self.require_stmt(docs),
            "ensure" => self.ensure_stmt(docs),
            "allow" => return self.later_phase("allow", "policies (phase 4)"),
            "transition" => self.transition_stmt(&docs),
            _ => {
                self.unexpected("a statement (`fact`, `transition`, `require` or `ensure`)");
                None
            }
        };
        if parsed.is_none() {
            self.skip_statement();
        }
        parsed
    }

    fn later_phase(&mut self, word: &str, phase: &str) -> Option<Stmt> {
        let span = self.advance().span;
        self.err(
            "E0003",
            format!("`{word}` belongs to {phase} and is not supported yet"),
            span,
        );
        self.skip_statement();
        None
    }

    /// Skips to the next statement word at the start of a line, or to the closing `}`.
    fn skip_statement(&mut self) {
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.peek() {
                Tok::RBrace if depth == 0 => return,
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => depth = (depth - 1).max(0),
                Tok::Ident(w)
                    if depth == 0
                        && STATEMENT_WORDS.contains(&w.as_str())
                        && self.newline_between(self.prev_end(), self.span().start) =>
                {
                    return;
                }
                Tok::Doc(_) if depth == 0 => return,
                _ => {}
            }
            self.advance();
        }
    }

    pub(super) fn end_from(&self, start: Span) -> Span {
        let end = Span {
            file: self.file,
            start: self.prev_end(),
            end: self.prev_end(),
        };
        start.to(end)
    }

    fn fact_stmt(&mut self, docs: &[Doc]) -> Option<Stmt> {
        let kw = self.advance().span;
        if let Some(d) = docs.first() {
            self.err(
                "E0003",
                "a doc comment can be attached to `require` and `ensure`, not to `fact`",
                d.span,
            );
        }
        let name = self.expect_ident("a fact name")?;
        let optional = self.eat(&Tok::Question);
        self.expect(&Tok::Colon, "`:` after the fact name")?;
        let ty = self.parse_type()?;
        let key = if self.eat(&Tok::Eq) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Some(Stmt::Fact(FactStmt {
            name,
            optional,
            ty,
            key,
            span: self.end_from(kw),
        }))
    }

    /// `name:` before an expression: an identifier followed by a colon.
    fn statement_name(&mut self) -> Option<Ident> {
        if matches!(self.peek(), Tok::Ident(_)) && matches!(self.tok_at(1), Tok::Colon) {
            let name = self.expect_ident("a statement name")?;
            self.advance();
            return Some(name);
        }
        None
    }

    fn message(&mut self) -> Option<Option<StrLit>> {
        if self.at_word("message") && matches!(self.tok_at(1), Tok::Str(_)) {
            self.advance();
            return self.expect_str("a message string").map(Some);
        }
        Some(None)
    }

    fn require_stmt(&mut self, docs: Vec<Doc>) -> Option<Stmt> {
        let kw = self.advance().span;
        let name = self.statement_name();
        let expr = self.parse_expr()?;
        let otherwise = if self.at_word("else") {
            self.advance();
            Some(self.parse_else()?)
        } else {
            None
        };
        let message = self.message()?;
        Some(Stmt::Require(RequireStmt {
            docs,
            name,
            expr,
            otherwise,
            message,
            span: self.end_from(kw),
        }))
    }

    fn parse_else(&mut self) -> Option<Else> {
        let error = self.parse_typeref()?;
        let payload = if matches!(self.peek(), Tok::LBrace) {
            Some(self.parse_payload()?)
        } else {
            None
        };
        Some(Else { error, payload })
    }

    fn parse_payload(&mut self) -> Option<Vec<PayloadField>> {
        self.advance();
        let mut fields = Vec::new();
        loop {
            while self.eat(&Tok::Comma) {}
            if matches!(self.peek(), Tok::RBrace) {
                self.advance();
                return Some(fields);
            }
            if self.at_eof() {
                self.unexpected("`}` to close the payload");
                return None;
            }
            let name = self.expect_ident("a field name")?;
            self.expect(&Tok::Colon, "`:` after the field name")?;
            let value = self.parse_expr()?;
            fields.push(PayloadField { name, value });
        }
    }

    fn ensure_stmt(&mut self, docs: Vec<Doc>) -> Option<Stmt> {
        let kw = self.advance().span;
        let name = self.statement_name();
        let expr = self.parse_expr()?;
        let message = self.message()?;
        Some(Stmt::Ensure(EnsureStmt {
            docs,
            name,
            expr,
            message,
            span: self.end_from(kw),
        }))
    }
}

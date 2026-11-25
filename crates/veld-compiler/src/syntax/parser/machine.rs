//! State machines (`spec/rules.md` section 10): `flow` and `final` in an enum body, and the
//! `transition` statement of an endpoint block.
//!
//! `flow` and `final` are words only where the lookahead of section 2.2 says so; everywhere else
//! they are variant names.

use super::Parser;
use crate::syntax::ast::{Doc, FinalStmt, FlowStmt, Ident, Stmt, TransitionStmt};
use crate::syntax::lexer::Tok;

impl Parser<'_> {
    /// `flow` IDENT `->`: a flow statement, not a variant named `flow`.
    pub(super) fn at_flow_stmt(&self) -> bool {
        self.at_word("flow")
            && matches!(self.tok_at(1), Tok::Ident(_))
            && matches!(self.tok_at(2), Tok::Arrow)
    }

    /// `final` IDENT on the same line: a final statement, not a variant named `final`.
    pub(super) fn at_final_stmt(&self) -> bool {
        self.at_word("final")
            && matches!(self.tok_at(1), Tok::Ident(_))
            && !self.newline_between(self.span_at(0).end, self.span_at(1).start)
    }

    /// `flow a -> b -> c`, with the cursor on `flow`.
    pub(super) fn parse_flow(&mut self) -> Option<FlowStmt> {
        let kw = self.advance().span;
        let mut states = vec![self.expect_ident("a state name")?];
        while self.eat(&Tok::Arrow) {
            states.push(self.expect_ident("a state name after `->`")?);
        }
        self.reject_emits();
        Some(FlowStmt {
            states,
            span: self.end_from(kw),
        })
    }

    /// `final a, b`, with the cursor on `final`.
    pub(super) fn parse_final(&mut self) -> Option<FinalStmt> {
        let kw = self.advance().span;
        let mut states = vec![self.expect_ident("a state name")?];
        while self.eat(&Tok::Comma) {
            if !matches!(self.peek(), Tok::Ident(_)) {
                break;
            }
            states.push(self.expect_ident("a state name")?);
        }
        Some(FinalStmt {
            states,
            span: self.end_from(kw),
        })
    }

    /// `flow a -> b emits Event` belongs to events: say so, and skip the rest of the line.
    fn reject_emits(&mut self) {
        let same_line = !self.newline_between(self.prev_end(), self.span().start);
        if !(self.at_word("emits") && same_line) {
            return;
        }
        let at = self.span();
        self.err(
            "E0003",
            "`emits` attaches an event to a flow, and events are not supported yet",
            at,
        );
        while !self.at_eof()
            && !matches!(self.peek(), Tok::RBrace)
            && !self.newline_between(self.prev_end(), self.span().start)
        {
            self.advance();
        }
    }

    /// `transition fact.path: a, b -> c`, with the cursor on `transition`.
    pub(super) fn transition_stmt(&mut self, docs: &[Doc]) -> Option<Stmt> {
        let kw = self.advance().span;
        if let Some(d) = docs.first() {
            self.err(
                "E0003",
                "a doc comment can be attached to `require` and `ensure`, not to `transition`",
                d.span,
            );
        }
        let mut path = vec![self.expect_ident("the name of a fact")?];
        while self.eat(&Tok::Dot) {
            path.push(self.expect_ident("a field name")?);
        }
        self.expect(&Tok::Colon, "`:` after the path of the transition")?;
        let mut from: Vec<Ident> = vec![self.expect_ident("a source state")?];
        while self.eat(&Tok::Comma) {
            from.push(self.expect_ident("a source state")?);
        }
        self.expect(&Tok::Arrow, "`->` before the target state")?;
        let to = self.expect_ident("the target state")?;
        Some(Stmt::Transition(TransitionStmt {
            path,
            from,
            to,
            span: self.end_from(kw),
        }))
    }
}

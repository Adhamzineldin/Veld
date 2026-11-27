//! Policies and the principal (`spec/rules.md` section 11): the top-level `policy` and
//! `principal` declarations and the `allow` statement of an endpoint block.
//!
//! `policy`, `permit`, `forbid`, `when`, `allow` and `principal` are contextual words. A
//! declaration starts with `policy IDENT (` or `principal IDENT`, so a model, a field or an
//! endpoint may still be called by any of them.

use super::Parser;
use crate::syntax::ast::{
    AllowStmt, Annotation, Doc, PolicyClause, PolicyDecl, PolicyParam, PrincipalDecl, Stmt,
};
use crate::syntax::lexer::Tok;

impl Parser<'_> {
    /// `policy` IDENT `(` at token `n`.
    pub(super) fn is_policy_start_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Ident(w) if w == "policy")
            && matches!(self.tok_at(n + 1), Tok::Ident(_))
            && matches!(self.tok_at(n + 2), Tok::LParen)
    }

    /// `principal` IDENT at token `n`.
    pub(super) fn is_principal_start_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Ident(w) if w == "principal")
            && matches!(self.tok_at(n + 1), Tok::Ident(_))
    }

    /// The start of an example, a policy, a principal, an event or a channel: the top-level
    /// items that a `model`-like keyword does not introduce.
    pub(super) fn is_rule_decl_start_at(&self, n: usize) -> bool {
        self.is_example_start_at(n)
            || self.is_policy_start_at(n)
            || self.is_principal_start_at(n)
            || self.is_event_item_start_at(n)
    }

    /// Reports the annotations written before `what`, which takes none: `@rollout` is `E0602`
    /// (it has a place, and this is not it), and the first other annotation is `E0003`.
    pub(super) fn no_annotations(&mut self, anns: &[Annotation], what: &str) {
        for a in anns.iter().filter(|a| a.name.name == "rollout") {
            self.rollout_misplaced(a, what);
        }
        if let Some(a) = anns.iter().find(|a| a.name.name != "rollout") {
            self.err("E0003", format!("{what} takes no annotations"), a.span);
        }
    }

    /// `E0602` for `@rollout` where it does not belong.
    pub(super) fn rollout_misplaced(&mut self, a: &Annotation, what: &str) {
        let msg = format!("`@rollout` cannot be applied to {what}");
        self.err("E0602", msg, a.span);
    }

    /// `policy Name(a: T, ..) { permit when e [message ".."] | forbid when e .. }`, with the
    /// cursor on `policy`.
    pub(super) fn parse_policy(&mut self, docs: Vec<Doc>) -> Option<PolicyDecl> {
        let kw = self.advance().span;
        let name = self.expect_ident("a policy name")?;
        self.expect(&Tok::LParen, "`(` to open the parameters of the policy")?;
        let mut params = Vec::new();
        loop {
            let pname = self.expect_ident("a parameter name")?;
            self.expect(&Tok::Colon, "`:` after the parameter name")?;
            let ty = self.parse_type()?;
            params.push(PolicyParam { name: pname, ty });
            if !self.eat(&Tok::Comma) || matches!(self.peek(), Tok::RParen) {
                break;
            }
        }
        self.expect(&Tok::RParen, "`)` to close the parameters of the policy")?;
        self.expect(&Tok::LBrace, "`{` to open the policy")?;
        let mut clauses = Vec::new();
        let close = loop {
            if self.at_eof() {
                self.unexpected("`}` to close the policy");
                break self.span();
            }
            let before = self.pos;
            let docs = self.docs();
            if matches!(self.peek(), Tok::RBrace) {
                self.warn_dangling_docs_if_any(&docs, true);
                break self.advance().span;
            }
            self.warn_dangling_docs_if_any(&docs, true);
            match self.parse_clause() {
                Some(c) => clauses.push(c),
                None => self.skip_clause(),
            }
            if self.pos == before {
                self.advance();
            }
        };
        Some(PolicyDecl {
            docs,
            name,
            params,
            clauses,
            span: kw.to(close),
        })
    }

    fn parse_clause(&mut self) -> Option<PolicyClause> {
        let forbid = match self.peek() {
            Tok::Ident(w) if w == "permit" => false,
            Tok::Ident(w) if w == "forbid" => true,
            _ => {
                self.unexpected("`permit` or `forbid`");
                return None;
            }
        };
        let kw = self.advance().span;
        if !self.eat_word("when") {
            self.unexpected("`when` after `permit` or `forbid`");
            return None;
        }
        let expr = self.parse_expr()?;
        let message = self.message()?;
        let flag = self.flag_suffix()?;
        Some(PolicyClause {
            forbid,
            expr,
            message,
            flag,
            span: self.end_from(kw),
        })
    }

    /// Skips to the next clause word at the start of a line, or to the closing `}`.
    fn skip_clause(&mut self) {
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.peek() {
                Tok::RBrace if depth == 0 => return,
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => depth = (depth - 1).max(0),
                Tok::Ident(w)
                    if depth == 0
                        && (w == "permit" || w == "forbid")
                        && self.newline_between(self.prev_end(), self.span().start) =>
                {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    /// `principal Type [as name]`, with the cursor on `principal`.
    pub(super) fn parse_principal(&mut self, docs: Vec<Doc>) -> Option<PrincipalDecl> {
        let kw = self.advance().span;
        let ty = self.parse_typeref()?;
        let same_line = !self.newline_between(self.prev_end(), self.span().start);
        let name = if self.at_word("as") && same_line {
            self.advance();
            Some(self.expect_ident("the name of the principal")?)
        } else {
            None
        };
        Some(PrincipalDecl {
            docs,
            ty,
            name,
            span: self.end_from(kw),
        })
    }

    /// `allow Policy(args)`, with the cursor on `allow`.
    pub(super) fn allow_stmt(&mut self, docs: &[Doc]) -> Option<Stmt> {
        let kw = self.advance().span;
        if let Some(d) = docs.first() {
            self.err(
                "E0003",
                "a doc comment can be attached to `require` and `ensure`, not to `allow`",
                d.span,
            );
        }
        let policy = self.expect_ident("the name of a policy")?;
        self.expect(&Tok::LParen, "`(` after the name of the policy")?;
        let mut args = Vec::new();
        loop {
            if matches!(self.peek(), Tok::RParen) {
                break;
            }
            args.push(self.parse_expr()?);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(&Tok::RParen, "`)` to close the arguments of the policy")?;
        self.reject_flag_suffix("`allow`");
        Some(Stmt::Allow(AllowStmt {
            policy,
            args,
            span: self.end_from(kw),
        }))
    }
}

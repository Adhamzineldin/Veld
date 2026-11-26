//! Sagas (`spec/platform.md` sections 17 and 18): the block of a `saga post` endpoint and the
//! lines of a saga example.
//!
//! `saga`, `step`, `compensate` and `timeout` are contextual. `saga` starts an endpoint only
//! when a method word and a string follow it, `step` starts a line of the block only before the
//! name of a step and `=`, and `undo`, `unavailable`, `compensated` and `stuck` are words only
//! in the lines of an example, so every one of them is still an ordinary name elsewhere.

use super::Parser;
use crate::diagnostics::Span;
use crate::language;
use crate::syntax::ast::{
    Annotation, SagaAnswerLine, SagaArgDecl, SagaCallDecl, SagaDecl, SagaExpectKind,
    SagaExpectLine, SagaFieldDecl, SagaGivenLine, SagaStepDecl, SagaTimeoutDecl, SagaValueDecl,
};
use crate::syntax::lexer::Tok;

/// The deepest nesting of `{ .. }` values in a call of a saga. Each level costs three containers
/// of IR JSON (`{"object": [{"value": ..}]}`), which keeps the IR readable by `serde_json`.
pub const MAX_SAGA_OBJECT_NESTING: usize = 8;

/// Words that start a statement of an endpoint block. A saga has none of them (`E1203`).
const RULE_STATEMENT_WORDS: [&str; 5] = ["fact", "require", "ensure", "allow", "transition"];

impl Parser<'_> {
    /// `saga` followed by a method word and a string at token `n`: the start of a saga endpoint.
    pub(super) fn is_saga_start_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Ident(w) if w == "saga")
            && matches!(self.tok_at(n + 1), Tok::Ident(m) if language::is_method_word(m))
            && matches!(self.tok_at(n + 2), Tok::Str(_))
    }

    /// The block after the head of a saga endpoint. `word` is the `saga` word.
    pub(super) fn parse_saga_block(&mut self, word: Span) -> Option<SagaDecl> {
        if !matches!(self.peek(), Tok::LBrace) {
            self.err(
                "E1203",
                "a saga needs a block with its steps and a `timeout`",
                word,
            );
            return Some(SagaDecl {
                word,
                open: word,
                steps: Vec::new(),
                timeout: None,
                span: word,
            });
        }
        let open = self.advance().span;
        let mut steps: Vec<SagaStepDecl> = Vec::new();
        let mut timeout: Option<SagaTimeoutDecl> = None;
        let close = loop {
            if self.at_eof() {
                self.unexpected("`}` to close the saga");
                break self.span();
            }
            let before = self.pos;
            let docs = self.docs();
            let anns = self.annotations_before();
            if matches!(self.peek(), Tok::RBrace) {
                self.warn_dangling_docs_if_any(&docs, true);
                self.dangling_saga_annotations(&anns);
                break self.advance().span;
            }
            self.warn_dangling_docs_if_any(&docs, true);
            self.no_annotations(&anns, "a saga line");
            self.saga_line(&mut steps, &mut timeout);
            if self.pos == before {
                self.advance();
            }
        };
        Some(SagaDecl {
            word,
            open,
            steps,
            timeout,
            span: word.to(open.to(close)),
        })
    }

    fn dangling_saga_annotations(&mut self, anns: &[Annotation]) {
        if let Some(a) = anns.first() {
            self.err("E0003", "annotation is not followed by a saga line", a.span);
        }
    }

    /// One line of the block: `step` or `timeout`.
    fn saga_line(&mut self, steps: &mut Vec<SagaStepDecl>, timeout: &mut Option<SagaTimeoutDecl>) {
        let word = match self.peek() {
            Tok::Ident(w) => w.clone(),
            _ => String::new(),
        };
        let at = self.span();
        let parsed = match word.as_str() {
            "step" => {
                if timeout.is_some() {
                    self.err("E1203", "a `step` cannot follow the `timeout`", at);
                }
                self.saga_step().map(|s| steps.push(s))
            }
            "timeout" => self.saga_timeout(timeout, at),
            w if RULE_STATEMENT_WORDS.contains(&w) => {
                self.err(
                    "E1203",
                    format!("a saga has no `{w}`: its endpoint body is its steps"),
                    at,
                );
                None
            }
            _ => {
                self.unexpected("`step` or `timeout`");
                None
            }
        };
        if parsed.is_none() {
            self.skip_saga_line();
        }
    }

    /// Skips to the next line that starts with a word of the block, or to the closing `}`.
    fn skip_saga_line(&mut self) {
        let mut depth = 0i32;
        let mut first = true;
        while !self.at_eof() {
            match self.peek() {
                Tok::RBrace if depth == 0 => return,
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => depth = (depth - 1).max(0),
                Tok::Ident(w)
                    if !first
                        && depth == 0
                        && (w == "step"
                            || w == "timeout"
                            || RULE_STATEMENT_WORDS.contains(&w.as_str()))
                        && self.newline_between(self.prev_end(), self.span().start) =>
                {
                    return;
                }
                _ => {}
            }
            first = false;
            self.advance();
        }
    }

    fn saga_timeout(&mut self, seen: &mut Option<SagaTimeoutDecl>, at: Span) -> Option<()> {
        let kw = self.advance().span;
        let expr = self.parse_expr()?;
        if seen.is_some() {
            self.err("E1203", "a saga has one `timeout`", at);
            return Some(());
        }
        *seen = Some(SagaTimeoutDecl {
            span: kw.to(expr.span),
            expr,
        });
        Some(())
    }

    /// `step name = call [compensate call]`.
    fn saga_step(&mut self) -> Option<SagaStepDecl> {
        let kw = self.advance().span;
        let name = self.expect_ident("the name of the step")?;
        self.expect(&Tok::Eq, "`=` after the name of the step")?;
        let call = self.saga_call()?;
        let compensate = if self.at_word("compensate") && matches!(self.tok_at(1), Tok::Ident(_)) {
            self.advance();
            Some(self.saga_call()?)
        } else {
            None
        };
        Some(SagaStepDecl {
            name,
            call,
            compensate,
            span: self.end_from(kw),
        })
    }

    /// `Service.endpoint(name: value, ..)`.
    fn saga_call(&mut self) -> Option<SagaCallDecl> {
        let service = self.expect_ident("the name of a service")?;
        self.expect(&Tok::Dot, "`.` and the name of an endpoint")?;
        let endpoint = self.expect_ident("the name of an endpoint")?;
        self.expect(&Tok::LParen, "`(` to open the arguments")?;
        let mut args = Vec::new();
        while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
            let name = self.expect_ident("the name of a parameter")?;
            self.expect(&Tok::Colon, "`:` after the name of the parameter")?;
            let value = self.saga_value(0)?;
            args.push(SagaArgDecl { name, value });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        let close = self.expect(&Tok::RParen, "`)` to close the arguments")?;
        Some(SagaCallDecl {
            span: service.span.to(close),
            service,
            endpoint,
            args,
        })
    }

    /// An expression, or `{ field: value, .. }` (`level` objects deep).
    fn saga_value(&mut self, level: usize) -> Option<SagaValueDecl> {
        if !matches!(self.peek(), Tok::LBrace) {
            return self.parse_expr().map(SagaValueDecl::Expr);
        }
        if level >= MAX_SAGA_OBJECT_NESTING {
            let span = self.span();
            self.err(
                "E0005",
                format!(
                    "nesting is too deep (the limit is {MAX_SAGA_OBJECT_NESTING} levels of `{{ }}`)"
                ),
                span,
            );
            return None;
        }
        let open = self.advance().span;
        let mut fields = Vec::new();
        loop {
            while self.eat(&Tok::Comma) {}
            if matches!(self.peek(), Tok::RBrace) {
                break;
            }
            if self.at_eof() {
                self.unexpected("`}` to close the object");
                return None;
            }
            let name = self.expect_ident("the name of a field")?;
            self.expect(&Tok::Colon, "`:` after the name of the field")?;
            let value = self.saga_value(level + 1)?;
            fields.push(SagaFieldDecl { name, value });
        }
        let close = self.advance().span;
        Some(SagaValueDecl::Object {
            fields,
            span: open.to(close),
        })
    }

    // ---- examples ----------------------------------------------------------------

    /// `given step name =` or `given undo name =`: a saga line of an example. `given step = v` is
    /// still a fact called `step`.
    pub(super) fn is_saga_given_start(&self) -> bool {
        self.at_word("given")
            && matches!(self.tok_at(1), Tok::Ident(w) if w == "step" || w == "undo")
            && matches!(self.tok_at(2), Tok::Ident(_))
            && matches!(self.tok_at(3), Tok::Eq)
    }

    pub(super) fn parse_saga_given(&mut self) -> Option<SagaGivenLine> {
        let kw = self.advance().span;
        let undo = matches!(self.peek(), Tok::Ident(w) if w == "undo");
        self.advance();
        let step = self.expect_ident("the name of a step")?;
        self.advance();
        let answer = self.saga_answer(undo)?;
        Some(SagaGivenLine {
            undo,
            step,
            answer,
            span: self.end_from(kw),
        })
    }

    /// `unavailable`, `ok [value]`, `Error [value]`, or (for a step) the value of its success.
    fn saga_answer(&mut self, undo: bool) -> Option<SagaAnswerLine> {
        let span = self.span();
        if self.at_word("unavailable") && !matches!(self.tok_at(1), Tok::Dot) {
            self.advance();
            return Some(SagaAnswerLine::Unavailable(span));
        }
        let names_outcome = matches!(self.peek(), Tok::Ident(w) if w != "true" && w != "false");
        if names_outcome || undo {
            return self.parse_outcome(true).map(SagaAnswerLine::Outcome);
        }
        self.parse_value().map(SagaAnswerLine::Value)
    }

    /// `expect compensated` or `expect stuck`, at the cursor. An endpoint called `expect` is
    /// followed by `(`.
    pub(super) fn is_saga_expect_start(&self) -> bool {
        self.at_word("expect")
            && matches!(self.tok_at(1), Tok::Ident(w) if w == "compensated" || w == "stuck")
    }

    pub(super) fn parse_saga_expect(&mut self) -> Option<SagaExpectLine> {
        let kw = self.advance().span;
        let stuck = matches!(self.peek(), Tok::Ident(w) if w == "stuck");
        self.advance();
        if stuck {
            return Some(SagaExpectLine {
                kind: SagaExpectKind::Stuck,
                span: self.end_from(kw),
            });
        }
        let mut steps = vec![self.expect_ident("the name of a step")?];
        while self.eat(&Tok::Comma) {
            steps.push(self.expect_ident("the name of a step")?);
        }
        Some(SagaExpectLine {
            kind: SagaExpectKind::Compensated(steps),
            span: self.end_from(kw),
        })
    }
}

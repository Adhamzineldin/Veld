//! Examples (`spec/rules.md` section 12): `example "name" { given.. [as v] [at ".."] call }`.
//!
//! `given`, `as`, `at`, `returns`, `ok` and `absent` are contextual: `given` starts a given only
//! when `IDENT =` follows, so an endpoint may still be called `given`.

use super::Parser;
use crate::diagnostics::Span;
use crate::syntax::ast::{
    Doc, Example, ExampleArgument, ExampleAs, ExampleAt, ExampleCall, Given, Ident, Outcome, Value,
    ValueField, ValueKind,
};
use crate::syntax::lexer::Tok;

/// The deepest nesting of an example value. An object costs three JSON containers per level
/// (`{"object": [{"value": ..}]}`), so the limit keeps the IR readable by `serde_json`.
pub const MAX_VALUE_NESTING: usize = 32;

impl Parser<'_> {
    /// `example` STRING at token `n`: the start of an example (`example` stays an ordinary name
    /// everywhere else).
    pub(super) fn is_example_start_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Ident(w) if w == "example")
            && matches!(self.tok_at(n + 1), Tok::Str(_))
    }

    pub(super) fn parse_example(&mut self, docs: Vec<Doc>) -> Option<Example> {
        let kw = self.advance().span;
        let name = self.expect_str("the name of the example")?;
        self.expect(&Tok::LBrace, "`{` to open the example")?;
        let mut givens = Vec::new();
        while self.is_given_start() {
            givens.push(self.parse_given()?);
        }
        let principal = if self.at_word("as") {
            let start = self.advance().span;
            let value = self.parse_value()?;
            Some(ExampleAs {
                span: start.to(value.span),
                value,
            })
        } else {
            None
        };
        let at = if self.at_word("at") {
            let start = self.advance().span;
            let time = self.expect_str("a time such as \"2026-03-01T09:00:00Z\"")?;
            Some(ExampleAt {
                span: start.to(time.span),
                time,
            })
        } else {
            None
        };
        let call = self.parse_call_line()?;
        let close = self.expect(&Tok::RBrace, "`}` to close the example")?;
        Some(Example {
            docs,
            name,
            givens,
            principal,
            at,
            call,
            span: kw.to(close),
        })
    }

    fn is_given_start(&self) -> bool {
        self.at_word("given")
            && matches!(self.tok_at(1), Tok::Ident(_))
            && matches!(self.tok_at(2), Tok::Eq)
    }

    fn parse_given(&mut self) -> Option<Given> {
        let kw = self.advance().span;
        let name = self.expect_ident("the name of a fact")?;
        self.advance();
        let absent = self.at_word("absent") && !matches!(self.tok_at(1), Tok::Dot);
        let value = if absent {
            self.advance();
            None
        } else {
            Some(self.parse_value()?)
        };
        Some(Given {
            name,
            value,
            span: self.end_from(kw),
        })
    }

    fn parse_call_line(&mut self) -> Option<ExampleCall> {
        let start = self.span();
        let (service, endpoint) = if matches!(self.tok_at(1), Tok::Dot) {
            let s = self.expect_ident("a service name")?;
            self.advance();
            (Some(s), self.expect_ident("an endpoint name")?)
        } else {
            (None, self.expect_ident("an endpoint name")?)
        };
        self.expect(&Tok::LParen, "`(` to open the arguments")?;
        let mut args = Vec::new();
        while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
            let name = self.expect_ident("a parameter name")?;
            self.expect(&Tok::Colon, "`:` after the parameter name")?;
            args.push(ExampleArgument {
                name,
                value: self.parse_value()?,
            });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(&Tok::RParen, "`)` to close the arguments")?;
        let returns = if self.at_word("returns") {
            self.advance();
            Some(self.parse_value()?)
        } else {
            None
        };
        self.expect(&Tok::Arrow, "`->` and the outcome of the call")?;
        let outcome = self.parse_outcome()?;
        Some(ExampleCall {
            service,
            endpoint,
            args,
            returns,
            outcome,
            span: self.end_from(start),
        })
    }

    fn parse_outcome(&mut self) -> Option<Outcome> {
        let start = self.span();
        let error = if self.at_word("ok") {
            self.advance();
            None
        } else {
            Some(self.parse_typeref()?)
        };
        let value = if self.starts_value() {
            Some(self.parse_value()?)
        } else {
            None
        };
        Some(Outcome {
            error,
            value,
            span: self.end_from(start),
        })
    }

    fn starts_value(&self) -> bool {
        matches!(
            self.peek(),
            Tok::Ident(_)
                | Tok::Int(_)
                | Tok::Float(_)
                | Tok::Str(_)
                | Tok::Dot
                | Tok::LBrace
                | Tok::LBracket
                | Tok::Minus
        )
    }

    fn enter_value(&mut self) -> bool {
        if self.depth + 1 > MAX_VALUE_NESTING {
            let span = self.span();
            self.err(
                "E0005",
                format!("nesting is too deep (the limit is {MAX_VALUE_NESTING} levels)"),
                span,
            );
            return false;
        }
        self.depth += 1;
        true
    }

    pub(super) fn parse_value(&mut self) -> Option<Value> {
        let base = self.depth;
        if !self.enter_value() {
            return None;
        }
        let parsed = self.value_inner();
        self.depth = base;
        parsed
    }

    fn value_inner(&mut self) -> Option<Value> {
        let span = self.span();
        match self.peek().clone() {
            Tok::Str(s) => self.scalar_value(span, ValueKind::Str(s)),
            Tok::Int(n) => self.scalar_value(span, ValueKind::Int(n)),
            Tok::Float(value) => {
                let text = self.source_text(span);
                self.scalar_value(span, ValueKind::Float { value, text })
            }
            Tok::Minus => self.negative_value(),
            Tok::Dot => {
                self.advance();
                let name = self.expect_ident("an enum variant name")?;
                Some(Value {
                    span: span.to(name.span),
                    kind: ValueKind::EnumLit(name),
                })
            }
            Tok::LBrace => self.object_value(),
            Tok::LBracket => self.list_value(),
            Tok::Ident(w) if w == "true" || w == "false" => {
                self.scalar_value(span, ValueKind::Bool(w == "true"))
            }
            Tok::Ident(w) if w == "null" => {
                self.err("E0003", "expected a value, found `null`", span);
                None
            }
            Tok::Ident(_) => self.path_value(),
            _ => {
                self.unexpected("a value");
                None
            }
        }
    }

    fn source_text(&self, span: Span) -> String {
        self.text
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .to_string()
    }

    fn scalar_value(&mut self, span: Span, kind: ValueKind) -> Option<Value> {
        self.advance();
        Some(Value { kind, span })
    }

    /// `-5` after a word: the lexer read an operator, the value is a negative literal.
    fn negative_value(&mut self) -> Option<Value> {
        let Some(lit) = self.parse_negative_literal() else {
            self.unexpected("a value");
            return None;
        };
        let kind = match lit.kind {
            crate::syntax::ast::ExprKind::Int(n) => ValueKind::Int(n),
            crate::syntax::ast::ExprKind::Float { value, text } => ValueKind::Float { value, text },
            _ => return None,
        };
        Some(Value {
            kind,
            span: lit.span,
        })
    }

    fn path_value(&mut self) -> Option<Value> {
        let first = self.expect_ident("a name")?;
        let mut span = first.span;
        let mut path = vec![first];
        while matches!(self.peek(), Tok::Dot) && matches!(self.tok_at(1), Tok::Ident(_)) {
            self.advance();
            let id = self.expect_ident("a field name")?;
            span = span.to(id.span);
            path.push(id);
        }
        Some(Value {
            kind: ValueKind::Path(path),
            span,
        })
    }

    fn object_value(&mut self) -> Option<Value> {
        let open = self.advance().span;
        let mut fields: Vec<ValueField> = Vec::new();
        loop {
            while self.eat(&Tok::Comma) {}
            if matches!(self.peek(), Tok::RBrace) {
                break;
            }
            if self.at_eof() {
                self.unexpected("`}` to close the object");
                return None;
            }
            let name: Ident = self.expect_ident("a field name")?;
            self.expect(&Tok::Colon, "`:` after the field name")?;
            fields.push(ValueField {
                name,
                value: self.parse_value()?,
            });
        }
        let close = self.advance().span;
        Some(Value {
            kind: ValueKind::Object(fields),
            span: open.to(close),
        })
    }

    fn list_value(&mut self) -> Option<Value> {
        let open = self.advance().span;
        let mut items = Vec::new();
        while !matches!(self.peek(), Tok::RBracket | Tok::Eof) {
            items.push(self.parse_value()?);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        let close = self.expect(&Tok::RBracket, "`]` to close the list")?;
        Some(Value {
            kind: ValueKind::List(items),
            span: open.to(close),
        })
    }
}

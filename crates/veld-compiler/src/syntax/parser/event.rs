//! Events (`spec/events.md` section 2): the `event` and `channel` declarations, the `publishes`
//! and `subscribes` lines of a service and the event form of `example`.
//!
//! `event`, `channel`, `carries`, `key`, `publishes`, `subscribes`, `on`, `called` and `rejected`
//! are contextual words. A declaration starts with `event IDENT` or `channel IDENT` where a
//! declaration may stand (error recovery, which scans other text, wants the `{` too), a
//! `publishes` line with `publishes IDENT`, and an event example with `given event IDENT`, so
//! any of them stays an ordinary name everywhere else.

use super::Parser;
use crate::diagnostics::Span;
use crate::syntax::ast::{
    Annotation, Carries, ChannelDecl, ChannelKeyLine, Doc, EventDecl, EventExample, Example,
    ExampleBody, ExpectKind, Expectation, OnLine, Publishes, StrLit, Subscribes,
};
use crate::syntax::lexer::Tok;

impl Parser<'_> {
    /// `event` IDENT `{` or `channel` IDENT `{` at token `n`. Error recovery looks for this
    /// form: it scans text that may be the inside of a body, where two names can follow each
    /// other (a field of type `event` before a field called `key`).
    pub(super) fn is_event_item_start_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Ident(w) if w == "event" || w == "channel")
            && matches!(self.tok_at(n + 1), Tok::Ident(_))
            && matches!(self.tok_at(n + 2), Tok::LBrace)
    }

    /// `event` IDENT or `channel` IDENT where a declaration may start (`spec/events.md`
    /// section 2: at declaration level the word starts a declaration when an identifier
    /// follows). What comes after the name is the declaration's business, so that
    /// `event A extends B` is reported as what it is.
    pub(super) fn is_event_decl_start(&self) -> bool {
        matches!(self.tok_at(0), Tok::Ident(w) if w == "event" || w == "channel")
            && matches!(self.tok_at(1), Tok::Ident(_))
    }

    /// `publishes` IDENT or `subscribes` IDENT at token `n`.
    pub(super) fn is_event_line_start_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Ident(w) if w == "publishes" || w == "subscribes")
            && matches!(self.tok_at(n + 1), Tok::Ident(_))
    }

    /// `publishes` or `subscribes` where a line of a service starts, with no channel name after
    /// it. No endpoint starts with either word, so it can only be an unfinished line.
    pub(super) fn is_bare_event_word(&self) -> bool {
        (self.at_word("publishes") || self.at_word("subscribes")) && !self.is_event_line_start_at(0)
    }

    /// Reports the missing channel name of an unfinished `publishes` or `subscribes` line and
    /// skips the rest of the line.
    pub(super) fn bare_event_word(&mut self) {
        let word = if self.at_word("publishes") {
            "publishes"
        } else {
            "subscribes"
        };
        self.advance();
        self.unexpected(&format!("the name of a channel after `{word}`"));
        self.skip_service_line();
    }

    /// `given event` IDENT at token `n`: the start of an event example.
    pub(super) fn is_event_example_start_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Ident(w) if w == "given")
            && matches!(self.tok_at(n + 1), Tok::Ident(w) if w == "event")
            && matches!(self.tok_at(n + 2), Tok::Ident(_))
    }

    /// One `publishes` or `subscribes` line of a service body; docs and annotations do not
    /// belong to it.
    pub(super) fn service_event_line(
        &mut self,
        docs: &[Doc],
        anns: &[Annotation],
        publishes: &mut Vec<Publishes>,
        subscribes: &mut Vec<Subscribes>,
    ) {
        self.warn_dangling_docs_if_any(docs, true);
        self.no_annotations(anns, "a `publishes` or `subscribes` line");
        let before = self.pos;
        if self.at_word("publishes") {
            match self.parse_publishes() {
                Some(p) => publishes.push(p),
                None => self.skip_service_line(),
            }
        } else {
            match self.parse_subscribes() {
                Some(s) => subscribes.push(s),
                None => self.skip_service_line(),
            }
        }
        if self.pos == before {
            self.advance();
        }
    }

    /// Skips to the next endpoint, event line or the closing `}` of the service.
    fn skip_service_line(&mut self) {
        let mut depth = 0i32;
        while !self.at_eof() {
            match self.peek() {
                Tok::RBrace if depth == 0 => return,
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => depth = (depth - 1).max(0),
                Tok::Doc(_) | Tok::At if depth == 0 => return,
                Tok::Ident(_)
                    if depth == 0
                        && (self.is_endpoint_start_at(0) || self.is_event_line_start_at(0)) =>
                {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    fn keyword_span(&self) -> Span {
        self.span_at(0)
    }

    /// `event Name { fields and rules }`, with the keyword already consumed.
    pub(super) fn parse_event(&mut self) -> Option<EventDecl> {
        let name = self.expect_ident("an event name")?;
        if self.at_word("extends") {
            let span = self.span();
            self.err(
                "E0003",
                "an event has no `extends`: write its fields in the event itself",
                span,
            );
            return None;
        }
        self.expect(&Tok::LBrace, "`{` to open the event body")?;
        let (fields, rules) = self.parse_members();
        Some(EventDecl {
            name,
            fields,
            rules,
        })
    }

    /// `channel Name { carries A, B  key f }`, with the keyword already consumed.
    pub(super) fn parse_channel(&mut self, kw: Span) -> Option<ChannelDecl> {
        let name = self.expect_ident("a channel name")?;
        self.expect(&Tok::LBrace, "`{` to open the channel body")?;
        let mut carries = Vec::new();
        let mut keys = Vec::new();
        let close = loop {
            if self.at_eof() {
                self.unexpected("`}` to close the channel");
                break self.keyword_span();
            }
            let before = self.pos;
            let docs = self.docs();
            self.warn_dangling_docs_if_any(&docs, true);
            if matches!(self.peek(), Tok::RBrace) {
                break self.advance().span;
            }
            let ok = if self.at_word("carries") {
                self.parse_carries().map(|c| carries.push(c))
            } else if self.at_word("key") {
                self.parse_key_line().map(|k| keys.push(k))
            } else {
                self.unexpected("`carries`, `key` or `}` in a channel");
                None
            };
            if ok.is_none() {
                self.skip_channel_line();
            }
            if self.pos == before {
                self.advance();
            }
        };
        Some(ChannelDecl {
            name,
            carries,
            keys,
            span: kw.to(close),
        })
    }

    fn parse_carries(&mut self) -> Option<Carries> {
        let kw = self.advance().span;
        let mut events = vec![self.parse_typeref()?];
        while self.eat(&Tok::Comma) {
            events.push(self.parse_typeref()?);
        }
        Some(Carries {
            events,
            span: self.end_from(kw),
        })
    }

    fn parse_key_line(&mut self) -> Option<ChannelKeyLine> {
        let kw = self.advance().span;
        let field = self.expect_ident("the name of the key field")?;
        Some(ChannelKeyLine {
            field,
            span: self.end_from(kw),
        })
    }

    /// Skips to the next `carries` or `key` at the start of a line, or to the closing `}`.
    fn skip_channel_line(&mut self) {
        while !self.at_eof() {
            match self.peek() {
                Tok::RBrace => return,
                Tok::Ident(w)
                    if (w == "carries" || w == "key")
                        && self.newline_between(self.prev_end(), self.span().start) =>
                {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    /// `publishes A, B`, with the cursor on `publishes`.
    pub(super) fn parse_publishes(&mut self) -> Option<Publishes> {
        let kw = self.advance().span;
        let mut channels = vec![self.parse_typeref()?];
        while self.eat(&Tok::Comma) {
            channels.push(self.parse_typeref()?);
        }
        Some(Publishes {
            channels,
            span: self.end_from(kw),
        })
    }

    /// `subscribes C as "group" { on E -> handler .. }`, with the cursor on `subscribes`.
    pub(super) fn parse_subscribes(&mut self) -> Option<Subscribes> {
        let kw = self.advance().span;
        let channel = self.parse_typeref()?;
        if !self.eat_word("as") {
            self.unexpected("`as` and the name of the consumer group");
            return None;
        }
        let group = self.expect_str("a string consumer group such as \"notifications\"")?;
        self.expect(&Tok::LBrace, "`{` to open the subscription")?;
        let mut handlers = Vec::new();
        let close = loop {
            if self.at_eof() {
                self.unexpected("`}` to close the subscription");
                break self.keyword_span();
            }
            let before = self.pos;
            let docs = self.docs();
            if matches!(self.peek(), Tok::RBrace) {
                self.warn_dangling_docs_if_any(&docs, true);
                break self.advance().span;
            }
            match self.parse_on_line(docs) {
                Some(h) => handlers.push(h),
                None => self.skip_on_line(),
            }
            if self.pos == before {
                self.advance();
            }
        };
        Some(Subscribes {
            channel,
            group,
            handlers,
            span: kw.to(close),
        })
    }

    fn parse_on_line(&mut self, docs: Vec<Doc>) -> Option<OnLine> {
        if !self.at_word("on") {
            self.unexpected("`on` or `}` in a subscription");
            return None;
        }
        let kw = self.advance().span;
        let event = self.parse_typeref()?;
        self.expect(&Tok::Arrow, "`->` and the name of the handler")?;
        let handler = self.expect_ident("a handler name")?;
        Some(OnLine {
            docs,
            event,
            handler,
            span: self.end_from(kw),
        })
    }

    fn skip_on_line(&mut self) {
        while !self.at_eof() {
            match self.peek() {
                Tok::RBrace => return,
                Tok::Ident(w)
                    if w == "on" && self.newline_between(self.prev_end(), self.span().start) =>
                {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    /// The rest of an event example after its `{`: the event, the expectations and the `}`.
    pub(super) fn event_example_rest(
        &mut self,
        docs: Vec<Doc>,
        name: StrLit,
        kw: Span,
    ) -> Option<Example> {
        let event = self.parse_event_example()?;
        let close = self.expect(&Tok::RBrace, "`}` to close the example")?;
        Some(Example {
            docs,
            name,
            givens: Vec::new(),
            principal: None,
            at: None,
            body: ExampleBody::Event(event),
            saga: None,
            span: kw.to(close),
        })
    }

    /// `given event E { .. } [on C] expect ..`, with the cursor on `given`.
    pub(super) fn parse_event_example(&mut self) -> Option<EventExample> {
        let kw = self.advance().span;
        self.advance();
        let event = self.parse_typeref()?;
        if !matches!(self.peek(), Tok::LBrace) {
            self.unexpected("`{` and the fields of the event");
            return None;
        }
        let value = self.parse_value()?;
        let channel = if self.at_word("on") {
            self.advance();
            Some(self.parse_typeref()?)
        } else {
            None
        };
        let mut expects = Vec::new();
        while self.at_word("expect") {
            expects.push(self.parse_expectation()?);
        }
        if expects.is_empty() {
            self.unexpected("an `expect` line");
            return None;
        }
        Some(EventExample {
            event,
            value,
            channel,
            expects,
            span: self.end_from(kw),
        })
    }

    fn parse_expectation(&mut self) -> Option<Expectation> {
        let kw = self.advance().span;
        if self.at_word("rejected") && !matches!(self.tok_at(1), Tok::Dot) {
            self.advance();
            return Some(Expectation {
                kind: ExpectKind::Rejected,
                span: self.end_from(kw),
            });
        }
        let service = self.expect_ident("a service name")?;
        self.expect(&Tok::Dot, "`.` and the name of a handler")?;
        let handler = self.expect_ident("a handler name")?;
        let not = self.eat_word("not");
        if !self.eat_word("called") {
            self.unexpected("`called`");
            return None;
        }
        Some(Expectation {
            kind: ExpectKind::Called {
                service,
                handler,
                not,
            },
            span: self.end_from(kw),
        })
    }
}

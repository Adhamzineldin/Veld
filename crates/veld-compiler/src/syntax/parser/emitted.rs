//! `expect emitted Event` and `expect not emitted` after the call of an example
//! (`spec/events.md` section 11.1).
//!
//! `expect`, `emitted` and `not` are contextual: the lines are read only where `expect` is
//! followed by `emitted`, or by `not emitted`, so a value that is a name stays a value.

use super::Parser;
use crate::syntax::ast::ExpectEmitted;
use crate::syntax::lexer::Tok;

impl Parser<'_> {
    /// `expect emitted` or `expect not emitted` at the cursor.
    pub(super) fn at_emitted_expect(&self) -> bool {
        let word = |n: usize, w: &str| matches!(self.tok_at(n), Tok::Ident(s) if s == w);
        word(0, "expect") && (word(1, "emitted") || (word(1, "not") && word(2, "emitted")))
    }

    /// Every `expect emitted` line that follows the outcome of a call.
    pub(super) fn parse_emitted_lines(&mut self) -> Option<Vec<ExpectEmitted>> {
        let mut lines = Vec::new();
        while self.at_emitted_expect() {
            let kw = self.advance().span;
            let not = self.eat_word("not");
            self.advance();
            let event = if not {
                None
            } else {
                Some(self.parse_typeref()?)
            };
            lines.push(ExpectEmitted {
                event,
                span: self.end_from(kw),
            });
        }
        Some(lines)
    }
}

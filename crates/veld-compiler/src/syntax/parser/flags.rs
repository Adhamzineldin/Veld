//! `when flag("name")` (`spec/platform.md` section 11): a suffix of one statement.
//!
//! `when` and `flag` are words only in the shape `when flag (`; a field named `when` is followed
//! by `:` and stays a field. A bad name, and a suffix on a statement that cannot have one, is
//! `E0937`.

use super::Parser;
use crate::syntax::ast::FlagSuffix;
use crate::syntax::lexer::Tok;

/// The longest flag name.
pub const MAX_FLAG_NAME: usize = 64;

/// True when `name` is 1 to 64 characters of `A-Z a-z 0-9 _ . -` that start with a letter.
#[must_use]
pub fn is_valid_flag_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && name.chars().count() <= MAX_FLAG_NAME
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

impl Parser<'_> {
    /// `when flag (` at the cursor.
    pub(super) fn at_flag_suffix(&self) -> bool {
        self.at_word("when")
            && matches!(self.tok_at(1), Tok::Ident(w) if w == "flag")
            && matches!(self.tok_at(2), Tok::LParen)
    }

    /// The suffix, when there is one. A suffix with a bad name is reported and left out.
    pub(super) fn flag_suffix(&mut self) -> Option<Option<FlagSuffix>> {
        if !self.at_flag_suffix() {
            return Some(None);
        }
        let start = self.advance().span;
        self.advance();
        self.advance();
        let name = self.expect_str("the name of the flag, in quotes")?;
        let close = self.expect(&Tok::RParen, "`)` to close `flag(`")?;
        let span = start.to(close);
        if !is_valid_flag_name(&name.value) {
            self.err(
                "E0937",
                format!(
                    "`{}` is not a flag name: use 1 to {MAX_FLAG_NAME} characters of A-Z a-z 0-9 _ . -, starting with a letter",
                    name.value
                ),
                name.span,
            );
            return Some(None);
        }
        Some(Some(FlagSuffix {
            name: name.value,
            span,
        }))
    }

    /// A suffix after a statement that cannot have one: reported and skipped.
    pub(super) fn reject_flag_suffix(&mut self, what: &str) {
        let at = self.span();
        if self.at_flag_suffix() && self.flag_suffix().is_some() {
            self.err("E0937", format!("`when flag(..)` cannot follow {what}"), at);
        }
    }
}

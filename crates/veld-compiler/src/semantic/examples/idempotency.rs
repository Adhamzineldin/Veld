//! The `ifMatch` and `key` lines of an example (`spec/rules.md` section 11.9). Every problem is
//! `E0952`: the line does not fit the endpoint the example calls.

use super::values::Values;
use super::{RKey, RKeyStore, ROutcome};
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::REndpoint;
use crate::syntax::ast::{self, KeyStoreLine};
use veld_ir::ExampleIfMatch;

/// The longest idempotency key a server accepts (`spec/rules.md` section 11.9).
pub const MAX_KEY_LEN: usize = 255;

/// True for a key a request can carry: 1 to [`MAX_KEY_LEN`] visible ASCII characters.
#[must_use]
pub fn is_valid_key(key: &str) -> bool {
    (1..=MAX_KEY_LEN).contains(&key.len()) && key.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

impl Values<'_, '_, '_> {
    fn line_error(&mut self, msg: String, span: Span, help: &str) {
        self.diags
            .push(Diagnostic::error("E0952", msg, span).with_help(help));
    }

    /// `ifMatch 3` or `ifMatch any`: the endpoint must have a version guard.
    pub fn if_match(
        &mut self,
        line: Option<&ast::IfMatchLine>,
        ep: &REndpoint,
    ) -> Option<Option<ExampleIfMatch>> {
        let Some(line) = line else { return Some(None) };
        if ep.rules.version.is_none() {
            self.line_error(
                format!(
                    "endpoint `{}` has no version guard to send `If-Match` to",
                    ep.name
                ),
                line.span,
                "a guard needs a `fact` whose model has a `@version` field",
            );
            return None;
        }
        Some(Some(match line.version {
            Some(n) => ExampleIfMatch::Version(n.to_string()),
            None => ExampleIfMatch::Any,
        }))
    }

    /// `key "k"` and what the store holds: the endpoint must be `@idempotent`, the key must be one
    /// a request can carry, and a stored answer must be one the server stores (never a `5xx`).
    pub fn key(&mut self, line: Option<&ast::KeyLine>, ep: &REndpoint) -> Option<Option<RKey>> {
        let Some(line) = line else { return Some(None) };
        if ep.idempotent.is_none() {
            self.line_error(
                format!(
                    "endpoint `{}` is not `@idempotent`, so a key means nothing",
                    ep.name
                ),
                line.span,
                "write `@idempotent` before the endpoint",
            );
            return None;
        }
        if !is_valid_key(&line.key.value) {
            self.line_error(
                format!("`{}` is not an idempotency key", line.key.value),
                line.key.span,
                "a key is 1 to 255 visible ASCII characters",
            );
            return None;
        }
        let store = match &line.store {
            KeyStoreLine::New => RKeyStore::New,
            KeyStoreLine::Other => RKeyStore::Other,
            KeyStoreLine::Running => RKeyStore::Running,
            KeyStoreLine::Stored(o) => RKeyStore::Stored(self.stored(o, ep)?),
        };
        Some(Some(RKey {
            value: line.key.value.clone(),
            store,
        }))
    }

    /// The outcome after `stored`. A `5xx` answer is never stored, so it cannot be held.
    fn stored(&mut self, o: &ast::Outcome, ep: &REndpoint) -> Option<ROutcome> {
        let outcome = self.outcome_of(o, ep)?;
        if let ROutcome::Error(decl, _) = &outcome {
            let status = ep
                .errors
                .iter()
                .find(|(_, d)| d == decl)
                .map(|(s, _)| *s)
                .or(self.prog.decls[*decl].res.default_status);
            if status.is_some_and(|s| s >= 500) {
                self.line_error(
                    format!(
                        "`{}` answers with a 5xx status, which the store never holds",
                        self.prog.decls[*decl].name
                    ),
                    o.span,
                    "an answer of 500 or more is not stored: the next request runs again",
                );
                return None;
            }
        }
        Some(outcome)
    }
}

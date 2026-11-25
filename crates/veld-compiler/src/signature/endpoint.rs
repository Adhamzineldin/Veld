//! Identity of endpoint rules, exactly `spec/rules.md` section 9.6.
//!
//! ```text
//! rule_id    = "veld:" package "/" Service "." endpoint "." name "@" hex16( sha256( rule_bytes ) )
//! rule_bytes = RFC 8785 JSON of { "d": "<qualified service>.<endpoint>", "k": "require" | "ensure",
//!                                 "n": name, "e": ExprShape }
//! ```
//!
//! `name` is the statement's name, or `require` or `ensure` when it has none. The `else` error,
//! its payload and the message are not part of the id. In `ExprShape` a parameter is
//! `{ "param": name }` and a fact is `{ "fact": name }`, both by name.

use std::collections::BTreeSet;

use veld_ir::rules::Expr;
use veld_ir::{Decl, Ir, ServiceId};

use super::rules::shape;
use super::{Json, canonical, cmp_utf16, hex16, obj, text};

/// The endpoint a statement belongs to.
#[derive(Debug, Clone, Copy)]
pub struct EndpointRef<'a> {
    pub package: &'a str,
    /// Dotted module path of the file that declares the service.
    pub module: &'a str,
    pub service: &'a str,
    pub endpoint: &'a str,
}

/// `require` or `ensure`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind {
    Require,
    Ensure,
}

impl StatementKind {
    fn word(self) -> &'static str {
        match self {
            StatementKind::Require => "require",
            StatementKind::Ensure => "ensure",
        }
    }
}

/// The `rule_bytes` of `spec/rules.md` section 9.6: the canonical JSON that the id hashes.
#[must_use]
pub fn endpoint_rule_bytes(
    decls: &[Decl],
    at: &EndpointRef<'_>,
    kind: StatementKind,
    name: Option<&str>,
    expr: &Expr,
) -> String {
    let qualified = format!(
        "{}:{}.{}.{}",
        at.package, at.module, at.service, at.endpoint
    );
    canonical(&obj(vec![
        ("d", text(&qualified)),
        ("k", text(kind.word())),
        ("n", text(name.unwrap_or(kind.word()))),
        ("e", shape(expr, decls, &mut Vec::new())),
    ]))
}

/// The id of a `require` or `ensure` statement.
#[must_use]
pub fn endpoint_rule_id(
    decls: &[Decl],
    at: &EndpointRef<'_>,
    kind: StatementKind,
    name: Option<&str>,
    expr: &Expr,
) -> String {
    let bytes = endpoint_rule_bytes(decls, at, kind, name, expr);
    format!(
        "veld:{}/{}.{}.{}@{}",
        at.package,
        at.service,
        at.endpoint,
        name.unwrap_or(kind.word()),
        hex16(bytes.as_bytes())
    )
}

/// What a `transition` statement is identified by (`spec/rules.md` section 10.2). Every name is a
/// wire value or a wire name, never a source name.
#[derive(Debug, Clone, Copy)]
pub struct TransitionRef<'a> {
    pub fact: &'a str,
    /// The wire names of the path below the fact.
    pub path: &'a [String],
    /// The wire values of the sources, in any order.
    pub from: &'a [String],
    pub to: &'a str,
}

/// The canonical JSON that the id of a `transition` hashes.
#[must_use]
pub fn transition_rule_bytes(at: &EndpointRef<'_>, t: &TransitionRef<'_>) -> String {
    let qualified = format!(
        "{}:{}.{}.{}",
        at.package, at.module, at.service, at.endpoint
    );
    let mut sources: Vec<&String> = t.from.iter().collect();
    sources.sort_by(|a, b| cmp_utf16(a, b));
    sources.dedup();
    canonical(&obj(vec![
        ("d", text(&qualified)),
        ("k", text("transition")),
        ("f", text(t.fact)),
        ("p", Json::Arr(t.path.iter().map(|p| text(p)).collect())),
        (
            "s",
            Json::Arr(sources.into_iter().map(|s| text(s)).collect()),
        ),
        ("t", text(t.to)),
    ]))
}

/// The id of a `transition`: `veld:<package>/<Service>.<endpoint>.transition@<hex16>`.
#[must_use]
pub fn transition_rule_id(at: &EndpointRef<'_>, t: &TransitionRef<'_>) -> String {
    format!(
        "veld:{}/{}.{}.transition@{}",
        at.package,
        at.service,
        at.endpoint,
        hex16(transition_rule_bytes(at, t).as_bytes())
    )
}

/// The rules digest of a service (`spec/rules.md` section 9.7): the first 16 hex characters of
/// the SHA-256 of the RFC 8785 JSON array of the sorted, distinct rule ids of every invariant of
/// every declaration its endpoints reach. `None` when it reaches no invariant.
///
/// What an endpoint reaches is what `Ir::reachable_decls` says: parameters, the success value
/// and the errors, with their fields, parents and union arms. Facts are values of the
/// application that a client never sees, so they are not part of it.
#[must_use]
pub fn rules_digest(ir: &Ir, service: ServiceId) -> Option<String> {
    let ids: BTreeSet<&str> = ir
        .reachable_decls(&[service])
        .into_iter()
        .flat_map(|d| ir.decl(d).rules())
        .map(|r| r.id.as_str())
        .collect();
    if ids.is_empty() {
        return None;
    }
    let list = Json::Arr(ids.into_iter().map(text).collect());
    Some(hex16(canonical(&list).as_bytes()))
}

/// Sets `rules_digest` on every service of `ir`.
pub fn fill_rules_digests(ir: &mut Ir) {
    let digests: Vec<Option<String>> = ir.services.iter().map(|s| rules_digest(ir, s.id)).collect();
    for (s, d) in ir.services.iter_mut().zip(digests) {
        s.rules_digest = d;
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod digest_tests;

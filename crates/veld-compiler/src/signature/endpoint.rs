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

use veld_ir::Decl;
use veld_ir::rules::Expr;

use super::rules::shape;
use super::{canonical, hex16, obj, text};

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

#[cfg(test)]
mod tests;

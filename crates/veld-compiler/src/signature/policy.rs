//! Identity of a policy, exactly `spec/rules.md` section 11.2.
//!
//! ```text
//! policy_id    = "veld:" package "/" Policy "@" hex16( sha256( policy_bytes ) )
//! policy_bytes = RFC 8785 JSON of { "d": "<qualified name>",
//!                                   "c": [ { "k": "permit" | "forbid", "e": ExprShape } .. ] }
//! ```
//!
//! The clauses are in source order and a parameter is `{ "param": name }`. Messages are not part
//! of the identity.

use veld_ir::rules::Expr;
use veld_ir::{Decl, PolicyEffect};

use super::rules::shape;
use super::{Json, canonical, hex16, obj, text};

/// The canonical JSON that the id of a policy hashes.
#[must_use]
pub fn policy_bytes(decls: &[Decl], qualified: &str, clauses: &[(PolicyEffect, &Expr)]) -> String {
    let c = clauses
        .iter()
        .map(|(effect, expr)| {
            let k = match effect {
                PolicyEffect::Permit => "permit",
                PolicyEffect::Forbid => "forbid",
            };
            obj(vec![
                ("k", text(k)),
                ("e", shape(expr, decls, &mut Vec::new())),
            ])
        })
        .collect();
    canonical(&obj(vec![("d", text(qualified)), ("c", Json::Arr(c))]))
}

/// The id of a policy: `veld:<package>/<Policy>@<hex16>`.
#[must_use]
pub fn policy_id(
    decls: &[Decl],
    package: &str,
    name: &str,
    qualified: &str,
    clauses: &[(PolicyEffect, &Expr)],
) -> String {
    format!(
        "veld:{package}/{name}@{}",
        hex16(policy_bytes(decls, qualified, clauses).as_bytes())
    )
}

#[cfg(test)]
mod tests;

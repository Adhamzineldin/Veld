//! The rules whose expression changed, as pairs of old and new rule, for a solver to judge
//! (`veld diff --prove`, `spec/rules.md` section 18).
//!
//! [`diff`](super::diff) says "possibly breaking" for each of these: it cannot tell whether the new
//! rule is tighter. This module only finds the pairs; it never judges them.

use veld_ir::{Decl, Endpoint, Ir, Rule, Service};

use super::ChangeKind;
use super::decls::by_name;
use super::endpoint_rules::{enforced as enforced_stmts, label, pair};
use super::endpoints::{index, subject};
use super::expr_text::expr_text;
use super::rules::{Position, enforced, own_rules};
use super::usage::Usage;

/// A rule of one contract: the contract, what carries the rule, and the rule.
pub type InvariantSide<'a> = (&'a Ir, &'a Decl, &'a Rule);

/// A `require` of one contract: the contract, the service, the endpoint and the index of the
/// line among the `require` lines of the endpoint.
pub type RequireSide<'a> = (&'a Ir, &'a Service, &'a Endpoint, usize);

/// The two versions of a changed rule.
#[derive(Debug, Clone, Copy)]
pub enum RuleSite<'a> {
    Invariant {
        old: InvariantSide<'a>,
        new: InvariantSide<'a>,
    },
    Require {
        old: RequireSide<'a>,
        new: RequireSide<'a>,
    },
}

/// A rule whose expression changed (its id differs).
#[derive(Debug, Clone, Copy)]
pub struct ChangedRule<'a> {
    /// The kind of the change `diff` reports for it.
    pub kind: ChangeKind,
    /// The subject of that change.
    pub subject: &'a str,
    pub site: RuleSite<'a>,
    /// Where the rule is checked: on requests, on responses, or both.
    pub request: bool,
    pub response: bool,
}

impl ChangedRule<'_> {
    /// The words a `diff` message of this change starts with after `possibly breaking: `.
    #[must_use]
    pub fn marker(&self) -> String {
        match self.site {
            RuleSite::Invariant { new, .. } => format!("rule `{}` changed", new.2.name),
            RuleSite::Require { new, .. } => {
                let r = &new.2.rules.requires[new.3];
                format!(
                    "{} changed",
                    label("require", r.name.as_deref(), &expr_text(&r.expr))
                )
            }
        }
    }

    /// The rule as the report names it.
    #[must_use]
    pub fn name(&self) -> String {
        match self.site {
            RuleSite::Invariant { new, .. } => format!("rule `{}`", new.2.name),
            RuleSite::Require { new, .. } => {
                let r = &new.2.rules.requires[new.3];
                label("require", r.name.as_deref(), &expr_text(&r.expr))
            }
        }
    }
}

/// Every enforced rule that exists in both contracts under one name and has a different id.
#[must_use]
pub fn changed_rules<'a>(old: &'a Ir, new: &'a Ir) -> Vec<ChangedRule<'a>> {
    let mut out = changed_invariants(old, new);
    out.extend(changed_requires(old, new));
    out
}

fn changed_invariants<'a>(old: &'a Ir, new: &'a Ir) -> Vec<ChangedRule<'a>> {
    let usage = Usage::of(old, new);
    let (olds, news) = (by_name(old), by_name(new));
    let mut out = Vec::new();
    for (q, o) in &olds {
        let Some(n) = news.get(q) else { continue };
        let pos = Position::of(usage.role(q));
        let (a, b) = (enforced(&own_rules(o)), enforced(&own_rules(n)));
        for (name, ro) in &a {
            let Some(rn) = b.get(name) else { continue };
            if ro.id != rn.id {
                out.push(ChangedRule {
                    kind: ChangeKind::InvariantChanged,
                    subject: n.qualified.as_str(),
                    site: RuleSite::Invariant {
                        old: (old, o, ro),
                        new: (new, n, rn),
                    },
                    request: pos.request,
                    response: pos.response,
                });
            }
        }
    }
    out
}

fn changed_requires<'a>(old: &'a Ir, new: &'a Ir) -> Vec<ChangedRule<'a>> {
    let (olds, news) = (index(old), index(new));
    let mut out = Vec::new();
    for (key, (os, o)) in &olds {
        let Some((ns, n)) = news.get(key) else {
            continue;
        };
        let p = pair(
            &enforced_stmts(&o.rules.requires),
            &enforced_stmts(&n.rules.requires),
        );
        for (a, b) in p.both.iter().filter(|(a, b)| a.id != b.id) {
            let at = |ep: &Endpoint, r: &veld_ir::EndpointRequire| {
                ep.rules.requires.iter().position(|x| std::ptr::eq(x, r))
            };
            let (Some(i), Some(j)) = (at(o, a), at(n, b)) else {
                continue;
            };
            out.push(ChangedRule {
                kind: ChangeKind::RequireChanged,
                subject: subject(o),
                site: RuleSite::Require {
                    old: (old, os, o, i),
                    new: (new, ns, n, j),
                },
                request: true,
                response: false,
            });
        }
    }
    out
}

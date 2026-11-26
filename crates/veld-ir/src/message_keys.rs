//! Message keys (`spec/platform.md` section 22).
//!
//! A catalog maps a **message key** to a text. The key of a rule is its id without `veld:` in front
//! and without `@<hex16>` behind, so it stays the same when the expression changes. The
//! functions here are the only place the keys are written down: `veld-compiler::signature`
//! re-exports them next to the ids, and a generator or a tool calls them from here (a generator
//! never depends on the compiler).
//!
//! | What | Key |
//! |---|---|
//! | an invariant of a declaration | `<package>/<Decl>.<rule>` |
//! | a named `require` or `ensure` | `<package>/<Service>.<endpoint>.<name>` |
//! | an unnamed one | `<package>/<Service>.<endpoint>.require` or `.ensure` (shared by all of them) |
//! | the `n`th `forbid` of a policy (1 is the first forbid, with or without a message) | `<package>/<Policy>.forbid<n>` |
//! | a refused `transition` | `veld/transition` |
//! | a `transition` before its time (platform P2) | `veld/transition-not-due` |
//! | a refused version guard | `veld/version` |
//! | a denial that no `forbid` explains | `veld/not-permitted` |

use std::collections::{BTreeMap, BTreeSet};

use crate::{Ir, PackageId, PolicyEffect};

/// The text of a `transition` that is not a legal move.
pub const TRANSITION: &str = "veld/transition";
/// The text of a timed move before its time.
pub const TRANSITION_NOT_DUE: &str = "veld/transition-not-due";
/// The text of a refused `If-Match`.
pub const VERSION: &str = "veld/version";
/// The text of a denial that no `forbid` message explains.
pub const NOT_PERMITTED: &str = "veld/not-permitted";
/// The four fixed keys, in the order of the table above.
pub const FIXED: [&str; 4] = [TRANSITION, TRANSITION_NOT_DUE, VERSION, NOT_PERMITTED];

/// The key of a rule whose id is `veld:<package>/<rest>@<hex16>`. `None` for a string of another
/// shape.
#[must_use]
pub fn rule_key(rule_id: &str) -> Option<String> {
    let body = rule_id.strip_prefix("veld:")?;
    let (key, hash) = body.rsplit_once('@')?;
    let well_formed = hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit());
    (well_formed && key.contains('/')).then(|| key.to_owned())
}

/// The key of the `forbid`th forbid clause (counted from 1) of the policy with id `policy_id`.
#[must_use]
pub fn forbid_key(policy_id: &str, forbid: usize) -> Option<String> {
    Some(format!("{}.forbid{forbid}", rule_key(policy_id)?))
}

/// What a catalog of one member can translate: key, and the contract's text when the rule has one.
pub type CatalogKeys = BTreeMap<String, Option<String>>;

impl Ir {
    /// The key a violation with the rule id `rule_id` carries as `messageKey`. `forbid` is the
    /// number (from 1) of the forbid clause that denied, for a denial by a policy that has a
    /// message; `None` for a denial no `forbid` explains.
    ///
    /// A refused `transition` and a refused version guard have the fixed keys, not the key of their
    /// id: their text is not the author's.
    #[must_use]
    pub fn violation_key(&self, rule_id: &str, forbid: Option<usize>) -> Option<String> {
        let own = |f: &dyn Fn(&crate::EndpointRules) -> Option<&str>| {
            self.services
                .iter()
                .flat_map(|s| s.endpoints.iter())
                .any(|e| f(&e.rules) == Some(rule_id))
        };
        if own(&|r| r.transition.as_ref().map(|t| t.id.as_str())) {
            return Some(TRANSITION.to_owned());
        }
        if own(&|r| r.version.as_ref().map(|v| v.id.as_str())) {
            return Some(VERSION.to_owned());
        }
        if self.policies.iter().any(|p| p.id == rule_id) {
            return Some(match forbid {
                Some(n) => forbid_key(rule_id, n)?,
                None => NOT_PERMITTED.to_owned(),
            });
        }
        rule_key(rule_id)
    }

    /// The translatable rules of the member `package`, by key, with the contract's message.
    ///
    /// A rule belongs to the member when its declaration is reachable from a service of the
    /// package or from an event it publishes or subscribes to, or is declared in the package; a
    /// `require` or `ensure` when its endpoint is; a `forbid` when its policy is declared in the
    /// package or an `allow` of the member names it. The fixed keys are not in the map: they are
    /// valid for every member and need no translation.
    #[must_use]
    pub fn catalog_keys(&self, package: PackageId) -> CatalogKeys {
        let services: Vec<_> = self
            .services
            .iter()
            .filter(|s| s.package == package)
            .map(|s| s.id)
            .collect();
        let mut keys = CatalogKeys::new();
        let mut put = |key: Option<String>, text: Option<&String>| {
            if let Some(key) = key {
                let slot = keys.entry(key).or_insert(None);
                if slot.is_none() {
                    slot.clone_from(&text.cloned());
                }
            }
        };
        let mut decls = self.member_decls(&services);
        decls.extend(
            self.decls
                .iter()
                .filter(|d| d.package == package)
                .map(|d| d.id),
        );
        for id in decls {
            for r in self.decl(id).rules() {
                put(rule_key(&r.id), r.message.as_ref());
            }
        }
        let mut policies: BTreeSet<usize> = BTreeSet::new();
        for s in &services {
            for e in &self.service(*s).endpoints {
                for r in &e.rules.requires {
                    put(rule_key(&r.id), r.message.as_ref());
                }
                for r in &e.rules.ensures {
                    put(rule_key(&r.id), r.message.as_ref());
                }
                policies.extend(e.rules.allows.iter().map(|a| a.policy.index()));
            }
        }
        policies.extend(
            self.policies
                .iter()
                .enumerate()
                .filter(|(_, p)| p.package == package)
                .map(|(i, _)| i),
        );
        for i in policies {
            let p = &self.policies[i];
            let forbids = p
                .clauses
                .iter()
                .filter(|c| c.effect == PolicyEffect::Forbid);
            for (n, c) in forbids.enumerate() {
                if c.message.is_some() {
                    put(forbid_key(&p.id, n + 1), c.message.as_ref());
                }
            }
        }
        keys
    }
}

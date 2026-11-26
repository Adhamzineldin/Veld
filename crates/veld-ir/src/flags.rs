//! Which feature flags a contract uses and how many statements each one gates
//! (`spec/platform.md` section 11).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{DeclKind, Ir, PolicyEffect};

/// The statements one flag gates, by kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct FlagUsage {
    pub invariants: usize,
    pub requires: usize,
    pub ensures: usize,
    pub permits: usize,
    pub forbids: usize,
    pub flows: usize,
}

impl FlagUsage {
    /// How many statements the flag gates.
    #[must_use]
    pub fn total(&self) -> usize {
        self.invariants + self.requires + self.ensures + self.permits + self.forbids + self.flows
    }
}

impl Ir {
    /// Every flag a statement of the contract uses, by name, with what it gates. An inherited
    /// invariant is counted once, where it is written. Empty for a contract without flags.
    #[must_use]
    pub fn flag_usage(&self) -> BTreeMap<String, FlagUsage> {
        let mut out: BTreeMap<String, FlagUsage> = BTreeMap::new();
        let mut bump =
            |name: &str, f: fn(&mut FlagUsage)| f(out.entry(name.to_owned()).or_default());
        for d in &self.decls {
            for r in d.rules().iter().filter(|r| r.inherited_from.is_none()) {
                if let Some(f) = &r.flag {
                    bump(f, |u| u.invariants += 1);
                }
            }
            if let DeclKind::Enum(e) = &d.kind {
                for f in e.flows.iter().filter_map(|f| f.flag.as_deref()) {
                    bump(f, |u| u.flows += 1);
                }
            }
        }
        for ep in self.services.iter().flat_map(|s| &s.endpoints) {
            for f in ep.rules.requires.iter().filter_map(|r| r.flag.as_deref()) {
                bump(f, |u| u.requires += 1);
            }
            for f in ep.rules.ensures.iter().filter_map(|r| r.flag.as_deref()) {
                bump(f, |u| u.ensures += 1);
            }
        }
        for c in self.policies.iter().flat_map(|p| &p.clauses) {
            let Some(f) = c.flag.as_deref() else { continue };
            match c.effect {
                PolicyEffect::Permit => bump(f, |u| u.permits += 1),
                PolicyEffect::Forbid => bump(f, |u| u.forbids += 1),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_total_adds_every_kind() {
        let u = FlagUsage {
            invariants: 1,
            requires: 2,
            ensures: 3,
            permits: 4,
            forbids: 5,
            flows: 6,
        };
        assert_eq!(u.total(), 21);
        assert_eq!(FlagUsage::default().total(), 0);
    }
}

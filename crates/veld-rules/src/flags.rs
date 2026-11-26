//! Feature flags (`spec/platform.md` section 11).
//!
//! A statement behind `when flag("f")` exists only while `f` is on: **off means the statement is
//! not there**, not evaluated, not reported and, for a `flow`, not an edge. The reference
//! evaluator therefore takes the flags of a request as a [`FlagState`] and answers the question
//! once, up front: [`FlagState::resolve`] returns the contract as that request sees it, with
//! every statement that is off removed. A server reads each flag at most once per request and
//! uses that answer until the response is sent; the resolved contract is exactly that snapshot.
//!
//! When the provider fails, a statement that restricts applies and one that grants does not: an
//! invariant, a `require`, an `ensure` and a `forbid` count as on, a `permit` and a `flow` as off.

use std::collections::BTreeSet;

use veld_ir::{DeclKind, Ir, PolicyEffect};

/// What a request knows about the flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagState {
    /// The provider answered: these flags are on, every other one is off.
    On(BTreeSet<String>),
    /// The provider failed.
    ProviderFailed,
}

impl Default for FlagState {
    /// No flag is on.
    fn default() -> Self {
        FlagState::On(BTreeSet::new())
    }
}

/// What a statement does, which decides how it is read when the provider fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// An invariant, a `require`, an `ensure` or a `forbid`.
    Restricts,
    /// A `permit` or a `flow`.
    Grants,
}

impl FlagState {
    /// The flags in `names` are on.
    #[must_use]
    pub fn on<I, S>(names: I) -> FlagState
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        FlagState::On(names.into_iter().map(Into::into).collect())
    }

    /// Whether a statement with this flag exists.
    #[must_use]
    pub fn exists(&self, flag: Option<&str>, effect: Effect) -> bool {
        let Some(name) = flag else {
            return true;
        };
        match self {
            FlagState::On(set) => set.contains(name),
            FlagState::ProviderFailed => effect == Effect::Restricts,
        }
    }

    /// The contract as a request with these flags sees it: every statement that is off is gone,
    /// and the statements that stay are ordinary ones (their flag is cleared), so resolving the
    /// result again changes nothing. A contract that uses no flag is returned as it is.
    #[must_use]
    pub fn resolve(&self, ir: &Ir) -> Ir {
        let mut out = ir.clone();
        for d in &mut out.decls {
            match &mut d.kind {
                DeclKind::Model(m) => self.keep_rules(&mut m.rules),
                DeclKind::Error(e) => self.keep_rules(&mut e.fields.rules),
                DeclKind::Event(e) => self.keep_rules(&mut e.rules),
                DeclKind::Enum(e) => {
                    e.flows
                        .retain(|f| self.exists(f.flag.as_deref(), Effect::Grants));
                    e.flows.iter_mut().for_each(|f| f.flag = None);
                }
                DeclKind::Union(_) | DeclKind::Const(_) => {}
            }
        }
        for ep in out.services.iter_mut().flat_map(|s| &mut s.endpoints) {
            let r = &mut ep.rules;
            r.requires
                .retain(|x| self.exists(x.flag.as_deref(), Effect::Restricts));
            r.requires.iter_mut().for_each(|x| x.flag = None);
            r.ensures
                .retain(|x| self.exists(x.flag.as_deref(), Effect::Restricts));
            r.ensures.iter_mut().for_each(|x| x.flag = None);
        }
        for p in &mut out.policies {
            p.clauses.retain(|c| {
                let effect = match c.effect {
                    PolicyEffect::Permit => Effect::Grants,
                    PolicyEffect::Forbid => Effect::Restricts,
                };
                self.exists(c.flag.as_deref(), effect)
            });
            p.clauses.iter_mut().for_each(|c| c.flag = None);
        }
        out
    }

    fn keep_rules(&self, rules: &mut Vec<veld_ir::Rule>) {
        rules.retain(|r| self.exists(r.flag.as_deref(), Effect::Restricts));
        rules.iter_mut().for_each(|r| r.flag = None);
    }
}

/// The names of the flags a contract uses, sorted.
#[must_use]
pub fn flag_names(ir: &Ir) -> Vec<String> {
    ir.flag_usage().into_keys().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_statement_without_a_flag_always_exists() {
        for state in [FlagState::default(), FlagState::ProviderFailed] {
            assert!(state.exists(None, Effect::Restricts));
            assert!(state.exists(None, Effect::Grants));
        }
    }

    #[test]
    fn a_flag_that_is_on_keeps_its_statement_and_one_that_is_off_drops_it() {
        let state = FlagState::on(["a"]);
        assert!(state.exists(Some("a"), Effect::Restricts));
        assert!(!state.exists(Some("b"), Effect::Restricts));
        assert!(!state.exists(Some("b"), Effect::Grants));
    }

    #[test]
    fn a_failing_provider_restricts_and_does_not_grant() {
        let state = FlagState::ProviderFailed;
        assert!(state.exists(Some("a"), Effect::Restricts));
        assert!(!state.exists(Some("a"), Effect::Grants));
    }
}

//! The fourth question (`veld diff --prove`): is every value the old rule accepted still accepted
//! by the new one?
//!
//! The question asks for a value that the old rule accepts and the new rule rejects. If the
//! reference evaluator confirms one, the rule got *tighter*; if there is none, it is *not
//! tightened*, proved. A rule with an open part is *unsupported*: a claim about all values is not
//! sound when part of the rule is left open (`spec/rules.md` section 18.3).

use std::collections::BTreeSet;

use veld_ir::{Decl, Endpoint, EndpointRequire, Ir, Rule, Service, Type};
use veld_rules::Env;

use crate::confirm::{self, Verdict, judge};
use crate::problem::Problem;
use crate::question::{Check, Question, assume_param_invariants, endpoint_roots};
use crate::report::QuestionKind;
use crate::sample;
use crate::smt;
use crate::types::{EncodeError, OpenPart, RootKey, RootSpec, unsupported};
use crate::values::Concrete;

/// A rule that changed between two contracts.
#[derive(Debug, Clone, Copy)]
pub enum Changed<'a> {
    /// An invariant of a model or error.
    Invariant {
        old: (&'a Ir, &'a Decl, &'a Rule),
        new: (&'a Ir, &'a Decl, &'a Rule),
    },
    /// A `require` of an endpoint, by its index among the `require` lines.
    Require {
        old: (&'a Ir, &'a Service, &'a Endpoint, usize),
        new: (&'a Ir, &'a Service, &'a Endpoint, usize),
    },
}

/// What the evaluator is asked about a changed rule.
#[derive(Debug)]
pub struct TightenedCheck<'a> {
    changed: Changed<'a>,
}

fn require_at(ep: &Endpoint, index: usize) -> Result<&EndpointRequire, String> {
    ep.rules
        .requires
        .get(index)
        .ok_or_else(|| "a require that is not there".to_owned())
}

impl TightenedCheck<'_> {
    /// The old rule accepts the value and the new rule rejects it, for the evaluator.
    ///
    /// # Errors
    /// A text that says what the evaluator found instead.
    pub fn confirm(&self, c: &Concrete) -> Result<(), String> {
        let verdicts = match self.changed {
            Changed::Invariant { old, new } => invariant_verdicts(old, new, c)?,
            Changed::Require { old, new } => require_verdicts(old, new, c)?,
        };
        match verdicts {
            (Verdict::Holds, Verdict::Fails | Verdict::Errors) => Ok(()),
            (Verdict::Holds, Verdict::Holds) => {
                Err("the evaluator accepts it under both rules".into())
            }
            (Verdict::Fails | Verdict::Errors, _) => {
                Err("the evaluator rejects it under the old rule".into())
            }
        }
    }
}

type Pairs = (Verdict, Verdict);

fn invariant_verdicts(
    old: (&Ir, &Decl, &Rule),
    new: (&Ir, &Decl, &Rule),
    c: &Concrete,
) -> Result<Pairs, String> {
    let json = c
        .roots
        .get(&RootKey::SelfValue)
        .ok_or("no value was built")?;
    sample::check_decl(new.0, new.1, json, 0)?;
    let o = judge(&Env::for_value(old.0, json), &old.2.expr)?;
    let n = judge(&Env::for_value(new.0, json), &new.2.expr)?;
    Ok((o, n))
}

fn require_verdicts(
    old: (&Ir, &Service, &Endpoint, usize),
    new: (&Ir, &Service, &Endpoint, usize),
    c: &Concrete,
) -> Result<Pairs, String> {
    confirm::confirm_params(new.0, &new.2.params, c)?;
    let old_env = confirm::env_of(old.0, c);
    for r in old
        .2
        .rules
        .requires
        .iter()
        .take(old.3)
        .filter(|r| r.rollout.is_none())
    {
        if judge(&old_env, &r.expr)? != Verdict::Holds {
            return Err("the evaluator rejects an earlier require of the old endpoint".into());
        }
    }
    let o = judge(&old_env, &require_at(old.2, old.3)?.expr)?;
    let n = judge(&confirm::env_of(new.0, c), &require_at(new.2, new.3)?.expr)?;
    Ok((o, n))
}

fn blocking_open(open: &BTreeSet<OpenPart>) -> Option<String> {
    let names: Vec<&str> = open
        .iter()
        .filter(|p| **p != OpenPart::PatternFormat)
        .map(|p| p.name())
        .collect();
    (!names.is_empty()).then(|| {
        format!(
            "a rule uses a part of the fragment that is left open ({}), where a claim about every value is not sound",
            names.join(", ")
        )
    })
}

/// The two rules as one problem: old accepts, new rejects.
fn encode_changed<'a>(changed: Changed<'a>) -> Result<Problem<'a>, EncodeError> {
    match changed {
        Changed::Invariant { old, new } => {
            let root = |decl: &Decl| {
                std::collections::BTreeMap::from([(
                    RootKey::SelfValue,
                    RootSpec::validated(Type::Decl(decl.id)),
                )])
            };
            let mut p = Problem::new(new.0, root(new.1));
            let new_holds = p.holds(&new.2.expr)?;
            p.switch(old.0, root(old.1));
            let old_holds = p.holds(&old.2.expr)?;
            finish(p, &old_holds, &new_holds)
        }
        Changed::Require { old, new } => {
            let mut p = Problem::new(new.0, endpoint_roots(new.1, new.2));
            let new_holds = p.holds(
                &require_at(new.2, new.3)
                    .map_err(EncodeError::Unsupported)?
                    .expr,
            )?;
            p.switch(old.0, endpoint_roots(old.1, old.2));
            let old_holds = p.holds(
                &require_at(old.2, old.3)
                    .map_err(EncodeError::Unsupported)?
                    .expr,
            )?;
            let p = finish(p, &old_holds, &new_holds)?;
            assume_requests(p, old, new)
        }
    }
}

fn finish<'a>(
    mut p: Problem<'a>,
    old_holds: &str,
    new_holds: &str,
) -> Result<Problem<'a>, EncodeError> {
    if let Some(why) = blocking_open(p.open_parts()) {
        return unsupported(why);
    }
    p.assert_named("the old rule accepts the value", old_holds);
    p.assert_named("the new rule rejects the value", &smt::not(new_holds));
    Ok(p)
}

/// The value is a request of the new contract that got past the earlier `require` lines of the
/// old endpoint.
fn assume_requests<'a>(
    mut p: Problem<'a>,
    old: (&'a Ir, &'a Service, &'a Endpoint, usize),
    new: (&'a Ir, &'a Service, &'a Endpoint, usize),
) -> Result<Problem<'a>, EncodeError> {
    p.switch(new.0, endpoint_roots(new.1, new.2));
    assume_param_invariants(&mut p, new.0, &new.2.params)?;
    p.switch(old.0, endpoint_roots(old.1, old.2));
    for r in old
        .2
        .rules
        .requires
        .iter()
        .take(old.3)
        .filter(|r| r.rollout.is_none())
    {
        let holds = p.holds(&r.expr)?;
        p.assert_named("an earlier require of the old endpoint passes", &holds);
    }
    Ok(p)
}

/// The question for one changed rule.
#[must_use]
pub fn tightened_question<'a>(subject: &str, detail: &str, changed: Changed<'a>) -> Question<'a> {
    let origin = match changed {
        Changed::Invariant { new, .. } => new.2.origin.clone(),
        Changed::Require { new, .. } => require_at(new.2, new.3)
            .map(|r| r.origin.clone())
            .unwrap_or_default(),
    };
    let encoded = encode_changed(changed);
    Question {
        kind: QuestionKind::Tightened,
        subject: subject.to_owned(),
        detail: detail.to_owned(),
        origin,
        encoded,
        check: Check::Tightened(Box::new(TightenedCheck { changed })),
        core_origins: Vec::new(),
    }
}

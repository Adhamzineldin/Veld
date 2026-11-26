//! The three questions of `veld check --prove` (`spec/rules.md` section 18.1).

use std::collections::BTreeMap;

use veld_ir::{
    Decl, DeclKind, Endpoint, Ir, Origin, PackageKind, Param, Policy, PolicyEffect, Rule, Service,
    Type,
};

use crate::confirm;
use crate::diff::TightenedCheck;
use crate::problem::Problem;
use crate::report::QuestionKind;
use crate::smt;
use crate::types::{EncodeError, RootKey, RootSpec};
use crate::values::Concrete;

/// What the evaluator is asked once the solver has given a value.
#[derive(Debug)]
pub enum Check<'a> {
    Invariants(&'a Ir, &'a Decl),
    Require(&'a Ir, &'a Endpoint, usize),
    Policy(&'a Ir, &'a Policy),
    Tightened(Box<TightenedCheck<'a>>),
}

impl Check<'_> {
    /// Confirms a value: `Ok` when the evaluator agrees that it answers the question.
    ///
    /// # Errors
    /// A text that says what the evaluator found.
    pub fn confirm(&self, c: &Concrete) -> Result<(), String> {
        match self {
            Check::Invariants(ir, decl) => confirm::confirm_invariants(ir, decl, c),
            Check::Require(ir, ep, index) => confirm::confirm_require(ir, ep, *index, c),
            Check::Policy(ir, policy) => confirm::confirm_policy(ir, policy, c),
            Check::Tightened(t) => t.confirm(c),
        }
    }

    /// True when the value is shown with the reading of the clock.
    #[must_use]
    pub fn shows_clock(&self, problem: &Problem<'_>) -> bool {
        problem.open_parts().contains(&crate::types::OpenPart::Now)
    }
}

/// One question: its text, and how to confirm what the solver says.
#[derive(Debug)]
pub struct Question<'a> {
    pub kind: QuestionKind,
    pub subject: String,
    pub detail: String,
    pub origin: Origin,
    /// The encoded question, or why it cannot be encoded (*unsupported*).
    pub encoded: Result<Problem<'a>, EncodeError>,
    pub check: Check<'a>,
    /// Where the things an unsatisfiable core can name were declared, by description.
    pub core_origins: Vec<(String, Origin)>,
}

fn is_prelude(ir: &Ir, d: &Decl) -> bool {
    ir.package(d.package).kind == PackageKind::Prelude
}

fn enforced(rules: &[Rule]) -> Vec<&Rule> {
    rules.iter().filter(|r| !r.is_audited()).collect()
}

/// The first question: can the invariants of each declaration all hold, with its constraints?
#[must_use]
pub fn invariant_questions(ir: &Ir) -> Vec<Question<'_>> {
    ir.decls
        .iter()
        .filter(|d| !is_prelude(ir, d))
        .filter_map(|d| invariant_question(ir, d))
        .collect()
}

fn invariant_question<'a>(ir: &'a Ir, decl: &'a Decl) -> Option<Question<'a>> {
    let rules = enforced(decl.rules());
    if rules.is_empty() {
        return None;
    }
    let roots = BTreeMap::from([(RootKey::SelfValue, RootSpec::validated(Type::Decl(decl.id)))]);
    let encoded = encode_rules(Problem::new(ir, roots), &rules, "");
    Some(Question {
        kind: QuestionKind::Invariants,
        subject: decl.name.clone(),
        detail: String::new(),
        origin: decl.origin.clone(),
        encoded,
        check: Check::Invariants(ir, decl),
        core_origins: rules
            .iter()
            .map(|r| (format!("rule `{}`", r.name), r.origin.clone()))
            .collect(),
    })
}

/// Asserts each rule, under a name that says what it is.
fn encode_rules<'a>(
    mut p: Problem<'a>,
    rules: &[&Rule],
    suffix: &str,
) -> Result<Problem<'a>, EncodeError> {
    for r in rules {
        let holds = p.holds(&r.expr)?;
        p.assert_named(&format!("rule `{}`{suffix}", r.name), &holds);
    }
    Ok(p)
}

/// The roots of an endpoint: validated parameters, trusted facts and principal.
#[must_use]
pub fn endpoint_roots(service: &Service, ep: &Endpoint) -> BTreeMap<RootKey, RootSpec> {
    let mut roots = BTreeMap::new();
    for p in ep.params.iter().filter(|p| !p.stream) {
        let spec = RootSpec {
            ty: p.ty.clone(),
            optional: p.optional,
            constraints: p.constraints.clone(),
            validated: true,
        };
        roots.insert(RootKey::Param(p.name.clone()), spec);
    }
    for f in &ep.rules.facts {
        let spec = RootSpec {
            optional: f.optional,
            ..RootSpec::trusted(f.ty.clone())
        };
        roots.insert(RootKey::Fact(f.name.clone()), spec);
    }
    if let Some(pr) = &service.principal {
        roots.insert(RootKey::Principal, RootSpec::trusted(Type::Decl(pr.decl)));
    }
    roots
}

/// The invariants of the parameters hold: what a request has passed before any `require` runs.
/// A rule that reads a `@readonly` field is skipped in a request (`spec/rules.md` section 5.2).
///
/// # Errors
/// As [`Problem::encode`].
pub fn assume_param_invariants(
    p: &mut Problem<'_>,
    ir: &Ir,
    params: &[Param],
) -> Result<(), EncodeError> {
    for param in params.iter().filter(|q| !q.stream) {
        let Type::Decl(id) = &param.ty else { continue };
        let decl = ir.decl(*id);
        if !matches!(decl.kind, DeclKind::Model(_) | DeclKind::Error(_)) {
            continue;
        }
        let root = RootKey::Param(param.name.clone());
        p.self_root = root.clone();
        for r in enforced(decl.rules())
            .into_iter()
            .filter(|r| !r.reads_readonly)
        {
            let holds = p.holds(&r.expr)?;
            let guarded = if param.optional {
                let has = p.has_root(&root);
                smt::or2(&smt::not(&has), &holds)
            } else {
                holds
            };
            p.assert_named(
                &format!("rule `{}` of parameter `{}`", r.name, param.name),
                &guarded,
            );
        }
        p.self_root = RootKey::SelfValue;
    }
    Ok(())
}

fn label(r: &veld_ir::EndpointRequire, index: usize) -> String {
    match &r.name {
        Some(n) => format!("require `{n}`"),
        None => format!("require #{}", index + 1),
    }
}

/// The second question: can each `require` pass, given the invariants of the parameters and the
/// `require` lines before it?
#[must_use]
pub fn require_questions(ir: &Ir) -> Vec<Question<'_>> {
    let mut out = Vec::new();
    for s in &ir.services {
        for ep in &s.endpoints {
            for (i, r) in ep.rules.requires.iter().enumerate() {
                if r.rollout.is_none() {
                    out.push(require_question(ir, s, ep, i, &label(r, i)));
                }
            }
        }
    }
    out
}

fn require_question<'a>(
    ir: &'a Ir,
    s: &'a Service,
    ep: &'a Endpoint,
    index: usize,
    what: &str,
) -> Question<'a> {
    let encoded = encode_require(ir, s, ep, index);
    let mut core_origins = vec![(what.to_owned(), ep.rules.requires[index].origin.clone())];
    for (i, r) in ep.rules.requires.iter().enumerate().take(index) {
        core_origins.push((label(r, i), r.origin.clone()));
    }
    Question {
        kind: QuestionKind::Require,
        subject: format!("{}.{}", s.name, ep.name),
        detail: what.to_owned(),
        origin: ep.rules.requires[index].origin.clone(),
        encoded,
        check: Check::Require(ir, ep, index),
        core_origins,
    }
}

fn encode_require<'a>(
    ir: &'a Ir,
    s: &'a Service,
    ep: &'a Endpoint,
    index: usize,
) -> Result<Problem<'a>, EncodeError> {
    let mut p = Problem::new(ir, endpoint_roots(s, ep));
    assume_param_invariants(&mut p, ir, &ep.params)?;
    for (i, r) in ep.rules.requires.iter().enumerate().take(index + 1) {
        if r.rollout.is_some() {
            continue;
        }
        let holds = p.holds(&r.expr)?;
        p.assert_named(&label(r, i), &holds);
    }
    Ok(p)
}

/// The third question: does each policy permit anybody?
#[must_use]
pub fn policy_questions(ir: &Ir) -> Vec<Question<'_>> {
    ir.policies
        .iter()
        .map(|policy| Question {
            kind: QuestionKind::Policy,
            subject: policy.name.clone(),
            detail: String::new(),
            origin: policy.origin.clone(),
            encoded: encode_policy(ir, policy),
            check: Check::Policy(ir, policy),
            core_origins: Vec::new(),
        })
        .collect()
}

/// A policy permits when some `permit` holds and no `forbid` holds or fails to evaluate
/// (`spec/rules.md` section 11.2).
///
/// # Errors
/// As [`Problem::encode`].
pub fn encode_policy<'a>(ir: &'a Ir, policy: &'a Policy) -> Result<Problem<'a>, EncodeError> {
    let roots = policy
        .params
        .iter()
        .map(|q| {
            (
                RootKey::Param(q.name.clone()),
                RootSpec::trusted(q.ty.clone()),
            )
        })
        .collect();
    let mut p = Problem::new(ir, roots);
    let permitted = permitted_term(&mut p, policy)?;
    p.assert_named("the policy permits somebody", &permitted);
    Ok(p)
}

/// The term that says a policy permits, for the arguments the variables stand for.
///
/// # Errors
/// As [`Problem::encode`].
pub fn permitted_term(p: &mut Problem<'_>, policy: &Policy) -> Result<String, EncodeError> {
    let (mut permits, mut forbids) = (Vec::new(), Vec::new());
    for clause in &policy.clauses {
        let pair = p.encode(&clause.expr)?;
        match clause.effect {
            PolicyEffect::Permit => permits.push(smt::and2(&smt::not(&pair.err), &pair.v)),
            PolicyEffect::Forbid => forbids.push(smt::or2(&pair.err, &pair.v)),
        }
    }
    Ok(smt::and2(
        &smt::or_all(&permits),
        &smt::not(&smt::or_all(&forbids)),
    ))
}

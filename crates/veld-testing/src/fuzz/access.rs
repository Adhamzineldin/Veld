//! Policies and the transition guard: how the generator gets past them, and into every state.
//!
//! * A **denial** by an `allow` is repaired by making a `permit` clause of the policy true (one
//!   chosen at random, so that every `permit` is reached) and every `forbid` clause that holds
//!   false. The clauses speak of the policy's parameters; the arguments of the `allow` say which
//!   path of the request each stands for, so a clause is rewritten into the endpoint's scope and
//!   handed to the same machinery that satisfies a `require` (`rules::apply`).
//! * An **illegal state** is repaired by writing one of the source states of the transition.
//! * **Every state** of the machine is also written on purpose (see [`states`]), so that the
//!   guard is met in each state it refuses and each state it lets through.

use std::collections::BTreeMap;

use serde_json::Value as Json;
use veld_ir::{
    BinExpr, CallExpr, DeclKind, Endpoint, EndpointAllow, EndpointTransition, Expr, ExprKind, Ir,
    PathRef, Policy, PolicyEffect, QuantExpr,
};
use veld_rules::{Env, Request, Value, Violation, eval};

use super::loc::{Loc, Step, slot};
use super::rules::{Scope, Task, apply, env_of, falsifying, path_of, satisfying, tasks};
use super::values::Gen;

/// Repairs a violation that names a policy or the transition of `ep`. `None` when it names
/// neither, and the rules take it from there.
pub(crate) fn repair(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    v: &Violation,
) -> Option<bool> {
    if let Some(t) = ep.rules.transition.as_ref().filter(|t| t.id == v.rule_id) {
        let from = &t.from;
        let state = from.get(g.rng.index(from.len()))?;
        return Some(set_state(g, ep, req, t, state).is_some());
    }
    for allow in &ep.rules.allows {
        let policy = g.ir.policies.get(allow.policy.index())?;
        if policy.id == v.rule_id {
            return Some(repair_policy(g, ep, req, allow, policy));
        }
    }
    None
}

/// Every state of the machine of the transition of `ep`, with the transition, in declaration
/// order. Empty when the endpoint has no transition.
pub(crate) fn states<'a>(ir: &'a Ir, ep: &'a Endpoint) -> Vec<(&'a EndpointTransition, &'a str)> {
    let Some(t) = ep.rules.transition.as_ref() else {
        return Vec::new();
    };
    match ir.decls.get(t.decl.index()).map(|d| &d.kind) {
        Some(DeclKind::Enum(e)) => e.variants.iter().map(|v| (t, v.name.as_str())).collect(),
        _ => Vec::new(),
    }
}

/// Writes the state `name` into the state field of the fact of `t`.
pub(crate) fn set_state(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    t: &EndpointTransition,
    name: &str,
) -> Option<()> {
    let Some(DeclKind::Enum(machine)) = g.ir.decls.get(t.decl.index()).map(|d| &d.kind) else {
        return None;
    };
    let wire = machine
        .variants
        .iter()
        .find(|v| v.name == name)?
        .wire
        .clone();
    let at = Loc {
        root: t.fact.clone(),
        fact: true,
        steps: t
            .steps
            .iter()
            .map(|s| Step::Field(s.wire.clone()))
            .collect(),
    };
    *slot(g, ep, req, &at)?.json = Json::String(wire);
    Some(())
}

/// The path each parameter of the policy stands for, where the argument is a plain path.
type Stands<'a> = BTreeMap<&'a str, PathRef>;

fn repair_policy(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    allow: &EndpointAllow,
    policy: &Policy,
) -> bool {
    let stands: Stands<'_> = policy
        .params
        .iter()
        .zip(&allow.args)
        .filter_map(|(p, a)| Some((p.name.as_str(), path_of(a)?.clone())))
        .collect();
    let Some(env) = policy_env(g.ir, req, allow, policy) else {
        return false;
    };
    let fires = |c: &veld_ir::PolicyClause| match eval(&env, &c.expr) {
        Ok(Value::Bool(b)) => b,
        // A clause that cannot be evaluated fails closed: a `forbid` holds, a `permit` does not.
        Err(_) => c.effect == PolicyEffect::Forbid,
        Ok(_) => false,
    };
    let mut changed = false;
    let mut permitted = false;
    for c in &policy.clauses {
        match (c.effect, fires(c)) {
            (PolicyEffect::Permit, held) => permitted |= held,
            (PolicyEffect::Forbid, true) => {
                let found = rewrite(&c.expr, &stands)
                    .map(|e| falsifying(&Scope::Request, &e))
                    .unwrap_or_default();
                changed |= apply_all(g, ep, req, &found);
            }
            (PolicyEffect::Forbid, false) => {}
        }
    }
    if !permitted {
        changed |= permit_one(g, ep, req, policy, &stands);
    }
    changed
}

/// Makes one `permit` clause true, starting from a random one.
fn permit_one(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    policy: &Policy,
    stands: &Stands<'_>,
) -> bool {
    let permits: Vec<&Expr> = policy
        .clauses
        .iter()
        .filter(|c| c.effect == PolicyEffect::Permit)
        .map(|c| &c.expr)
        .collect();
    if permits.is_empty() {
        return false;
    }
    let start = g.rng.index(permits.len());
    for k in 0..permits.len() {
        let clause = permits[(start + k) % permits.len()];
        let Some(rewritten) = rewrite(clause, stands) else {
            continue;
        };
        let found = tasks(&Scope::Request, &rewritten, false, true);
        if apply_all(g, ep, req, &found) {
            return true;
        }
    }
    false
}

fn apply_all(g: &mut Gen<'_>, ep: &Endpoint, req: &mut Request, found: &[Task]) -> bool {
    let mut any = false;
    for t in found {
        any |= apply(g, ep, req, t, satisfying(t.op)).is_some();
    }
    any
}

/// The environment the clauses of `policy` are evaluated in for this request: its parameters bound
/// to the values of the arguments of `allow`.
fn policy_env<'a>(
    ir: &'a Ir,
    req: &Request,
    allow: &EndpointAllow,
    policy: &Policy,
) -> Option<Env<'a>> {
    let outer = env_of(ir, req, None);
    let mut env = Env::new(ir);
    env.now = Some(req.now);
    for (p, a) in policy.params.iter().zip(&allow.args) {
        env.params
            .insert(p.name.clone(), eval(&outer, a).ok()?.to_json().ok()?);
    }
    Some(env)
}

// ---- rewriting a clause into the scope of the endpoint ------------------------------------

/// `path` with its root replaced by the path the policy parameter stands for.
fn rewrite_path(p: &PathRef, stands: &Stands<'_>) -> Option<PathRef> {
    let veld_ir::Root::Param(name) = &p.root else {
        return Some(p.clone());
    };
    let base = stands.get(name.as_str())?;
    let mut out = base.clone();
    out.steps.extend(p.steps.iter().cloned());
    Some(out)
}

fn rewrite_all(list: &[Expr], stands: &Stands<'_>) -> Option<Vec<Expr>> {
    list.iter().map(|e| rewrite(e, stands)).collect()
}

/// `e` with every path of a policy parameter written as the path of the endpoint it stands for.
/// `None` when a parameter's argument is not a plain path.
fn rewrite(e: &Expr, stands: &Stands<'_>) -> Option<Expr> {
    let boxed = |x: &Expr| rewrite(x, stands).map(Box::new);
    let kind = match &e.e {
        ExprKind::Lit(_) | ExprKind::Now(_) => e.e.clone(),
        ExprKind::Path(p) => ExprKind::Path(rewrite_path(p, stands)?),
        ExprKind::Has(p) => ExprKind::Has(rewrite_path(p, stands)?),
        ExprKind::Not(x) => ExprKind::Not(boxed(x)?),
        ExprKind::Neg(x) => ExprKind::Neg(boxed(x)?),
        ExprKind::Widen(x) => ExprKind::Widen(boxed(x)?),
        ExprKind::Old(x) => ExprKind::Old(boxed(x)?),
        ExprKind::Bin(b) => ExprKind::Bin(Box::new(BinExpr {
            op: b.op,
            l: rewrite(&b.l, stands)?,
            r: rewrite(&b.r, stands)?,
        })),
        ExprKind::Call(c) => ExprKind::Call(CallExpr {
            f: c.f,
            args: rewrite_all(&c.args, stands)?,
        }),
        ExprKind::Quant(q) => ExprKind::Quant(Box::new(QuantExpr {
            q: q.q,
            list: rewrite(&q.list, stands)?,
            var: q.var.clone(),
            body: rewrite(&q.body, stands)?,
        })),
        ExprKind::List(items) => ExprKind::List(rewrite_all(items, stands)?),
    };
    Some(Expr {
        t: e.t.clone(),
        e: kind,
    })
}

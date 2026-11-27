//! The confirmation step (`spec/rules.md` section 18.2): a value from the solver is shown only
//! after the reference evaluator agrees with it.

use serde_json::{Map, Value as Json};
use veld_ir::{DeclKind, Endpoint, Expr, Ir, Param, Policy, Type};
use veld_rules::{Env, Failure, Options, Value, check_value, eval, eval_policy};

use crate::sample;
use crate::types::RootKey;
use crate::values::Concrete;

/// The value as text, for the report.
#[must_use]
pub fn describe(c: &Concrete, with_now: bool) -> String {
    let mut obj = Map::new();
    for (key, json) in &c.roots {
        let name = match key {
            RootKey::SelfValue => {
                return json.to_string();
            }
            RootKey::Param(n) => n.clone(),
            RootKey::Fact(n) => format!("fact {n}"),
            RootKey::Principal => "principal".to_owned(),
            RootKey::Result => "result".to_owned(),
        };
        obj.insert(name, json.clone());
    }
    if with_now {
        obj.insert(
            "now".to_owned(),
            Json::from(veld_rules::format_datetime(c.now)),
        );
    }
    Json::Object(obj).to_string()
}

/// The environment of an endpoint rule or a policy for the inputs `c`.
#[must_use]
pub fn env_of<'a>(ir: &'a Ir, c: &Concrete) -> Env<'a> {
    let mut env = Env::new(ir);
    env.now = Some(c.now);
    for (key, json) in &c.roots {
        match key {
            RootKey::Param(n) => {
                env.params.insert(n.clone(), json.clone());
            }
            RootKey::Fact(n) => {
                env.facts.insert(n.clone(), json.clone());
            }
            RootKey::Principal => env.principal = Some(json.clone()),
            RootKey::Result => env.result = Some(json.clone()),
            RootKey::SelfValue => {}
        }
    }
    env
}

/// What an expression came to for the evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Holds,
    Fails,
    /// An evaluation error: the rule fails closed.
    Errors,
}

/// Evaluates a Boolean expression.
///
/// # Errors
/// A text when the evaluator cannot evaluate it at all (a shape that does not fit).
pub fn judge(env: &Env<'_>, expr: &Expr) -> Result<Verdict, String> {
    match eval(env, expr) {
        Ok(Value::Bool(true)) => Ok(Verdict::Holds),
        Ok(Value::Bool(false)) => Ok(Verdict::Fails),
        Ok(other) => Err(format!("a rule is {}, not bool", other.kind())),
        Err(Failure::Eval(_)) => Ok(Verdict::Errors),
        Err(Failure::Input(e)) => Err(e.to_string()),
    }
}

/// Every enforced rule of `decl` holds for the value, and the value keeps its constraints.
///
/// # Errors
/// A text that says which check the value fails.
pub fn confirm_invariants(ir: &Ir, decl: &veld_ir::Decl, c: &Concrete) -> Result<(), String> {
    let json = c
        .roots
        .get(&RootKey::SelfValue)
        .ok_or("no value was built")?;
    let found = check_value(ir, decl, json, Options::default()).map_err(|e| e.to_string())?;
    if let Some(v) = found.iter().find(|v| v.path == "$") {
        return Err(format!(
            "the evaluator rejects the value: rule `{}`",
            v.rule_name
        ));
    }
    sample::check_decl(ir, decl, json, 0)
}

/// The parameters are valid requests: their invariants and constraints hold.
///
/// # Errors
/// A text that says which parameter fails which check.
pub fn confirm_params(ir: &Ir, params: &[Param], c: &Concrete) -> Result<(), String> {
    for p in params.iter().filter(|p| !p.stream) {
        let Some(json) = c.roots.get(&RootKey::Param(p.name.clone())) else {
            continue;
        };
        sample::check_scalar(&p.ty, &p.constraints, json)
            .map_err(|e| format!("parameter `{}`: {e}", p.name))?;
        if let Type::Decl(id) = &p.ty {
            let decl = ir.decl(*id);
            if matches!(decl.kind, DeclKind::Model(_) | DeclKind::Error(_)) {
                let options = Options {
                    request: true,
                    partial: false,
                };
                let found = check_value(ir, decl, json, options).map_err(|e| e.to_string())?;
                if let Some(v) = found.iter().find(|v| v.path == "$") {
                    return Err(format!(
                        "the evaluator rejects parameter `{}`: rule `{}`",
                        p.name, v.rule_name
                    ));
                }
                sample::check_decl(ir, decl, json, 0)?;
            }
        }
    }
    Ok(())
}

/// The enforced `require` lines of `ep` up to and including `index` all pass.
///
/// # Errors
/// A text that says which line fails.
pub fn confirm_require(ir: &Ir, ep: &Endpoint, index: usize, c: &Concrete) -> Result<(), String> {
    confirm_params(ir, &ep.params, c)?;
    let env = env_of(ir, c);
    for r in ep.rules.requires.iter().take(index + 1) {
        if r.rollout.is_some() {
            continue;
        }
        let name = r.name.as_deref().unwrap_or("require");
        match judge(&env, &r.expr)? {
            Verdict::Holds => {}
            Verdict::Fails | Verdict::Errors => {
                return Err(format!("the evaluator rejects `{name}`"));
            }
        }
    }
    Ok(())
}

/// The policy permits the arguments.
///
/// # Errors
/// A text when it does not.
pub fn confirm_policy(ir: &Ir, policy: &Policy, c: &Concrete) -> Result<(), String> {
    let mut args = Vec::new();
    for p in &policy.params {
        let arg = c.roots.get(&RootKey::Param(p.name.clone()));
        args.push(arg.cloned().ok_or("an argument is missing")?);
    }
    let verdict = eval_policy(ir, policy, &args, c.now).map_err(|e| e.to_string())?;
    if verdict.permitted {
        Ok(())
    } else {
        Err("the evaluator says the policy denies the arguments".to_owned())
    }
}

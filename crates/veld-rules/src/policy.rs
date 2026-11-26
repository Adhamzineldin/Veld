//! Policies (`spec/rules.md` section 11.2): the verdict of one policy, and the `allow` step of
//! the pipeline.
//!
//! A policy permits when at least one `permit` is true and no `forbid` is true. A clause that
//! cannot be evaluated (section 3.7) fails closed: such a `permit` does not permit, and such a
//! `forbid` forbids.

use std::collections::BTreeMap;

use serde_json::Value as Json;
use veld_ir::{Ir, Policy, PolicyEffect};

use crate::error::{Failure, InputError};
use crate::eval::{Env, eval};
use crate::value::Value;

/// The text of a denial that no `forbid` explains.
pub const NOT_PERMITTED: &str = "not permitted";

/// What a policy decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyVerdict {
    /// True when at least one `permit` is true and no `forbid` is true.
    pub permitted: bool,
    /// The message of the first `forbid` that was true (one that cannot be evaluated counts as
    /// true), when the policy denies and that clause has a message. `None` otherwise.
    pub message: Option<String>,
    /// The number, counted from 1 among the `forbid` clauses of the policy, of the one that denied,
    /// when it has a message: what the message key `<package>/<Policy>.forbid<n>` names
    /// (`spec/platform.md` section 22). `None` when the policy permits or no `forbid` with a
    /// message explains the denial.
    pub forbid: Option<usize>,
}

impl PolicyVerdict {
    /// The text of the denial: the message of the first `forbid` that was true if it has one,
    /// otherwise `not permitted`.
    #[must_use]
    pub fn denial_message(&self) -> &str {
        self.message.as_deref().unwrap_or(NOT_PERMITTED)
    }
}

/// What a clause came to: it holds, it does not, or it could not be evaluated.
enum Clause {
    Holds,
    Fails,
    Errors,
}

fn clause(env: &Env<'_>, expr: &veld_ir::Expr) -> Result<Clause, InputError> {
    match eval(env, expr) {
        Ok(Value::Bool(true)) => Ok(Clause::Holds),
        Ok(Value::Bool(false)) => Ok(Clause::Fails),
        Ok(other) => Err(InputError::Ir(format!(
            "a clause is {}, not bool",
            other.kind()
        ))),
        Err(Failure::Eval(_)) => Ok(Clause::Errors),
        Err(Failure::Input(e)) => Err(e),
    }
}

/// Evaluates `policy` for the arguments `args` (wire form, one per parameter) at the instant
/// `now` (milliseconds since the epoch).
///
/// # Errors
/// [`InputError`] when an argument does not have the shape of its parameter, or the IR is not
/// well formed. A clause that fails to evaluate is not an error: it fails closed.
pub fn eval_policy(
    ir: &Ir,
    policy: &Policy,
    args: &[Json],
    now: i64,
) -> Result<PolicyVerdict, InputError> {
    if args.len() != policy.params.len() {
        return Err(InputError::Ir(format!(
            "policy `{}` takes {} arguments, found {}",
            policy.name,
            policy.params.len(),
            args.len()
        )));
    }
    let mut env = Env::new(ir);
    env.now = Some(now);
    env.params = policy
        .params
        .iter()
        .zip(args)
        .map(|(p, a)| (p.name.clone(), a.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut permitted = false;
    let mut forbidden = false;
    let mut message = None;
    let mut forbid = None;
    let mut forbids = 0;
    for c in &policy.clauses {
        forbids += usize::from(c.effect == PolicyEffect::Forbid);
        let held = clause(&env, &c.expr)?;
        match (c.effect, held) {
            (PolicyEffect::Permit, Clause::Holds) => permitted = true,
            (PolicyEffect::Permit, Clause::Fails | Clause::Errors) => {}
            (PolicyEffect::Forbid, Clause::Fails) => {}
            (PolicyEffect::Forbid, Clause::Holds | Clause::Errors) => {
                if !forbidden {
                    message.clone_from(&c.message);
                    forbid = c.message.is_some().then_some(forbids);
                }
                forbidden = true;
            }
        }
    }
    Ok(PolicyVerdict {
        permitted: permitted && !forbidden,
        message: if forbidden { message } else { None },
        forbid: if forbidden { forbid } else { None },
    })
}

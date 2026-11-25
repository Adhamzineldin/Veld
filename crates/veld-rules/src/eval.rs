//! Evaluates a typed rule expression against a value (`spec/rules.md` section 3).

use std::collections::BTreeMap;

use serde_json::Value as Json;
use veld_ir::{
    BinExpr, Expr, ExprKind, ExprLit, ExprType, Ir, Op, PathRef, QuantExpr, Quantifier, Root,
    Scalar, Step, Type,
};

use crate::builtins;
use crate::error::{EvalResult, Failure, InputError};
use crate::helpers::decimal::Decimal;
use crate::helpers::num;
use crate::ops;
use crate::value::Value;

/// What an expression can read. Only `this` is filled in for invariants; the other roots are
/// for endpoint rules and policies (Phase 2 and later). A root that is not filled in reads as
/// [`InputError::NotAvailable`].
#[derive(Debug, Clone)]
pub struct Env<'a> {
    pub ir: &'a Ir,
    /// The object that `self` paths start from.
    pub this: Option<&'a Json>,
    pub params: BTreeMap<String, Json>,
    pub facts: BTreeMap<String, Json>,
    pub principal: Option<Json>,
    pub result: Option<Json>,
    /// The injected clock, in milliseconds since the epoch.
    pub now: Option<i64>,
    /// The saved `old(...)` values (`spec/rules.md` section 9.4), found by the structural
    /// equality of the expression inside `old`. See [`crate::save_old`].
    pub old: Vec<SavedOld>,
}

/// One saved `old(e)`: the expression and what it evaluated to before the handler ran, or the
/// evaluation error it failed with (which then fails every rule that reads it).
pub type SavedOld = (Expr, Result<Value, num::EvalError>);

impl<'a> Env<'a> {
    /// An environment with nothing in scope.
    #[must_use]
    pub fn new(ir: &'a Ir) -> Env<'a> {
        Env {
            ir,
            this: None,
            params: BTreeMap::new(),
            facts: BTreeMap::new(),
            principal: None,
            result: None,
            now: None,
            old: Vec::new(),
        }
    }

    /// The environment of an invariant: `self` is `this`.
    #[must_use]
    pub fn for_value(ir: &'a Ir, this: &'a Json) -> Env<'a> {
        Env {
            this: Some(this),
            ..Env::new(ir)
        }
    }
}

/// Evaluates `expr`.
///
/// # Errors
/// [`Failure::Eval`] for overflow, division by zero or a non-finite float (the rule fails
/// closed); [`Failure::Input`] for malformed input or a root that is not available.
pub fn eval(env: &Env<'_>, expr: &Expr) -> Result<Value, Failure> {
    Evaluator {
        env,
        vars: Vec::new(),
    }
    .eval(expr)
}

struct Evaluator<'e, 'a> {
    env: &'e Env<'a>,
    vars: Vec<(String, Value)>,
}

/// Where a path starts.
enum Base<'v> {
    Json(&'v Json),
    Val(&'v Value),
}

fn not_available(what: &'static str) -> InputError {
    InputError::NotAvailable(what)
}

impl Evaluator<'_, '_> {
    fn eval(&mut self, e: &Expr) -> EvalResult<Value> {
        match &e.e {
            ExprKind::Lit(l) => Ok(literal(l, &e.t)?),
            ExprKind::Path(p) => self.path(p, &e.t),
            ExprKind::Now(_) => match self.env.now {
                Some(ms) => Ok(Value::Datetime(ms)),
                None => Err(not_available("now").into()),
            },
            ExprKind::Not(x) => Ok(Value::Bool(!self.eval_bool(x)?)),
            ExprKind::Neg(x) => ops::neg(self.eval(x)?),
            ExprKind::Bin(b) => self.bin(b),
            ExprKind::Has(p) => Ok(Value::Bool(self.has(p)?)),
            ExprKind::Call(c) => {
                let args: EvalResult<Vec<Value>> = c.args.iter().map(|a| self.eval(a)).collect();
                builtins::call(c.f, &args?)
            }
            ExprKind::Quant(q) => self.quant(q, &e.t),
            ExprKind::List(items) => {
                let values: EvalResult<Vec<Value>> = items.iter().map(|a| self.eval(a)).collect();
                Ok(Value::List(values?))
            }
            ExprKind::Widen(x) => Ok(ops::widen(self.eval(x)?, &e.t)?),
            ExprKind::Old(inner) => self.old(inner),
        }
    }

    fn old(&self, inner: &Expr) -> EvalResult<Value> {
        match self.env.old.iter().find(|(saved, _)| saved == inner) {
            Some((_, Ok(v))) => Ok(v.clone()),
            Some((_, Err(e))) => Err((*e).into()),
            None => Err(not_available("old").into()),
        }
    }

    fn eval_bool(&mut self, e: &Expr) -> EvalResult<bool> {
        Ok(as_bool(self.eval(e)?)?)
    }

    fn bin(&mut self, b: &BinExpr) -> EvalResult<Value> {
        let short = match b.op {
            Op::And => Some(self.eval_bool(&b.l)? && self.eval_bool(&b.r)?),
            Op::Or => Some(self.eval_bool(&b.l)? || self.eval_bool(&b.r)?),
            Op::Implies => Some(!self.eval_bool(&b.l)? || self.eval_bool(&b.r)?),
            Op::Eq
            | Op::Ne
            | Op::Lt
            | Op::Le
            | Op::Gt
            | Op::Ge
            | Op::In
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Rem => None,
        };
        if let Some(verdict) = short {
            return Ok(Value::Bool(verdict));
        }
        let l = self.eval(&b.l)?;
        let r = self.eval(&b.r)?;
        match b.op {
            Op::Eq => Ok(Value::Bool(ops::equals(&l, &r)?)),
            Op::Ne => Ok(Value::Bool(!ops::equals(&l, &r)?)),
            Op::Lt => Ok(Value::Bool(ops::compare(&l, &r)?.is_lt())),
            Op::Le => Ok(Value::Bool(ops::compare(&l, &r)?.is_le())),
            Op::Gt => Ok(Value::Bool(ops::compare(&l, &r)?.is_gt())),
            Op::Ge => Ok(Value::Bool(ops::compare(&l, &r)?.is_ge())),
            Op::In => Ok(Value::Bool(ops::contains(&l, &r)?)),
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem => ops::arith(b.op, l, r),
            Op::And | Op::Or | Op::Implies => Err(InputError::ir("logical operator").into()),
        }
    }

    fn base(&self, root: &Root) -> Result<Base<'_>, InputError> {
        let env = self.env;
        match root {
            Root::SelfValue => env.this.map(Base::Json).ok_or(not_available("self")),
            Root::Var(name) => self
                .vars
                .iter()
                .rev()
                .find(|(n, _)| n == name)
                .map(|(_, v)| Base::Val(v))
                .ok_or_else(|| InputError::Ir(format!("unbound variable `{name}`"))),
            Root::Param(n) => env
                .params
                .get(n)
                .map(Base::Json)
                .ok_or(not_available("param")),
            Root::Fact(n) => env
                .facts
                .get(n)
                .map(Base::Json)
                .ok_or(not_available("fact")),
            Root::Principal => env
                .principal
                .as_ref()
                .map(Base::Json)
                .ok_or(not_available("principal")),
            Root::Result => env
                .result
                .as_ref()
                .map(Base::Json)
                .ok_or(not_available("result")),
        }
    }

    fn path(&self, p: &PathRef, ty: &ExprType) -> EvalResult<Value> {
        let json = match self.base(&p.root)? {
            Base::Json(j) => j,
            Base::Val(v) if p.steps.is_empty() => return Ok(v.clone()),
            Base::Val(Value::Raw(j)) => j,
            Base::Val(v) => {
                return Err(InputError::Ir(format!("a path into {}", v.kind())).into());
            }
        };
        let leaf = navigate(json, &p.steps)?;
        match ty {
            ExprType::Type(t) => Ok(Value::from_json(self.env.ir, t, leaf)?),
            ExprType::Duration => Err(InputError::ir("a path of type duration").into()),
        }
    }

    /// `has(p)`: true when every step of the path is present.
    fn has(&self, p: &PathRef) -> EvalResult<bool> {
        let base = match self.base(&p.root) {
            Ok(base) => base,
            // An optional parameter, optional fact or principal that is absent is simply not
            // in the environment: `has` of it is false.
            Err(InputError::NotAvailable(_))
                if matches!(p.root, Root::Param(_) | Root::Fact(_) | Root::Principal) =>
            {
                return Ok(false);
            }
            Err(e) => return Err(e.into()),
        };
        let json = match base {
            Base::Json(j) => j,
            Base::Val(Value::Raw(j)) => j,
            Base::Val(_) => return Ok(p.steps.is_empty()),
        };
        let mut cur = json;
        for step in &p.steps {
            let obj = cur
                .as_object()
                .ok_or_else(|| InputError::mismatch("object", "a non-object"))?;
            match obj.get(&step.wire) {
                Some(next) => cur = next,
                None => return Ok(false),
            }
        }
        Ok(true)
    }

    fn bind_eval(&mut self, var: &str, item: Value, body: &Expr) -> EvalResult<Value> {
        self.vars.push((var.to_owned(), item));
        let out = self.eval(body);
        self.vars.pop();
        out
    }

    fn quant(&mut self, q: &QuantExpr, ty: &ExprType) -> EvalResult<Value> {
        let Value::List(items) = self.eval(&q.list)? else {
            return Err(InputError::ir("a comprehension over something that is not a list").into());
        };
        let mut hits = 0usize;
        let mut acc = match q.q {
            Quantifier::Sum => zero_of(ty)?,
            Quantifier::All | Quantifier::Any | Quantifier::Count => Value::Bool(false),
        };
        for item in items {
            let v = self.bind_eval(&q.var, item, &q.body)?;
            if q.q == Quantifier::Sum {
                acc = ops::arith(Op::Add, acc, v)?;
                continue;
            }
            let hit = as_bool(v)?;
            match q.q {
                Quantifier::All if !hit => return Ok(Value::Bool(false)),
                Quantifier::Any if hit => return Ok(Value::Bool(true)),
                Quantifier::Count if hit => hits += 1,
                Quantifier::All | Quantifier::Any | Quantifier::Count | Quantifier::Sum => {}
            }
        }
        match q.q {
            Quantifier::All => Ok(Value::Bool(true)),
            Quantifier::Any => Ok(Value::Bool(false)),
            Quantifier::Count => Ok(Value::Int(
                i32::try_from(hits).map_err(|_| num::EvalError::Overflow)?,
            )),
            Quantifier::Sum => Ok(acc),
        }
    }
}

fn as_bool(v: Value) -> Result<bool, InputError> {
    if let Value::Bool(b) = v {
        Ok(b)
    } else {
        Err(InputError::Ir(format!("expected bool, got {}", v.kind())))
    }
}

/// The additive identity of a numeric type: where `sum` starts.
fn zero_of(ty: &ExprType) -> Result<Value, InputError> {
    let ExprType::Type(Type::Scalar(s)) = ty else {
        return Err(InputError::ir("`sum` of a type that is not numeric"));
    };
    match s {
        Scalar::Int => Ok(Value::Int(0)),
        Scalar::Long => Ok(Value::Long(0)),
        Scalar::Float => Ok(Value::Float(0.0)),
        Scalar::Decimal => Ok(Value::Decimal(Decimal::from_i64(0))),
        Scalar::String
        | Scalar::Bool
        | Scalar::Date
        | Scalar::Datetime
        | Scalar::Time
        | Scalar::Uuid
        | Scalar::Bytes
        | Scalar::Json => Err(InputError::ir("`sum` of a type that is not numeric")),
    }
}

/// Follows `steps` (wire names) from `from`.
fn navigate<'j>(from: &'j Json, steps: &[Step]) -> Result<&'j Json, InputError> {
    let mut cur = from;
    for step in steps {
        let obj = cur
            .as_object()
            .ok_or_else(|| InputError::mismatch("object", "a non-object"))?;
        cur = obj
            .get(&step.wire)
            .ok_or_else(|| InputError::MissingField {
                path: String::new(),
                field: step.wire.clone(),
            })?;
    }
    Ok(cur)
}

fn int_literal(n: i64, to: &ExprType) -> Result<Value, InputError> {
    let narrow = || i32::try_from(n).map_err(|_| InputError::ir("an int literal out of range"));
    match to {
        ExprType::Type(Type::Scalar(Scalar::Long)) => Ok(Value::Long(n)),
        ExprType::Type(Type::Scalar(Scalar::Float)) => Ok(Value::Float(f64::from(narrow()?))),
        ExprType::Type(Type::Scalar(Scalar::Decimal)) => Ok(Value::Decimal(Decimal::from_i64(n))),
        ExprType::Duration | ExprType::Type(_) => Ok(Value::Int(narrow()?)),
    }
}

fn literal(l: &ExprLit, ty: &ExprType) -> Result<Value, InputError> {
    Ok(match l {
        ExprLit::Bool(b) => Value::Bool(*b),
        ExprLit::Int(n) => int_literal(*n, ty)?,
        ExprLit::Long(n) => Value::Long(*n),
        ExprLit::Float(x) => Value::Float(*x),
        ExprLit::Decimal(t) => Value::Decimal(
            Decimal::parse(t).ok_or_else(|| InputError::ir("a decimal literal is not a number"))?,
        ),
        ExprLit::String(s) => Value::Str(s.clone()),
        ExprLit::Enum(e) => Value::Enum(e.wire.clone()),
    })
}

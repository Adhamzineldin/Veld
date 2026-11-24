//! The operators of `spec/rules.md` section 3.4 on decoded values.

use std::cmp::Ordering;

use veld_ir::{ExprType, Op, Scalar, Type};

use crate::error::{EvalResult, InputError};
use crate::helpers::decimal::Decimal;
use crate::helpers::instant::MS_PER_DAY;
use crate::helpers::num::{self, Res};
use crate::value::Value;

fn bad_operands(what: &str, l: &Value, r: &Value) -> InputError {
    InputError::Ir(format!("`{what}` on {} and {}", l.kind(), r.kind()))
}

/// The five arithmetic operators, so that dispatch on them is exhaustive.
#[derive(Clone, Copy)]
enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

fn arith_of(op: Op) -> Option<Arith> {
    match op {
        Op::Add => Some(Arith::Add),
        Op::Sub => Some(Arith::Sub),
        Op::Mul => Some(Arith::Mul),
        Op::Div => Some(Arith::Div),
        Op::Rem => Some(Arith::Rem),
        Op::And
        | Op::Or
        | Op::Implies
        | Op::Eq
        | Op::Ne
        | Op::Lt
        | Op::Le
        | Op::Gt
        | Op::Ge
        | Op::In => None,
    }
}

fn pick<T>(a: Arith, ops: [fn(T, T) -> Res<T>; 5]) -> fn(T, T) -> Res<T> {
    let [add, sub, mul, div, rem] = ops;
    match a {
        Arith::Add => add,
        Arith::Sub => sub,
        Arith::Mul => mul,
        Arith::Div => div,
        Arith::Rem => rem,
    }
}

/// `a + b` and friends on any numeric or temporal pair.
pub fn arith(op: Op, l: Value, r: Value) -> EvalResult<Value> {
    let a = arith_of(op).ok_or_else(|| InputError::Ir(format!("`{op:?}` is not arithmetic")))?;
    Ok(match (l, r) {
        (Value::Int(x), Value::Int(y)) => {
            let f = pick(
                a,
                [
                    num::add_i32,
                    num::sub_i32,
                    num::mul_i32,
                    num::div_i32,
                    num::rem_i32,
                ],
            );
            Value::Int(f(x, y)?)
        }
        (Value::Long(x), Value::Long(y)) => Value::Long(long_op(a, x, y)?),
        (Value::Float(x), Value::Float(y)) => {
            let f = pick(
                a,
                [
                    num::add_f64,
                    num::sub_f64,
                    num::mul_f64,
                    num::div_f64,
                    num::rem_f64,
                ],
            );
            Value::Float(f(x, y)?)
        }
        (Value::Decimal(x), Value::Decimal(y)) => Value::Decimal(decimal_op(a, &x, &y)?),
        (l, r) => temporal(a, l, r)?,
    })
}

fn long_op(a: Arith, x: i64, y: i64) -> Res<i64> {
    let f = pick(
        a,
        [
            num::add_i64,
            num::sub_i64,
            num::mul_i64,
            num::div_i64,
            num::rem_i64,
        ],
    );
    f(x, y)
}

fn decimal_op(a: Arith, x: &Decimal, y: &Decimal) -> Result<Decimal, InputError> {
    match a {
        Arith::Add => Ok(x.add(y)),
        Arith::Sub => Ok(x.sub(y)),
        Arith::Mul => Ok(x.mul(y)),
        Arith::Div | Arith::Rem => {
            Err(InputError::Unsupported("`/` and `%` on decimal".to_owned()))
        }
    }
}

/// `datetime` and `duration` arithmetic, and `date - date`.
fn temporal(a: Arith, l: Value, r: Value) -> EvalResult<Value> {
    Ok(match (a, l, r) {
        (Arith::Add, Value::Datetime(t), Value::Duration(d))
        | (Arith::Add, Value::Duration(d), Value::Datetime(t)) => {
            Value::Datetime(num::add_i64(t, d)?)
        }
        (Arith::Sub, Value::Datetime(t), Value::Duration(d)) => {
            Value::Datetime(num::sub_i64(t, d)?)
        }
        (Arith::Sub, Value::Datetime(x), Value::Datetime(y)) => {
            Value::Duration(num::sub_i64(x, y)?)
        }
        (Arith::Sub, Value::Date(x), Value::Date(y)) => {
            Value::Duration(num::mul_i64(num::sub_i64(x, y)?, MS_PER_DAY)?)
        }
        (Arith::Add, Value::Duration(x), Value::Duration(y)) => {
            Value::Duration(num::add_i64(x, y)?)
        }
        (Arith::Sub, Value::Duration(x), Value::Duration(y)) => {
            Value::Duration(num::sub_i64(x, y)?)
        }
        (_, l, r) => return Err(bad_operands("arithmetic", &l, &r).into()),
    })
}

/// Unary minus.
pub fn neg(v: Value) -> EvalResult<Value> {
    Ok(match v {
        Value::Int(x) => Value::Int(num::neg_i32(x)?),
        Value::Long(x) => Value::Long(num::neg_i64(x)?),
        Value::Float(x) => Value::Float(-x),
        Value::Decimal(x) => Value::Decimal(x.negated()),
        Value::Duration(x) => Value::Duration(num::neg_i64(x)?),
        other @ (Value::Bool(_)
        | Value::Str(_)
        | Value::Uuid(_)
        | Value::Bytes(_)
        | Value::Date(_)
        | Value::Datetime(_)
        | Value::Time(_)
        | Value::Enum(_)
        | Value::List(_)
        | Value::Map(_)
        | Value::Raw(_)) => {
            return Err(InputError::Ir(format!("unary `-` on {}", other.kind())).into());
        }
    })
}

/// The ordering of two values of the same orderable type.
pub fn compare(l: &Value, r: &Value) -> Result<Ordering, InputError> {
    let ord = match (l, r) {
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Long(a), Value::Long(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Decimal(a), Value::Decimal(b)) => Some(a.compare(b)),
        (Value::Date(a), Value::Date(b))
        | (Value::Datetime(a), Value::Datetime(b))
        | (Value::Time(a), Value::Time(b))
        | (Value::Duration(a), Value::Duration(b)) => Some(a.cmp(b)),
        _ => return Err(bad_operands("comparison", l, r)),
    };
    ord.ok_or_else(|| InputError::ir("a float that is not a number was compared"))
}

/// `==` by value.
pub fn equals(l: &Value, r: &Value) -> Result<bool, InputError> {
    match (l, r) {
        (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
        (Value::Str(a), Value::Str(b))
        | (Value::Uuid(a), Value::Uuid(b))
        | (Value::Enum(a), Value::Enum(b)) => Ok(a == b),
        (Value::Float(a), Value::Float(b)) => Ok(a == b),
        _ => compare(l, r).map(Ordering::is_eq),
    }
}

/// `x in list` and `key in map`.
pub fn contains(needle: &Value, haystack: &Value) -> Result<bool, InputError> {
    match (needle, haystack) {
        (_, Value::List(items)) => {
            for item in items {
                if equals(needle, item)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        (Value::Str(key), Value::Map(entries)) => Ok(entries.contains_key(key)),
        _ => Err(bad_operands("in", needle, haystack)),
    }
}

/// The exact conversion to the type of a `widen` node (`int` to `long`, `float` or `decimal`;
/// `long` to `decimal`).
pub fn widen(v: Value, to: &ExprType) -> Result<Value, InputError> {
    let target = match to {
        ExprType::Type(Type::Scalar(s)) => Some(*s),
        ExprType::Type(_) | ExprType::Duration => None,
    };
    match (target, v) {
        (Some(Scalar::Long), Value::Int(x)) => Ok(Value::Long(i64::from(x))),
        (Some(Scalar::Float), Value::Int(x)) => Ok(Value::Float(f64::from(x))),
        (Some(Scalar::Decimal), Value::Int(x)) => Ok(Value::Decimal(Decimal::from_i64(x.into()))),
        (Some(Scalar::Decimal), Value::Long(x)) => Ok(Value::Decimal(Decimal::from_i64(x))),
        (_, v) => Err(InputError::Ir(format!(
            "`widen` cannot convert {}",
            v.kind()
        ))),
    }
}

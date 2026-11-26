//! Constant durations (`spec/rules.md` section 3.5, `spec/platform.md` sections 10, 12 and 13):
//! `hours(24)`, `seconds(2) + ms(500)`, `minutes(5) * 2`, folded to milliseconds with checked
//! arithmetic. Used by `flow ... after`, `@timeout` and `@slo`.

use crate::syntax::ast::{Arg, ArgValue, BinOp, CallArg, Expr, ExprKind, LitKind, UnOp};

/// The milliseconds in one unit of a duration constructor, `None` for another name.
#[must_use]
pub fn unit_ms(name: &str) -> Option<i64> {
    Some(match name {
        "days" => 86_400_000,
        "hours" => 3_600_000,
        "minutes" => 60_000,
        "seconds" => 1_000,
        "ms" => 1,
        _ => return None,
    })
}

/// A value of the constant fragment: a duration or a plain integer.
#[derive(Clone, Copy)]
enum Val {
    Dur(i64),
    Int(i64),
}

/// Folds a constant expression of type `duration` to milliseconds. The error is the reason, in
/// words a diagnostic can use.
pub fn const_duration_ms(e: &Expr) -> Result<i64, String> {
    match fold(e)? {
        Val::Dur(ms) => Ok(ms),
        Val::Int(_) => Err("this is a number, not a duration: write `hours(24)`, `ms(500)`".into()),
    }
}

/// Folds the value of a call argument of an annotation: `seconds(2)` in `@timeout(seconds(2))`.
pub fn const_duration_arg(a: &Arg) -> Result<i64, String> {
    let ArgValue::Call { name, args } = &a.value else {
        return Err("expected a duration such as `seconds(2)` or `ms(300)`".into());
    };
    let Some(unit) = unit_ms(&name.name) else {
        return Err(format!(
            "`{}` is not a duration: use days, hours, minutes, seconds or ms",
            name.name
        ));
    };
    let [one] = args.as_slice() else {
        return Err(format!("`{}` takes one argument", name.name));
    };
    match &one.value {
        ArgValue::Literal(l) => match l.kind {
            LitKind::Int(n) => n
                .checked_mul(unit)
                .ok_or_else(|| "the duration is too large".to_string()),
            _ => Err(format!("`{}` takes a whole number", name.name)),
        },
        _ => Err(format!("`{}` takes a whole number", name.name)),
    }
}

fn fold(e: &Expr) -> Result<Val, String> {
    match &e.kind {
        ExprKind::Int(n) => Ok(Val::Int(*n)),
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => match fold(operand)? {
            Val::Int(n) => n.checked_neg().map(Val::Int).ok_or_else(too_large),
            Val::Dur(n) => n.checked_neg().map(Val::Dur).ok_or_else(too_large),
        },
        ExprKind::Call { name, args } => call(&name.name, args),
        ExprKind::Binary { op, l, r } => binary(*op, fold(l)?, fold(r)?),
        _ => Err(
            "this is not a constant: only `days`, `hours`, `minutes`, `seconds` and `ms` \
                  of whole numbers, joined by `+`, `-` and `*`, are"
                .into(),
        ),
    }
}

fn call(name: &str, args: &[CallArg]) -> Result<Val, String> {
    let Some(unit) = unit_ms(name) else {
        return Err(format!("`{name}` is not a duration constructor"));
    };
    let [CallArg::Expr(arg)] = args else {
        return Err(format!("`{name}` takes one argument"));
    };
    match fold(arg)? {
        Val::Int(n) => n.checked_mul(unit).map(Val::Dur).ok_or_else(too_large),
        Val::Dur(_) => Err(format!("`{name}` takes a whole number, not a duration")),
    }
}

fn binary(op: BinOp, l: Val, r: Val) -> Result<Val, String> {
    let sum = |a: i64, b: i64, sub: bool| {
        if sub {
            a.checked_sub(b)
        } else {
            a.checked_add(b)
        }
        .ok_or_else(too_large)
    };
    match (op, l, r) {
        (BinOp::Add | BinOp::Sub, Val::Dur(a), Val::Dur(b)) => {
            sum(a, b, op == BinOp::Sub).map(Val::Dur)
        }
        (BinOp::Add | BinOp::Sub, Val::Int(a), Val::Int(b)) => {
            sum(a, b, op == BinOp::Sub).map(Val::Int)
        }
        (BinOp::Mul, Val::Dur(a), Val::Int(b)) | (BinOp::Mul, Val::Int(b), Val::Dur(a)) => {
            a.checked_mul(b).map(Val::Dur).ok_or_else(too_large)
        }
        (BinOp::Mul, Val::Int(a), Val::Int(b)) => {
            a.checked_mul(b).map(Val::Int).ok_or_else(too_large)
        }
        _ => Err("this combination of operands is not a constant duration".into()),
    }
}

fn too_large() -> String {
    "the duration is too large".into()
}

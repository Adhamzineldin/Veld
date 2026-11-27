//! Two small readers of expressions that a saga needs: the names an expression starts from, and
//! the value of a constant duration (`spec/platform.md` section 17, `timeout`).

use crate::syntax::ast::{self, CallArg, ExprKind as A};
use veld_ir::rules::{Builtin, Expr, ExprKind, ExprLit, Op};

/// Every name an expression starts a path from, in source order (`a` of `a.b.c`, and the names
/// inside calls, lists and operators).
pub(super) fn root_names(e: &ast::Expr) -> Vec<&ast::Ident> {
    let mut out = Vec::new();
    collect(e, &mut out);
    out
}

fn collect<'e>(e: &'e ast::Expr, out: &mut Vec<&'e ast::Ident>) {
    match &e.kind {
        A::Name(id) => out.push(id),
        A::Field { base, .. } => collect(base, out),
        A::Call { args, .. } => {
            for a in args {
                match a {
                    CallArg::Expr(x) => collect(x, out),
                    CallArg::Lambda { body, .. } => collect(body, out),
                }
            }
        }
        A::List(items) => items.iter().for_each(|i| collect(i, out)),
        A::Unary { operand, .. } => collect(operand, out),
        A::Binary { l, r, .. } => {
            collect(l, out);
            collect(r, out);
        }
        A::Bool(_) | A::Int(_) | A::Float { .. } | A::Str(_) | A::EnumLit(_) => {}
    }
}

/// The value in milliseconds of a typed expression made of duration calls with literal
/// arguments, `+`, `-` and unary `-`. `None` for anything else, and on overflow.
pub(super) fn duration_ms(e: &Expr) -> Option<i64> {
    match &e.e {
        ExprKind::Call(c) => {
            let unit: i64 = match c.f {
                Builtin::Days => 86_400_000,
                Builtin::Hours => 3_600_000,
                Builtin::Minutes => 60_000,
                Builtin::Seconds => 1000,
                Builtin::Ms => 1,
                Builtin::Len
                | Builtin::Matches
                | Builtin::StartsWith
                | Builtin::EndsWith
                | Builtin::Contains => return None,
            };
            let [arg] = c.args.as_slice() else {
                return None;
            };
            literal_int(arg)?.checked_mul(unit)
        }
        ExprKind::Neg(x) => duration_ms(x)?.checked_neg(),
        ExprKind::Bin(b) => {
            let (l, r) = (duration_ms(&b.l)?, duration_ms(&b.r)?);
            match b.op {
                Op::Add => l.checked_add(r),
                Op::Sub => l.checked_sub(r),
                Op::And
                | Op::Or
                | Op::Implies
                | Op::Eq
                | Op::Ne
                | Op::Lt
                | Op::Le
                | Op::Gt
                | Op::Ge
                | Op::In
                | Op::Mul
                | Op::Div
                | Op::Rem => None,
            }
        }
        ExprKind::Lit(_)
        | ExprKind::Path(_)
        | ExprKind::Now(_)
        | ExprKind::Not(_)
        | ExprKind::Has(_)
        | ExprKind::Quant(_)
        | ExprKind::List(_)
        | ExprKind::Widen(_)
        | ExprKind::Old(_) => None,
    }
}

fn literal_int(e: &Expr) -> Option<i64> {
    match &e.e {
        ExprKind::Lit(ExprLit::Int(n) | ExprLit::Long(n)) => Some(*n),
        ExprKind::Widen(x) => literal_int(x),
        ExprKind::Neg(x) => literal_int(x)?.checked_neg(),
        ExprKind::Lit(_)
        | ExprKind::Path(_)
        | ExprKind::Now(_)
        | ExprKind::Not(_)
        | ExprKind::Bin(_)
        | ExprKind::Has(_)
        | ExprKind::Call(_)
        | ExprKind::Quant(_)
        | ExprKind::List(_)
        | ExprKind::Old(_) => None,
    }
}

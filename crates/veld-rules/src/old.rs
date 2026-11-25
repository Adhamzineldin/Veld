//! `old(e)`: values saved before the handler runs (`spec/rules.md` section 9.4).

use veld_ir::{Expr, ExprKind};

use crate::error::{Failure, InputError};
use crate::eval::{Env, SavedOld, eval};

/// Every `old(e)` operand in `expr`, outermost first, in evaluation order.
fn operands<'e>(expr: &'e Expr, out: &mut Vec<&'e Expr>) {
    match &expr.e {
        ExprKind::Old(inner) => out.push(inner),
        ExprKind::Lit(_) | ExprKind::Path(_) | ExprKind::Now(_) | ExprKind::Has(_) => {}
        ExprKind::Not(x) | ExprKind::Neg(x) | ExprKind::Widen(x) => operands(x, out),
        ExprKind::Bin(b) => {
            operands(&b.l, out);
            operands(&b.r, out);
        }
        ExprKind::Call(c) => c.args.iter().for_each(|a| operands(a, out)),
        ExprKind::List(items) => items.iter().for_each(|a| operands(a, out)),
        ExprKind::Quant(q) => {
            operands(&q.list, out);
            operands(&q.body, out);
        }
    }
}

/// Evaluates every `old(e)` that occurs in `exprs` once, in `env` (the facts, parameters and
/// clock as they are before the handler runs), and returns the saved values for
/// [`Env::old`]. An `old` expression that fails with an evaluation error is saved as that
/// error, so the `ensure` that reads it fails closed.
///
/// The values are copies: changing the facts afterwards cannot alter them.
///
/// # Errors
/// [`InputError`] when an operand cannot be evaluated because of malformed input.
pub fn save_old(env: &Env<'_>, exprs: &[&Expr]) -> Result<Vec<SavedOld>, InputError> {
    let mut found = Vec::new();
    for e in exprs {
        operands(e, &mut found);
    }
    let mut saved: Vec<SavedOld> = Vec::new();
    for inner in found {
        if saved.iter().any(|(k, _)| k == inner) {
            continue;
        }
        let value = match eval(env, inner) {
            Ok(v) => Ok(v),
            Err(Failure::Eval(e)) => Err(e),
            Err(Failure::Input(e)) => return Err(e),
        };
        saved.push((inner.clone(), value));
    }
    Ok(saved)
}

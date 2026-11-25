//! Binary operators (`spec/rules.md` section 3.4): logic, comparison, `in` and arithmetic.

use super::checker::{Checker, is_flex};
use super::paths::{absent_facts, facts};
use super::types::{
    as_scalar, as_type, boolean, coerce, is_decimal, is_float, is_numeric, is_string, list_elem,
    node, of, scalar, unify_numeric,
};
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::DKind;
use crate::syntax::ast::{self, BinOp};
use veld_ir::rules::{BinExpr, Expr, ExprKind, ExprType, Op};
use veld_ir::{Scalar, Type};

fn ir_op(op: BinOp) -> Op {
    match op {
        BinOp::And => Op::And,
        BinOp::Or => Op::Or,
        BinOp::Implies => Op::Implies,
        BinOp::Eq => Op::Eq,
        BinOp::Ne => Op::Ne,
        BinOp::Lt => Op::Lt,
        BinOp::Le => Op::Le,
        BinOp::Gt => Op::Gt,
        BinOp::Ge => Op::Ge,
        BinOp::In => Op::In,
        BinOp::Add => Op::Add,
        BinOp::Sub => Op::Sub,
        BinOp::Mul => Op::Mul,
        BinOp::Div => Op::Div,
        BinOp::Rem => Op::Rem,
    }
}

fn bin(op: BinOp, t: ExprType, l: Expr, r: Expr) -> Expr {
    node(
        t,
        ExprKind::Bin(Box::new(BinExpr {
            op: ir_op(op),
            l,
            r,
        })),
    )
}

/// Which family of operator, for the checks that differ.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Logic,
    Equality,
    Ordering,
    Arithmetic,
}

fn family(op: BinOp) -> Family {
    match op {
        BinOp::And | BinOp::Or | BinOp::Implies | BinOp::In => Family::Logic,
        BinOp::Eq | BinOp::Ne => Family::Equality,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => Family::Ordering,
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => Family::Arithmetic,
    }
}

impl Checker<'_, '_, '_> {
    pub fn binary(&mut self, op: BinOp, l: &ast::Expr, r: &ast::Expr, span: Span) -> Option<Expr> {
        match op {
            BinOp::And | BinOp::Or | BinOp::Implies => self.logic(op, l, r),
            BinOp::In => self.membership(l, r, span),
            BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Le
            | BinOp::Gt
            | BinOp::Ge
            | BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::Div
            | BinOp::Rem => {
                let (lt, rt) = self.operands(l, r)?;
                match family(op) {
                    Family::Equality => self.equality(op, lt, rt, span),
                    Family::Ordering => self.ordering(op, lt, rt, span),
                    Family::Arithmetic => self.arithmetic(op, lt, rt, span),
                    Family::Logic => None,
                }
            }
        }
    }

    /// `&&`, `||` and `implies` short-circuit, and so does the guard analysis: the right
    /// operand sees what the left operand proves.
    fn logic(&mut self, op: BinOp, l: &ast::Expr, r: &ast::Expr) -> Option<Expr> {
        let name = format!("an operand of `{}`", op.text());
        let left = self.bool_expr(l, &name);
        let known = if op == BinOp::Or {
            absent_facts(l)
        } else {
            facts(l)
        };
        let right = self.with_known(known, |c| c.bool_expr(r, &name));
        Some(bin(op, boolean(), left?, right?))
    }

    /// Types both operands. A literal takes its type from the other side: the typed operand is
    /// checked first and gives the literal its type.
    fn operands(&mut self, l: &ast::Expr, r: &ast::Expr) -> Option<(Expr, Expr)> {
        match (is_flex(l), is_flex(r)) {
            (false, false) => {
                let (a, b) = (self.expr(l, None), self.expr(r, None));
                Some((a?, b?))
            }
            (true, false) => {
                let b = self.expr(r, None)?;
                let a = self.expr(l, Some(&b.t))?;
                Some((a, b))
            }
            (false, true) => {
                let a = self.expr(l, None)?;
                let b = self.expr(r, Some(&a.t))?;
                Some((a, b))
            }
            (true, true) => self.both_flex(l, r),
        }
    }

    /// Two literals: each takes its default type, then both take the common one.
    fn both_flex(&mut self, l: &ast::Expr, r: &ast::Expr) -> Option<(Expr, Expr)> {
        let mark = self.diags.len();
        let (a, b) = (self.expr(l, None), self.expr(r, None));
        if let (Some(a), Some(b)) = (&a, &b)
            && let (Some(x), Some(y)) = (as_scalar(&a.t), as_scalar(&b.t))
            && let Some(common) = unify_numeric(x, y)
            && (x != common || y != common)
        {
            self.diags.truncate(mark);
            let want = scalar(common);
            let (a, b) = (self.expr(l, Some(&want)), self.expr(r, Some(&want)));
            return Some((a?, b?));
        }
        Some((a?, b?))
    }

    fn mismatch(&mut self, op: BinOp, a: &Expr, b: &Expr, span: Span) -> Option<Expr> {
        let (x, y) = (self.tname(&a.t), self.tname(&b.t));
        self.report(
            Diagnostic::error(
                "E0902",
                format!("`{}` cannot be applied to `{x}` and `{y}`", op.text()),
                span,
            )
            .with_help("operand types must agree; only `int` to `long`, `float` or `decimal` and `long` to `decimal` convert"),
        );
        None
    }

    fn unsupported(&mut self, op: BinOp, t: &ExprType, span: Span, help: &str) -> Option<Expr> {
        let shown = self.tname(t);
        self.report(
            Diagnostic::error(
                "E0905",
                format!("`{}` is not supported on `{shown}`", op.text()),
                span,
            )
            .with_help(help),
        );
        None
    }

    /// Brings two operands to one type: numbers by widening, everything else must agree.
    fn unified(&mut self, op: BinOp, a: Expr, b: Expr, span: Span) -> Option<(Expr, Expr)> {
        if a.t == b.t {
            return Some((a, b));
        }
        if let (Some(x), Some(y)) = (as_scalar(&a.t), as_scalar(&b.t))
            && let Some(common) = unify_numeric(x, y)
        {
            let to = scalar(common);
            return Some((coerce(a, &to), coerce(b, &to)));
        }
        self.mismatch(op, &a, &b, span)?;
        None
    }

    fn equality(&mut self, op: BinOp, a: Expr, b: Expr, span: Span) -> Option<Expr> {
        let (a, b) = self.unified(op, a, b, span)?;
        if !self.supports_equality(&a.t) {
            return self.unsupported(
                op,
                &a.t,
                span,
                "models, unions, lists, maps, `bytes` and `json` cannot be compared; compare fields",
            );
        }
        if is_float(&a.t) {
            self.report(
                Diagnostic::warning(
                    "W0901",
                    "`==` and `!=` on `float` compare bits exactly",
                    span,
                )
                .with_help("compare with `<` and `>` against a bound, or use `decimal`"),
            );
        }
        Some(bin(op, boolean(), a, b))
    }

    fn supports_equality(&self, t: &ExprType) -> bool {
        match t {
            ExprType::Duration => true,
            ExprType::Type(Type::Scalar(s)) => !matches!(s, Scalar::Bytes | Scalar::Json),
            ExprType::Type(Type::Decl(d)) => self.prog.decls[d.index()].kind == DKind::Enum,
            ExprType::Type(Type::List(_) | Type::Map(_) | Type::Page(_)) => false,
        }
    }

    fn ordering(&mut self, op: BinOp, a: Expr, b: Expr, span: Span) -> Option<Expr> {
        let (a, b) = self.unified(op, a, b, span)?;
        let ordered = is_numeric(&a.t)
            || a.t == ExprType::Duration
            || matches!(
                as_scalar(&a.t),
                Some(Scalar::Date | Scalar::Datetime | Scalar::Time)
            );
        if !ordered {
            let help = if is_string(&a.t) {
                "strings are compared with `==`, `!=`, `in`, `startsWith`, `endsWith` and `contains`"
            } else {
                "only numbers, dates, datetimes, times and durations can be ordered"
            };
            return self.unsupported(op, &a.t, span, help);
        }
        Some(bin(op, boolean(), a, b))
    }

    fn arithmetic(&mut self, op: BinOp, a: Expr, b: Expr, span: Span) -> Option<Expr> {
        if is_numeric(&a.t) || is_numeric(&b.t) {
            let (a, b) = self.unified(op, a, b, span)?;
            if !is_numeric(&a.t) {
                return self.mismatch(op, &a, &b, span);
            }
            if matches!(op, BinOp::Div | BinOp::Rem) && is_decimal(&a.t) {
                return self.unsupported(
                    op,
                    &a.t,
                    span,
                    "decimal division needs a rounding rule; write `a <= b * 2`, not `a / 2 <= b`",
                );
            }
            let t = a.t.clone();
            return Some(bin(op, t, a, b));
        }
        match self.time_result(op, &a.t, &b.t) {
            Some(t) => Some(bin(op, t, a, b)),
            None => self.mismatch(op, &a, &b, span),
        }
    }

    /// Result type of `+` and `-` over datetimes, dates and durations.
    fn time_result(&self, op: BinOp, a: &ExprType, b: &ExprType) -> Option<ExprType> {
        let kind = |t: &ExprType| {
            if t == &ExprType::Duration {
                return Some("duration");
            }
            match as_scalar(t) {
                Some(Scalar::Datetime) => Some("datetime"),
                Some(Scalar::Date) => Some("date"),
                _ => None,
            }
        };
        match (op, kind(a)?, kind(b)?) {
            (BinOp::Add | BinOp::Sub, "datetime", "duration")
            | (BinOp::Add, "duration", "datetime") => Some(scalar(Scalar::Datetime)),
            (BinOp::Add | BinOp::Sub, "duration", "duration")
            | (BinOp::Sub, "datetime", "datetime")
            | (BinOp::Sub, "date", "date") => Some(ExprType::Duration),
            _ => None,
        }
    }

    /// `x in list` and `key in map`.
    fn membership(&mut self, l: &ast::Expr, r: &ast::Expr, span: Span) -> Option<Expr> {
        let (a, b) = if is_flex(l) {
            let b = self.expr(r, None)?;
            let want = list_elem(&b.t).map(of);
            (self.expr(l, want.as_ref())?, b)
        } else {
            let a = self.expr(l, None)?;
            let Some(elem) = as_type(&a.t) else {
                self.report(
                    Diagnostic::error("E0902", "a duration cannot be tested with `in`", span)
                        .with_help("durations have no lists; compare with `==`, `<` or `>`"),
                );
                return None;
            };
            let want = of(Type::List(Box::new(elem.clone())));
            let b = self.expr(r, Some(&want))?;
            (a, b)
        };
        self.membership_types(a, b, span)
    }

    fn membership_types(&mut self, a: Expr, b: Expr, span: Span) -> Option<Expr> {
        match &b.t {
            ExprType::Type(Type::Map(_)) => {
                if !is_string(&a.t) {
                    let shown = self.tname(&a.t);
                    self.err(
                        "E0902",
                        format!("the key of `in` over a map is a `string`, found `{shown}`"),
                        span,
                    );
                    return None;
                }
                Some(bin(BinOp::In, boolean(), a, b))
            }
            ExprType::Type(Type::List(elem)) => {
                let elem_t = of((**elem).clone());
                self.list_membership(a, b, &elem_t, span)
            }
            ExprType::Type(_) | ExprType::Duration => {
                let shown = self.tname(&b.t);
                self.err(
                    "E0902",
                    format!("the right side of `in` must be a list or a map, found `{shown}`"),
                    span,
                );
                None
            }
        }
    }

    fn list_membership(&mut self, a: Expr, b: Expr, elem: &ExprType, span: Span) -> Option<Expr> {
        if !self.supports_equality(elem) {
            return self.unsupported(
                BinOp::In,
                elem,
                span,
                "the elements of the list cannot be compared with `==`",
            );
        }
        let a = if &a.t == elem {
            a
        } else if let (Some(x), Some(y)) = (as_scalar(&a.t), as_scalar(elem))
            && unify_numeric(x, y) == Some(y)
        {
            coerce(a, elem)
        } else {
            self.mismatch(BinOp::In, &a, &b, span)?;
            return None;
        };
        Some(bin(BinOp::In, boolean(), a, b))
    }
}

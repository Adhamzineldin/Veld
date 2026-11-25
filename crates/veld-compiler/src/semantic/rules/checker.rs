//! The expression checker: literals, unary operators, lists and the shared plumbing. Binary
//! operators are in `ops`, names and guards in `paths`, built-in functions in `calls`.

use std::collections::BTreeSet;

use super::types::{
    as_scalar, boolean, coerce, decl_type, is_bool, is_numeric, list_elem, lit, node, of, scalar,
    unify_numeric,
};
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{DKind, Program, RField};
use crate::syntax::ast::{self, ExprKind as A, UnOp};
use veld_ir::rules::{EnumLit, Expr, ExprKind, ExprLit, ExprType};
use veld_ir::{Scalar, Type};

pub(super) struct Checker<'p, 'a, 'd> {
    pub prog: &'p Program<'a>,
    pub module: usize,
    /// The local scope: the fields of the declaration, inherited ones included.
    pub fields: &'p [RField],
    /// Names of fields that failed to resolve (reported elsewhere): uses stay silent.
    pub poisoned: BTreeSet<String>,
    pub diags: &'d mut Vec<Diagnostic>,
    /// Enclosing lambda variables with the type of their element.
    pub lambdas: Vec<(String, Type)>,
    /// Paths known to be present at this point of the expression (`spec/rules.md` section 3.6).
    pub known: BTreeSet<String>,
    /// For each enclosing comprehension: is its list bounded by `@length(max: n)`?
    pub bounded: Vec<bool>,
    pub reads: Vec<String>,
    pub reads_readonly: bool,
}

/// True for an expression that takes its type from the other operand: a numeric or enum
/// literal, possibly negated.
pub(super) fn is_flex(e: &ast::Expr) -> bool {
    match &e.kind {
        A::Int(_) | A::Float { .. } | A::EnumLit(_) => true,
        A::Unary {
            op: UnOp::Neg,
            operand,
        } => is_flex(operand),
        A::Bool(_)
        | A::Str(_)
        | A::Name(_)
        | A::Field { .. }
        | A::Call { .. }
        | A::List(_)
        | A::Unary { .. }
        | A::Binary { .. } => false,
    }
}

impl Checker<'_, '_, '_> {
    pub fn err(&mut self, code: &'static str, msg: impl Into<String>, span: Span) {
        self.diags.push(Diagnostic::error(code, msg, span));
    }

    pub fn report(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    pub fn tname(&self, t: &ExprType) -> String {
        match t {
            ExprType::Duration => "duration".to_string(),
            ExprType::Type(t) => self.prog.type_name(t),
        }
    }

    /// Types `e` and requires `bool`.
    pub fn bool_expr(&mut self, e: &ast::Expr, what: &str) -> Option<Expr> {
        let typed = self.expr(e, Some(&boolean()))?;
        if is_bool(&typed.t) {
            return Some(typed);
        }
        let found = self.tname(&typed.t);
        self.err(
            "E0902",
            format!("{what} must be a `bool`, found `{found}`"),
            e.span,
        );
        None
    }

    /// Types one expression. `want` is a hint for literals only: it never changes the type of
    /// anything that has a type of its own.
    pub fn expr(&mut self, e: &ast::Expr, want: Option<&ExprType>) -> Option<Expr> {
        match &e.kind {
            A::Bool(b) => Some(lit(boolean(), ExprLit::Bool(*b))),
            A::Str(s) => Some(lit(scalar(Scalar::String), ExprLit::String(s.clone()))),
            A::Int(n) => self.int_lit(*n, want, e.span),
            A::Float { value, text } => self.float_lit(*value, text, want, e.span),
            A::EnumLit(name) => self.enum_lit(&name.name, want, e.span),
            A::Name(_) | A::Field { .. } => self.path_expr(e),
            A::Call { name, args } => self.call(name, args, e.span),
            A::List(items) => self.list_lit(items, want, e.span),
            A::Unary { op, operand } => self.unary(*op, operand, want, e.span),
            A::Binary { op, l, r } => self.binary(*op, l, r, e.span),
        }
    }

    fn int_lit(&mut self, n: i64, want: Option<&ExprType>, span: Span) -> Option<Expr> {
        let fits_int = i32::try_from(n).is_ok();
        let target = want
            .and_then(as_scalar)
            .filter(|s| {
                matches!(
                    s,
                    Scalar::Int | Scalar::Long | Scalar::Float | Scalar::Decimal
                )
            })
            .unwrap_or(if fits_int { Scalar::Int } else { Scalar::Long });
        let literal = match target {
            Scalar::Int if !fits_int => {
                self.err(
                    "E0908",
                    format!("`{n}` does not fit in a 32-bit `int`"),
                    span,
                );
                return None;
            }
            Scalar::Long => ExprLit::Long(n),
            #[allow(clippy::cast_precision_loss)]
            Scalar::Float if n.unsigned_abs() <= (1 << 53) => ExprLit::Float(n as f64),
            Scalar::Float => {
                self.err(
                    "E0908",
                    format!("`{n}` cannot be represented exactly as a `float`"),
                    span,
                );
                return None;
            }
            Scalar::Decimal => ExprLit::Decimal(n.to_string()),
            Scalar::Int
            | Scalar::String
            | Scalar::Bool
            | Scalar::Date
            | Scalar::Datetime
            | Scalar::Time
            | Scalar::Uuid
            | Scalar::Bytes
            | Scalar::Json => ExprLit::Int(n),
        };
        Some(lit(scalar(target), literal))
    }

    fn float_lit(
        &mut self,
        value: f64,
        text: &str,
        want: Option<&ExprType>,
        span: Span,
    ) -> Option<Expr> {
        if want.and_then(as_scalar) == Some(Scalar::Float) {
            return Some(lit(scalar(Scalar::Float), ExprLit::Float(value)));
        }
        if text.contains(['e', 'E']) {
            self.report(
                Diagnostic::error(
                    "E0908",
                    format!("`{text}` cannot be a `decimal`: a decimal has no exponent"),
                    span,
                )
                .with_help("write the digits out in full, or compare against a `float` field"),
            );
            return None;
        }
        Some(lit(
            scalar(Scalar::Decimal),
            ExprLit::Decimal(text.to_string()),
        ))
    }

    fn enum_lit(&mut self, name: &str, want: Option<&ExprType>, span: Span) -> Option<Expr> {
        let enum_decl = match want {
            Some(ExprType::Type(Type::Decl(d)))
                if self.prog.decls[d.index()].kind == DKind::Enum =>
            {
                Some(d.index())
            }
            _ => None,
        };
        let Some(d) = enum_decl else {
            self.report(
                Diagnostic::error(
                    "E0902",
                    format!("cannot tell which enum `.{name}` belongs to"),
                    span,
                )
                .with_help("compare it with a value of the enum, e.g. `status == .paid`"),
            );
            return None;
        };
        let info = &self.prog.decls[d];
        if let Some(v) = info.res.variants.iter().find(|v| v.name == name) {
            let (name, wire) = (v.name.clone(), v.wire.clone());
            let decl = match decl_type(d) {
                ExprType::Type(Type::Decl(id)) => id,
                ExprType::Type(_) | ExprType::Duration => return None,
            };
            return Some(lit(
                decl_type(d),
                ExprLit::Enum(EnumLit { decl, name, wire }),
            ));
        }
        let all: Vec<&str> = info.res.variants.iter().map(|v| v.name.as_str()).collect();
        let diag = Diagnostic::error(
            "E0901",
            format!("enum `{}` has no variant `{name}`", info.name),
            span,
        )
        .did_you_mean(
            name,
            all.iter().copied(),
            Some(format!("variants: {}", all.join(", "))),
        );
        self.report(diag);
        None
    }

    fn unary(
        &mut self,
        op: UnOp,
        operand: &ast::Expr,
        want: Option<&ExprType>,
        span: Span,
    ) -> Option<Expr> {
        match op {
            UnOp::Not { .. } => {
                let inner = self.bool_expr(operand, "the operand of `!`")?;
                Some(node(boolean(), ExprKind::Not(Box::new(inner))))
            }
            UnOp::Neg => {
                let inner = self.expr(operand, want)?;
                if !(is_numeric(&inner.t) || inner.t == ExprType::Duration) {
                    let found = self.tname(&inner.t);
                    self.err(
                        "E0902",
                        format!("unary `-` needs a number or a duration, found `{found}`"),
                        span,
                    );
                    return None;
                }
                Some(node(inner.t.clone(), ExprKind::Neg(Box::new(inner))))
            }
        }
    }

    /// A list literal. The element type comes from `want`, else from the first element that has
    /// a type of its own, else from the defaults of its literals.
    fn list_lit(
        &mut self,
        items: &[ast::Expr],
        want: Option<&ExprType>,
        span: Span,
    ) -> Option<Expr> {
        let hint = want.and_then(list_elem).map(of);
        let elem = match hint {
            Some(t) => t,
            None => self.infer_elem(items, span)?,
        };
        let mut out = Vec::with_capacity(items.len());
        let mut ok = true;
        for item in items {
            match self.list_item(item, &elem) {
                Some(t) => out.push(t),
                None => ok = false,
            }
        }
        let ExprType::Type(elem_ty) = elem else {
            self.err("E0902", "a list of durations is not supported", span);
            return None;
        };
        ok.then(|| node(of(Type::List(Box::new(elem_ty))), ExprKind::List(out)))
    }

    fn infer_elem(&mut self, items: &[ast::Expr], span: Span) -> Option<ExprType> {
        let Some(first) = items.iter().find(|i| !is_flex(i)).or_else(|| items.first()) else {
            self.report(
                Diagnostic::error(
                    "E0902",
                    "cannot tell the element type of an empty list",
                    span,
                )
                .with_help("compare against a list in the other operand, e.g. `x in []`"),
            );
            return None;
        };
        let mark = self.diags.len();
        let t = self.expr(first, None).map(|e| e.t)?;
        self.diags.truncate(mark);
        Some(t)
    }

    fn list_item(&mut self, item: &ast::Expr, elem: &ExprType) -> Option<Expr> {
        let typed = self.expr(item, Some(elem))?;
        if &typed.t == elem {
            return Some(typed);
        }
        let exact = matches!(
            (as_scalar(&typed.t), as_scalar(elem)),
            (Some(a), Some(b)) if unify_numeric(a, b) == Some(b)
        );
        if exact {
            return Some(coerce(typed, elem));
        }
        let (found, expected) = (self.tname(&typed.t), self.tname(elem));
        self.err(
            "E0902",
            format!("list element is `{found}`, the list holds `{expected}`"),
            item.span,
        );
        None
    }
}

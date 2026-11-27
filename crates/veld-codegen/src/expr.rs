//! Shared traversal for printing rule expressions (`spec/rules.md` sections 3, 8 and 13).
//!
//! [`print_expr`] walks an [`Expr`] bottom-up and calls one [`ExprPrinter`] hook per node
//! with the already-printed children. A hook only turns text into target code: it never
//! decides a type, because every node carries its type in the IR.
//!
//! Error policy: `print_expr` is infallible. The one malformed shape the IR can express
//! here, a comprehension whose list operand is not list- or page-typed, is a compiler bug
//! that generator validation reports. The traversal still prints the body, using `json` as
//! the best-effort element type, instead of panicking.

use veld_ir::{BinExpr, CallExpr, DeclKind, Expr, ExprKind, ExprType, Ir, QuantExpr, Scalar, Type};
use veld_ir::{Builtin, ExprLit, Op, PathRef, Quantifier};

/// A printed sub-expression and how tightly it binds in the target language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Printed {
    pub code: String,
    pub prec: u8,
}

impl Printed {
    /// Highest precedence: an atom never needs parentheses.
    pub const ATOM: u8 = u8::MAX;

    #[must_use]
    pub fn atom(code: impl Into<String>) -> Self {
        Self::new(code, Self::ATOM)
    }

    #[must_use]
    pub fn new(code: impl Into<String>, prec: u8) -> Self {
        Self {
            code: code.into(),
            prec,
        }
    }

    /// The code, wrapped in parentheses when it binds less tightly than `min`.
    #[must_use]
    pub fn at(&self, min: u8) -> String {
        if self.prec < min {
            format!("({})", self.code)
        } else {
            self.code.clone()
        }
    }
}

/// Per-target hooks. `print_expr` does the traversal; a hook only turns already-printed
/// children into target code. Types come from the IR; a hook never decides a type.
pub trait ExprPrinter {
    fn lit(&mut self, lit: &ExprLit, ty: &ExprType) -> Printed;
    /// A value read. `ty` is the type of the value at the end of the path.
    fn path(&mut self, path: &PathRef, ty: &ExprType) -> Printed;
    fn has(&mut self, path: &PathRef) -> Printed;
    fn now(&mut self) -> Printed;
    fn not(&mut self, operand: Printed) -> Printed;
    fn neg(&mut self, operand: Printed, ty: &ExprType) -> Printed;
    /// `left` and `right` are the operand types; `out` is the node's type.
    fn bin(
        &mut self,
        op: Op,
        l: Printed,
        r: Printed,
        left: &ExprType,
        right: &ExprType,
        out: &ExprType,
    ) -> Printed;
    fn call(
        &mut self,
        f: Builtin,
        args: Vec<Printed>,
        arg_types: &[&ExprType],
        out: &ExprType,
    ) -> Printed;
    /// Called before the body of a comprehension is printed, and after it.
    fn enter_var(&mut self, var: &str, elem: &ExprType);
    fn exit_var(&mut self, var: &str);
    fn quant(
        &mut self,
        q: Quantifier,
        list: Printed,
        elem: &ExprType,
        var: &str,
        body: Printed,
        body_ty: &ExprType,
    ) -> Printed;
    fn list(&mut self, items: Vec<Printed>, elem: Option<&ExprType>) -> Printed;
    fn widen(&mut self, operand: Printed, from: &ExprType, to: &ExprType) -> Printed;
    fn old(&mut self, operand: Printed, ty: &ExprType) -> Printed;
}

/// Prints `expr` bottom-up through the hooks. Children are printed left to right.
pub fn print_expr<P: ExprPrinter + ?Sized>(p: &mut P, expr: &Expr) -> Printed {
    let ty = &expr.t;
    match &expr.e {
        ExprKind::Lit(l) => p.lit(l, ty),
        ExprKind::Path(path) => p.path(path, ty),
        ExprKind::Now(_) => p.now(),
        ExprKind::Not(x) => {
            let operand = print_expr(p, x);
            p.not(operand)
        }
        ExprKind::Neg(x) => {
            let operand = print_expr(p, x);
            p.neg(operand, ty)
        }
        ExprKind::Bin(b) => print_bin(p, b, ty),
        ExprKind::Has(path) => p.has(path),
        ExprKind::Call(c) => print_call(p, c, ty),
        ExprKind::Quant(q) => print_quant(p, q),
        ExprKind::List(items) => {
            let printed = items.iter().map(|i| print_expr(p, i)).collect();
            let elem = element_type(ty);
            p.list(printed, elem.as_ref())
        }
        ExprKind::Widen(x) => {
            let operand = print_expr(p, x);
            p.widen(operand, &x.t, ty)
        }
        ExprKind::Old(x) => {
            let operand = print_expr(p, x);
            p.old(operand, ty)
        }
    }
}

fn print_bin<P: ExprPrinter + ?Sized>(p: &mut P, b: &BinExpr, out: &ExprType) -> Printed {
    let l = print_expr(p, &b.l);
    let r = print_expr(p, &b.r);
    p.bin(b.op, l, r, &b.l.t, &b.r.t, out)
}

fn print_call<P: ExprPrinter + ?Sized>(p: &mut P, c: &CallExpr, out: &ExprType) -> Printed {
    let args = c.args.iter().map(|a| print_expr(p, a)).collect();
    let types: Vec<&ExprType> = c.args.iter().map(|a| &a.t).collect();
    p.call(c.f, args, &types, out)
}

fn print_quant<P: ExprPrinter + ?Sized>(p: &mut P, q: &QuantExpr) -> Printed {
    let list = print_expr(p, &q.list);
    let elem = element_type(&q.list.t).unwrap_or(ExprType::Type(Type::Scalar(Scalar::Json)));
    p.enter_var(&q.var, &elem);
    let body = print_expr(p, &q.body);
    p.exit_var(&q.var);
    p.quant(q.q, list, &elem, &q.var, body, &q.body.t)
}

/// The element type of a list- or page-typed expression type, if it is one.
#[must_use]
pub fn element_type(ty: &ExprType) -> Option<ExprType> {
    match ty {
        ExprType::Type(Type::List(t) | Type::Page(t)) => Some(ExprType::Type((**t).clone())),
        ExprType::Type(Type::Scalar(_) | Type::Map(_) | Type::Decl(_)) | ExprType::Duration => None,
    }
}

/// The kinds of value a printer switches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Bool,
    Int,
    Long,
    Float,
    Decimal,
    String,
    Uuid,
    Date,
    Datetime,
    Time,
    Duration,
    Bytes,
    Json,
    Enum,
    /// A model, error or union declaration.
    Model,
    List,
    Map,
    Page,
}

impl ValueKind {
    /// Classifies a type. A declaration id that is out of range, or that names a `const`
    /// (never a valid type), is classified as [`ValueKind::Json`], the most permissive kind.
    #[must_use]
    pub fn of(ty: &ExprType, ir: &Ir) -> ValueKind {
        match ty {
            ExprType::Duration => ValueKind::Duration,
            ExprType::Type(t) => Self::of_type(t, ir),
        }
    }

    fn of_type(ty: &Type, ir: &Ir) -> ValueKind {
        match ty {
            Type::Scalar(s) => Self::of_scalar(*s),
            Type::List(_) => ValueKind::List,
            Type::Map(_) => ValueKind::Map,
            Type::Page(_) => ValueKind::Page,
            Type::Decl(id) => match ir.decls.get(id.index()).map(|d| &d.kind) {
                Some(DeclKind::Enum(_)) => ValueKind::Enum,
                Some(
                    DeclKind::Model(_)
                    | DeclKind::Error(_)
                    | DeclKind::Union(_)
                    | DeclKind::Event(_),
                ) => ValueKind::Model,
                Some(DeclKind::Const(_)) | None => ValueKind::Json,
            },
        }
    }

    fn of_scalar(s: Scalar) -> ValueKind {
        match s {
            Scalar::String => ValueKind::String,
            Scalar::Int => ValueKind::Int,
            Scalar::Long => ValueKind::Long,
            Scalar::Float => ValueKind::Float,
            Scalar::Decimal => ValueKind::Decimal,
            Scalar::Bool => ValueKind::Bool,
            Scalar::Date => ValueKind::Date,
            Scalar::Datetime => ValueKind::Datetime,
            Scalar::Time => ValueKind::Time,
            Scalar::Uuid => ValueKind::Uuid,
            Scalar::Bytes => ValueKind::Bytes,
            Scalar::Json => ValueKind::Json,
        }
    }
}

#[cfg(test)]
mod tests;

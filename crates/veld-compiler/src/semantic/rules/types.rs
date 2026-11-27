//! Small helpers over expression types: predicates, the widening lattice and constructors.

use veld_ir::rules::{Expr, ExprKind, ExprLit, ExprType};
use veld_ir::{DeclId, Scalar, Type};

pub fn scalar(s: Scalar) -> ExprType {
    ExprType::Type(Type::Scalar(s))
}

pub fn boolean() -> ExprType {
    scalar(Scalar::Bool)
}

pub fn decl_type(index: usize) -> ExprType {
    ExprType::Type(Type::Decl(DeclId(u32::try_from(index).unwrap_or(u32::MAX))))
}

pub fn node(t: ExprType, e: ExprKind) -> Expr {
    Expr { t, e }
}

pub fn lit(t: ExprType, l: ExprLit) -> Expr {
    node(t, ExprKind::Lit(l))
}

/// The scalar behind a type, if it is one.
pub fn as_scalar(t: &ExprType) -> Option<Scalar> {
    match t {
        ExprType::Type(Type::Scalar(s)) => Some(*s),
        ExprType::Type(_) | ExprType::Duration => None,
    }
}

pub fn is_numeric(t: &ExprType) -> bool {
    matches!(
        as_scalar(t),
        Some(Scalar::Int | Scalar::Long | Scalar::Float | Scalar::Decimal)
    )
}

pub fn is_bool(t: &ExprType) -> bool {
    as_scalar(t) == Some(Scalar::Bool)
}

pub fn is_string(t: &ExprType) -> bool {
    as_scalar(t) == Some(Scalar::String)
}

pub fn is_float(t: &ExprType) -> bool {
    as_scalar(t) == Some(Scalar::Float)
}

pub fn is_decimal(t: &ExprType) -> bool {
    as_scalar(t) == Some(Scalar::Decimal)
}

/// The common type of two numeric scalars when the conversion is exact: `int` to `long`,
/// `int` to `float`, `int` or `long` to `decimal` (`spec/rules.md` section 3.3).
pub fn unify_numeric(a: Scalar, b: Scalar) -> Option<Scalar> {
    use Scalar::{Decimal, Float, Int, Long};
    match (a, b) {
        _ if a == b => Some(a),
        (Int, Long) | (Long, Int) => Some(Long),
        (Int, Float) | (Float, Int) => Some(Float),
        (Int | Long, Decimal) | (Decimal, Int | Long) => Some(Decimal),
        _ => None,
    }
}

/// Wraps `e` in an explicit `widen` node when its type is not already `to`.
pub fn coerce(e: Expr, to: &ExprType) -> Expr {
    if &e.t == to {
        return e;
    }
    node(to.clone(), ExprKind::Widen(Box::new(e)))
}

/// The element type of a list type.
pub fn list_elem(t: &ExprType) -> Option<Type> {
    match t {
        ExprType::Type(Type::List(i)) => Some((**i).clone()),
        ExprType::Type(_) | ExprType::Duration => None,
    }
}

pub fn as_type(t: &ExprType) -> Option<&Type> {
    match t {
        ExprType::Type(t) => Some(t),
        ExprType::Duration => None,
    }
}

pub fn of(t: Type) -> ExprType {
    ExprType::Type(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widening_is_exact_only() {
        use Scalar::{Decimal, Float, Int, Long};
        assert_eq!(unify_numeric(Int, Long), Some(Long));
        assert_eq!(unify_numeric(Long, Int), Some(Long));
        assert_eq!(unify_numeric(Int, Float), Some(Float));
        assert_eq!(unify_numeric(Int, Decimal), Some(Decimal));
        assert_eq!(unify_numeric(Long, Decimal), Some(Decimal));
        assert_eq!(unify_numeric(Long, Float), None);
        assert_eq!(unify_numeric(Decimal, Float), None);
        assert_eq!(unify_numeric(Float, Float), Some(Float));
    }

    #[test]
    fn coerce_adds_a_widen_node_only_when_needed() {
        let one = lit(scalar(Scalar::Int), ExprLit::Int(1));
        assert_eq!(coerce(one.clone(), &scalar(Scalar::Int)), one);
        let wide = coerce(one, &scalar(Scalar::Long));
        assert!(matches!(wide.e, ExprKind::Widen(_)));
        assert_eq!(wide.t, scalar(Scalar::Long));
    }
}

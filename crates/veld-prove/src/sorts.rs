//! How Veld types map to SMT sorts and to the range of their integer model.

use veld_ir::{DeclKind, ExprType, Ir, Scalar, Type};
use veld_rules::helpers::instant::{MS_PER_DAY, days_from_civil};

use crate::types::Sort;

pub const I32: (i128, i128) = (i32::MIN as i128, i32::MAX as i128);
pub const I64: (i128, i128) = (i64::MIN as i128, i64::MAX as i128);

/// The first and last day of the years 0000 to 9999, the span of the wire form.
fn day_span() -> (i128, i128) {
    (
        i128::from(days_from_civil(0, 1, 1)),
        i128::from(days_from_civil(9999, 12, 31)),
    )
}

/// The sort of a value of type `ty`, when it has one. Lists, maps, pages, `bytes`, `json` and
/// models are only ever asked for their length or their parts.
#[must_use]
pub fn sort_of_type(ir: &Ir, ty: &Type) -> Option<Sort> {
    match ty {
        Type::Scalar(s) => match s {
            Scalar::Bool => Some(Sort::Bool),
            Scalar::Int | Scalar::Long | Scalar::Date | Scalar::Datetime | Scalar::Time => {
                Some(Sort::Int)
            }
            Scalar::Float | Scalar::Decimal => Some(Sort::Real),
            Scalar::String => Some(Sort::Str),
            Scalar::Uuid => Some(Sort::Uuid),
            Scalar::Bytes | Scalar::Json => None,
        },
        Type::Decl(id) => match &ir.decl(*id).kind {
            DeclKind::Enum(_) => Some(Sort::Int),
            DeclKind::Model(_)
            | DeclKind::Error(_)
            | DeclKind::Union(_)
            | DeclKind::Const(_)
            | DeclKind::Event(_) => None,
        },
        Type::List(_) | Type::Map(_) | Type::Page(_) => None,
    }
}

/// The sort of an expression of type `t`; a `duration` is an integer count of milliseconds.
#[must_use]
pub fn sort_of_expr(ir: &Ir, t: &ExprType) -> Option<Sort> {
    match t {
        ExprType::Duration => Some(Sort::Int),
        ExprType::Type(ty) => sort_of_type(ir, ty),
    }
}

/// The values of an integer-sorted type that exist on the wire.
#[must_use]
pub fn domain(ir: &Ir, ty: &Type) -> Option<(i128, i128)> {
    match ty {
        Type::Scalar(Scalar::Int) => Some(I32),
        Type::Scalar(Scalar::Long) => Some(I64),
        Type::Scalar(Scalar::Date) => Some(day_span()),
        Type::Scalar(Scalar::Datetime) => {
            let (lo, hi) = day_span();
            let ms = i128::from(MS_PER_DAY);
            Some((lo * ms, (hi + 1) * ms - 1))
        }
        Type::Scalar(Scalar::Time) => Some((0, i128::from(MS_PER_DAY) - 1)),
        Type::Decl(id) => match &ir.decl(*id).kind {
            DeclKind::Enum(e) => Some((0, i128::try_from(e.variants.len()).ok()? - 1)),
            DeclKind::Model(_)
            | DeclKind::Error(_)
            | DeclKind::Union(_)
            | DeclKind::Const(_)
            | DeclKind::Event(_) => None,
        },
        Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => None,
    }
}

/// The range an arithmetic result of type `t` must stay in; leaving it is an evaluation error.
#[must_use]
pub fn checked_range(t: &ExprType) -> Option<(i128, i128)> {
    match t {
        ExprType::Duration => Some(I64),
        ExprType::Type(Type::Scalar(Scalar::Int)) => Some(I32),
        ExprType::Type(Type::Scalar(Scalar::Long | Scalar::Datetime)) => Some(I64),
        ExprType::Type(_) => None,
    }
}

/// True for a `float`.
#[must_use]
pub fn is_float(t: &ExprType) -> bool {
    matches!(t, ExprType::Type(Type::Scalar(Scalar::Float)))
}

/// True for a `decimal`.
#[must_use]
pub fn is_decimal(t: &ExprType) -> bool {
    matches!(t, ExprType::Type(Type::Scalar(Scalar::Decimal)))
}

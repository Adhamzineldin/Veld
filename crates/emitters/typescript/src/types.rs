//! IR type -> TypeScript type text.

use veld_ir::{Scalar, Type};

use crate::tree::FileCtx;

/// The TypeScript type for a scalar (`spec/targets/typescript.md` section 2).
#[must_use]
pub fn scalar_ts(s: Scalar) -> &'static str {
    match s {
        Scalar::Int | Scalar::Float => "number",
        Scalar::Bool => "boolean",
        Scalar::Json => "unknown",
        Scalar::String
        | Scalar::Long
        | Scalar::Decimal
        | Scalar::Date
        | Scalar::Datetime
        | Scalar::Time
        | Scalar::Uuid
        | Scalar::Bytes => "string",
    }
}

/// TypeScript type text for `ty`, recording the imports it needs.
pub fn ts_type(fc: &mut FileCtx<'_, '_>, ty: &Type) -> String {
    match ty {
        Type::Scalar(s) => scalar_ts(*s).to_string(),
        Type::List(t) => format!("{}[]", ts_type(fc, t)),
        Type::Map(t) => format!("Record<string, {}>", ts_type(fc, t)),
        Type::Page(t) => {
            let page = fc.rt("Page");
            format!("{page}<{}>", ts_type(fc, t))
        }
        Type::Decl(id) => fc.type_ref(*id),
        _ => "unknown".to_string(),
    }
}

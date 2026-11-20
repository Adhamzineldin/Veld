//! IR type -> Java type text, codec expressions, schema expressions and literals.

use veld_codegen::canonical::es_number;
use veld_ir::{Constraints, Field, Literal, Scalar, StringFormat, Type};

use crate::tree::FileCtx;
use crate::util::java_str;

/// Whether a field is represented by a nullable (boxed) type: optional fields, and fields with a
/// default, where `null` means "use the default".
#[must_use]
pub fn nullable(f: &Field) -> bool {
    f.optional || f.default.is_some()
}

/// The Java type of a scalar. `boxed` selects `Integer` over `int` and so on.
fn scalar_java(fc: &mut FileCtx<'_, '_>, s: Scalar, boxed: bool) -> String {
    let pick = |prim: &str, wrapper: &str| {
        if boxed {
            wrapper.to_string()
        } else {
            prim.to_string()
        }
    };
    match s {
        Scalar::String => "String".to_string(),
        Scalar::Int => pick("int", "Integer"),
        Scalar::Long => pick("long", "Long"),
        Scalar::Float => pick("double", "Double"),
        Scalar::Bool => pick("boolean", "Boolean"),
        Scalar::Decimal => fc.name("java.math.BigDecimal"),
        Scalar::Date => fc.name("java.time.LocalDate"),
        Scalar::Datetime => fc.name("java.time.Instant"),
        Scalar::Time => fc.name("java.time.LocalTime"),
        Scalar::Uuid => fc.name("java.util.UUID"),
        Scalar::Bytes => "byte[]".to_string(),
        Scalar::Json => fc.rt("Json"),
    }
}

/// Java type text for `ty`. Type arguments are always boxed; `boxed` applies to `ty` itself.
pub fn java_type(fc: &mut FileCtx<'_, '_>, ty: &Type, boxed: bool) -> String {
    match ty {
        Type::Scalar(s) => scalar_java(fc, *s, boxed),
        Type::List(t) => {
            let list = fc.name("java.util.List");
            format!("{list}<{}>", java_type(fc, t, true))
        }
        Type::Map(t) => {
            let map = fc.name("java.util.Map");
            format!("{map}<String, {}>", java_type(fc, t, true))
        }
        Type::Page(t) => {
            let page = fc.rt("Page");
            format!("{page}<{}>", java_type(fc, t, true))
        }
        Type::Decl(id) => fc.decl(*id),
        _ => fc.rt("Json"),
    }
}

/// A `Codec<T>` expression for `ty`.
pub fn codec_expr(fc: &mut FileCtx<'_, '_>, ty: &Type) -> String {
    let codec = fc.rt("Codec");
    match ty {
        Type::Scalar(s) => format!("{codec}.{}()", scalar_codec(*s)),
        Type::List(t) => format!("{codec}.list({})", codec_expr(fc, t)),
        Type::Map(t) => format!("{codec}.map({})", codec_expr(fc, t)),
        Type::Page(t) => format!("{codec}.page({})", codec_expr(fc, t)),
        Type::Decl(id) => format!("{}.codec()", fc.decl(*id)),
        _ => format!("{codec}.json()"),
    }
}

fn scalar_codec(s: Scalar) -> &'static str {
    match s {
        Scalar::String => "string",
        Scalar::Int => "int32",
        Scalar::Long => "int64",
        Scalar::Float => "float64",
        Scalar::Decimal => "decimal",
        Scalar::Bool => "bool",
        Scalar::Date => "date",
        Scalar::Datetime => "datetime",
        Scalar::Time => "time",
        Scalar::Uuid => "uuid",
        Scalar::Bytes => "bytes",
        Scalar::Json => "json",
    }
}

fn format_name(f: StringFormat) -> &'static str {
    match f {
        StringFormat::Email => "email",
        StringFormat::Uri => "uri",
        StringFormat::Hostname => "hostname",
        StringFormat::Ipv4 => "ipv4",
        StringFormat::Ipv6 => "ipv6",
    }
}

fn bounds(c: &Constraints) -> String {
    let mut out = String::new();
    if let Some(v) = c.min {
        out.push_str(&format!(".min({})", es_number(v)));
    }
    if let Some(v) = c.max {
        out.push_str(&format!(".max({})", es_number(v)));
    }
    if let Some(v) = c.min_len {
        out.push_str(&format!(".minLen({v})"));
    }
    if let Some(v) = c.max_len {
        out.push_str(&format!(".maxLen({v})"));
    }
    if let Some(p) = &c.pattern {
        out.push_str(&format!(".pattern({})", java_str(p)));
    }
    if let Some(f) = c.format {
        out.push_str(&format!(".format({})", java_str(format_name(f))));
    }
    out
}

/// A `Schema` expression for `ty` with `c` applied to the outermost node. References to other
/// declarations are `Schema.ref(() -> X.SCHEMA)` when `lazy` (inside declarations, which may be
/// recursive) and `X.SCHEMA` otherwise.
pub fn schema_expr(fc: &mut FileCtx<'_, '_>, ty: &Type, c: &Constraints, lazy: bool) -> String {
    let schema = fc.rt("Schema");
    let base = match ty {
        Type::Scalar(s) => format!("{schema}.{}()", scalar_codec(*s)),
        Type::List(t) => format!(
            "{schema}.list({})",
            schema_expr(fc, t, &Constraints::default(), lazy)
        ),
        Type::Map(t) => format!(
            "{schema}.map({})",
            schema_expr(fc, t, &Constraints::default(), lazy)
        ),
        Type::Page(t) => format!(
            "{schema}.page({})",
            schema_expr(fc, t, &Constraints::default(), lazy)
        ),
        Type::Decl(id) => {
            let name = fc.decl(*id);
            if lazy {
                format!("{schema}.ref(() -> {name}.SCHEMA)")
            } else {
                format!("{name}.SCHEMA")
            }
        }
        _ => format!("{schema}.json()"),
    };
    format!("{base}{}", bounds(c))
}

/// A `Json` expression for an IR literal (enum variants use the wire value).
pub fn json_literal(fc: &mut FileCtx<'_, '_>, l: &Literal) -> String {
    let json = fc.rt("Json");
    match l {
        Literal::String(s) => format!("{json}.of({})", java_str(s)),
        Literal::Int(i) => format!("{json}.of({i}L)"),
        Literal::Float(f) => format!("{json}.of({})", java_double(*f)),
        Literal::Bool(b) => format!("{json}.of({b})"),
        Literal::Null => format!("{json}.NULL"),
        Literal::EnumVariant { wire, .. } => format!("{json}.of({})", java_str(wire)),
        Literal::EmptyList => format!("{json}.array()"),
    }
}

/// A Java `double` literal: always has a `.` or an exponent so it is never read as an `int`.
pub fn java_double(f: f64) -> String {
    let s = es_number(f);
    if s.contains(['.', 'e']) {
        s
    } else {
        format!("{s}.0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_are_never_integer_literals() {
        assert_eq!(java_double(2.0), "2.0");
        assert_eq!(java_double(0.5), "0.5");
        assert_eq!(java_double(1e21), "1e+21");
    }
}

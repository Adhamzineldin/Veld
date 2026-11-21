//! The type mapping (`spec/targets/rust.md` section 2) and the static schema expressions.

use veld_ir::{Constraints, Literal, Scalar, StringFormat, Type};

use crate::tree::Tree;
use crate::util::{json_string, lit};

/// Where a type is written: the tree (for paths) and the module (so local names stay bare).
#[derive(Debug, Clone, Copy)]
pub struct Ctx<'t, 'a> {
    pub tree: &'t Tree<'a>,
    pub here: &'t str,
}

/// The Rust type of a contract type. `boxed` puts a declaration reference behind a `Box` (a
/// recursive field).
#[must_use]
pub fn rust_type(ctx: &Ctx<'_, '_>, t: &Type, boxed: bool) -> String {
    match t {
        Type::Scalar(s) => scalar_type(*s).to_string(),
        Type::List(inner) => format!("Vec<{}>", rust_type(ctx, inner, false)),
        Type::Map(inner) => format!(
            "std::collections::BTreeMap<String, {}>",
            rust_type(ctx, inner, false)
        ),
        Type::Page(inner) => format!("veld::Page<{}>", rust_type(ctx, inner, false)),
        Type::Decl(id) => {
            let path = ctx.tree.type_path(*id, ctx.here);
            if boxed { format!("Box<{path}>") } else { path }
        }
    }
}

/// The Rust type of a scalar.
#[must_use]
pub fn scalar_type(s: Scalar) -> &'static str {
    match s {
        Scalar::String
        | Scalar::Decimal
        | Scalar::Date
        | Scalar::Datetime
        | Scalar::Time
        | Scalar::Uuid => "String",
        Scalar::Int => "i32",
        Scalar::Long => "i64",
        Scalar::Float => "f64",
        Scalar::Bool => "bool",
        Scalar::Bytes => "Vec<u8>",
        Scalar::Json => "serde_json::Value",
    }
}

fn scalar_variant(s: Scalar) -> &'static str {
    match s {
        Scalar::String => "String",
        Scalar::Int => "Int",
        Scalar::Long => "Long",
        Scalar::Float => "Float",
        Scalar::Decimal => "Decimal",
        Scalar::Bool => "Bool",
        Scalar::Date => "Date",
        Scalar::Datetime => "Datetime",
        Scalar::Time => "Time",
        Scalar::Uuid => "Uuid",
        Scalar::Bytes => "Bytes",
        Scalar::Json => "Json",
    }
}

fn format_variant(f: StringFormat) -> &'static str {
    match f {
        StringFormat::Email => "Email",
        StringFormat::Uri => "Uri",
        StringFormat::Hostname => "Hostname",
        StringFormat::Ipv4 => "Ipv4",
        StringFormat::Ipv6 => "Ipv6",
    }
}

/// A float literal that Rust always reads as `f64`.
fn float(f: f64) -> String {
    format!("{f:?}")
}

/// `vs::Rules { .. }` for a field's or parameter's constraints.
#[must_use]
pub fn rules_expr(c: &Constraints) -> String {
    let mut parts = Vec::new();
    if let Some(v) = c.min {
        parts.push(format!("min: Some({})", float(v)));
    }
    if let Some(v) = c.max {
        parts.push(format!("max: Some({})", float(v)));
    }
    if let Some(v) = c.min_len {
        parts.push(format!("min_len: Some({v})"));
    }
    if let Some(v) = c.max_len {
        parts.push(format!("max_len: Some({v})"));
    }
    if let Some(p) = &c.pattern {
        parts.push(format!("pattern: Some({})", lit(p)));
    }
    if let Some(f) = c.format {
        parts.push(format!("format: Some(vs::Format::{})", format_variant(f)));
    }
    if parts.is_empty() {
        "vs::Rules::NONE".to_string()
    } else {
        format!("vs::Rules {{ {}, ..vs::Rules::NONE }}", parts.join(", "))
    }
}

/// A `vs::Schema` value for a contract type with constraints (constraints bind to the outermost
/// scalar, list or map).
#[must_use]
pub fn schema_expr(ctx: &Ctx<'_, '_>, t: &Type, c: &Constraints) -> String {
    match t {
        Type::Scalar(s) => format!(
            "vs::Schema::Scalar(vs::Scalar::{}, {})",
            scalar_variant(*s),
            rules_expr(c)
        ),
        Type::List(inner) => format!(
            "vs::Schema::List(&{}, {})",
            schema_expr(ctx, inner, &Constraints::default()),
            rules_expr(c)
        ),
        Type::Map(inner) => format!(
            "vs::Schema::Map(&{}, {})",
            schema_expr(ctx, inner, &Constraints::default()),
            rules_expr(c)
        ),
        Type::Page(inner) => format!(
            "vs::Schema::Page(&{})",
            schema_expr(ctx, inner, &Constraints::default())
        ),
        Type::Decl(id) => ctx.tree.schema_path(*id, ctx.here),
    }
}

/// A reference to a schema: `&<expr>`.
#[must_use]
pub fn schema_ref(ctx: &Ctx<'_, '_>, t: &Type, c: &Constraints) -> String {
    format!("&{}", schema_expr(ctx, t, c))
}

/// The JSON text of a default value, as the schema stores it.
#[must_use]
pub fn default_json(l: &Literal) -> Option<String> {
    Some(match l {
        Literal::String(s) => json_string(s),
        Literal::Int(i) => i.to_string(),
        Literal::Float(f) => float(*f),
        Literal::Bool(b) => b.to_string(),
        Literal::EnumVariant { wire, .. } => json_string(wire),
        Literal::EmptyList => "[]".to_string(),
        Literal::Null => return None,
    })
}

/// The Rust text of a constant's value.
#[must_use]
pub fn const_value(l: &Literal) -> Option<String> {
    Some(match l {
        Literal::String(s) => lit(s),
        Literal::Int(i) => i.to_string(),
        Literal::Float(f) => float(*f),
        Literal::Bool(b) => b.to_string(),
        Literal::Null | Literal::EnumVariant { .. } | Literal::EmptyList => return None,
    })
}

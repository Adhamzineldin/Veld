//! Schema objects: types, constraints, declarations.

use serde_json::{Map, Value, json};
use veld_ir::{
    Constraints, Decl, DeclId, DeclKind, Field, Ir, Literal, Scalar, StringFormat, Type,
};

/// Component name of a declaration: `<package>.<Name>`, made safe for
/// component keys (`@acme/billing` becomes `acme_billing`).
pub(super) fn schema_name(ir: &Ir, id: DeclId) -> String {
    let d = ir.decl(id);
    let pkg = &ir.package(d.package).name;
    let safe: String = pkg
        .trim_start_matches('@')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    format!("{safe}.{}", d.name)
}

pub(super) fn schema_ref(ir: &Ir, id: DeclId) -> Value {
    json!({ "$ref": format!("#/components/schemas/{}", schema_name(ir, id)) })
}

/// A JSON number that prints as an integer when it is one.
fn num(f: f64) -> Value {
    if f.fract() == 0.0 && f.abs() < 9.0e15 {
        #[allow(clippy::cast_possible_truncation)]
        return Value::from(f as i64);
    }
    Value::from(f)
}

fn scalar_schema(s: Scalar) -> Value {
    match s {
        Scalar::String => json!({ "type": "string" }),
        Scalar::Int => json!({ "type": "integer", "format": "int32" }),
        Scalar::Long => json!({ "type": "string", "format": "int64" }),
        Scalar::Float => json!({ "type": "number", "format": "double" }),
        Scalar::Decimal => json!({ "type": "string", "format": "decimal" }),
        Scalar::Bool => json!({ "type": "boolean" }),
        Scalar::Date => json!({ "type": "string", "format": "date" }),
        Scalar::Datetime => json!({ "type": "string", "format": "date-time" }),
        Scalar::Time => json!({ "type": "string", "format": "time" }),
        Scalar::Uuid => json!({ "type": "string", "format": "uuid" }),
        Scalar::Bytes => json!({ "type": "string", "contentEncoding": "base64" }),
        Scalar::Json => json!({}),
    }
}

/// The schema of a type. `Page<T>` is inlined.
pub(super) fn type_schema(ir: &Ir, ty: &Type) -> Value {
    match ty {
        Type::Scalar(s) => scalar_schema(*s),
        Type::List(t) => json!({ "type": "array", "items": type_schema(ir, t) }),
        Type::Map(t) => json!({ "type": "object", "additionalProperties": type_schema(ir, t) }),
        Type::Page(t) => json!({
            "type": "object",
            "properties": {
                "items": { "type": "array", "items": type_schema(ir, t) },
                "total": scalar_schema(Scalar::Long),
                "page": scalar_schema(Scalar::Int),
                "size": scalar_schema(Scalar::Int),
            },
            "required": ["items", "total", "page", "size"],
        }),
        Type::Decl(id) => schema_ref(ir, *id),
        _ => json!({}),
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

fn put(m: &mut Map<String, Value>, key: &str, v: Option<Value>) {
    if let Some(v) = v {
        m.insert(key.to_string(), v);
    }
}

/// Maps constraints onto the schema of `ty`. Length applies to the outermost
/// string, list or map; the rest is pushed down to the element schema.
pub(super) fn apply_constraints(schema: &mut Value, ty: &Type, c: &Constraints) {
    let Some(m) = schema.as_object_mut() else {
        return;
    };
    match ty {
        Type::Scalar(s) => scalar_constraints(m, *s, c),
        Type::List(inner) => {
            put(m, "minItems", c.min_len.map(Value::from));
            put(m, "maxItems", c.max_len.map(Value::from));
            let rest = Constraints { min_len: None, max_len: None, ..c.clone() };
            if let Some(items) = m.get_mut("items") {
                apply_constraints(items, inner, &rest);
            }
        }
        Type::Map(inner) => {
            put(m, "minProperties", c.min_len.map(Value::from));
            put(m, "maxProperties", c.max_len.map(Value::from));
            let rest = Constraints { min_len: None, max_len: None, ..c.clone() };
            if let Some(v) = m.get_mut("additionalProperties") {
                apply_constraints(v, inner, &rest);
            }
        }
        _ => {}
    }
}

fn scalar_constraints(m: &mut Map<String, Value>, s: Scalar, c: &Constraints) {
    if s.is_numeric() {
        put(m, "minimum", c.min.map(num));
        put(m, "maximum", c.max.map(num));
    }
    if s == Scalar::String {
        put(m, "minLength", c.min_len.map(Value::from));
        put(m, "maxLength", c.max_len.map(Value::from));
        put(m, "pattern", c.pattern.clone().map(Value::from));
        put(m, "format", c.format.map(|f| Value::from(format_name(f))));
    }
}

/// A literal in the wire form of `ty` (`long` and `decimal` are strings).
pub(super) fn literal_json(ty: &Type, l: &Literal) -> Value {
    let as_string = matches!(ty, Type::Scalar(Scalar::Long | Scalar::Decimal));
    match l {
        Literal::String(s) => Value::from(s.clone()),
        Literal::Int(n) if as_string => Value::from(n.to_string()),
        Literal::Int(n) => Value::from(*n),
        Literal::Float(f) if as_string => Value::from(f.to_string()),
        Literal::Float(f) => Value::from(*f),
        Literal::Bool(b) => Value::from(*b),
        Literal::Null => Value::Null,
        Literal::EnumVariant { wire, .. } => Value::from(wire.clone()),
        Literal::EmptyList => json!([]),
    }
}

/// The property schema of one field.
pub(super) fn field_schema(ir: &Ir, f: &Field) -> Value {
    let mut s = type_schema(ir, &f.ty);
    apply_constraints(&mut s, &f.ty, &f.constraints);
    if let Some(m) = s.as_object_mut() {
        put(m, "description", f.docs.clone().map(Value::from));
        if f.deprecated.is_some() {
            m.insert("deprecated".into(), Value::Bool(true));
        }
        if f.readonly {
            m.insert("readOnly".into(), Value::Bool(true));
        }
        put(m, "default", f.default.as_ref().map(|d| literal_json(&f.ty, d)));
        put(m, "examples", f.example.as_ref().map(|e| json!([literal_json(&f.ty, e)])));
    }
    s
}

fn object_of(ir: &Ir, fields: &[&Field]) -> Value {
    let mut props = Map::new();
    let mut required = Vec::new();
    for f in fields {
        props.insert(f.wire.clone(), field_schema(ir, f));
        if !f.optional && f.default.is_none() {
            required.push(Value::from(f.wire.clone()));
        }
    }
    let mut m = Map::new();
    m.insert("type".into(), Value::from("object"));
    m.insert("properties".into(), Value::Object(props));
    if !required.is_empty() {
        m.insert("required".into(), Value::Array(required));
    }
    Value::Object(m)
}

fn decorate(m: &mut Map<String, Value>, d: &Decl) {
    put(m, "description", d.docs.clone().map(Value::from));
    if d.deprecated.is_some() {
        m.insert("deprecated".into(), Value::Bool(true));
    }
    m.insert("x-veld-sig".into(), Value::from(d.sig.clone()));
}

fn model_schema(ir: &Ir, d: &Decl) -> Value {
    let own: Vec<&Field> = d.fields().iter().filter(|f| f.inherited_from.is_none()).collect();
    let parent = match &d.kind {
        DeclKind::Model(m) => m.parent,
        _ => None,
    };
    let mut out = match parent {
        Some(p) => json!({ "allOf": [schema_ref(ir, p), object_of(ir, &own)] }),
        None => object_of(ir, &own),
    };
    if let Some(m) = out.as_object_mut() {
        decorate(m, d);
    }
    out
}

fn union_arm(ir: &Ir, discriminator: &str, label: &str, decl: DeclId) -> Value {
    json!({
        "allOf": [
            schema_ref(ir, decl),
            {
                "type": "object",
                "properties": { discriminator: { "const": label } },
                "required": [discriminator],
            },
        ]
    })
}

/// The component schema of a declaration; `None` for constants.
pub(super) fn decl_schema(ir: &Ir, d: &Decl) -> Option<Value> {
    let mut out = match &d.kind {
        DeclKind::Model(_) | DeclKind::Error(_) => return Some(model_schema(ir, d)),
        DeclKind::Enum(e) => json!({
            "type": "string",
            "enum": e.variants.iter().map(|v| Value::from(v.wire.clone())).collect::<Vec<_>>(),
        }),
        DeclKind::Union(u) => {
            let arms: Vec<Value> = u
                .arms
                .iter()
                .map(|a| union_arm(ir, &u.discriminator, &a.label, a.decl))
                .collect();
            let mapping: Map<String, Value> = u
                .arms
                .iter()
                .map(|a| {
                    (
                        a.label.clone(),
                        Value::from(format!("#/components/schemas/{}", schema_name(ir, a.decl))),
                    )
                })
                .collect();
            json!({
                "oneOf": arms,
                "discriminator": { "propertyName": u.discriminator, "mapping": mapping },
            })
        }
        _ => return None,
    };
    if let Some(m) = out.as_object_mut() {
        decorate(m, d);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_as_integers_when_whole() {
        assert_eq!(num(3.0), json!(3));
        assert_eq!(num(1.5), json!(1.5));
        assert_eq!(num(-5.0), json!(-5));
    }

    #[test]
    fn scalar_mapping_follows_the_wire_format() {
        assert_eq!(scalar_schema(Scalar::Long), json!({"type": "string", "format": "int64"}));
        assert_eq!(scalar_schema(Scalar::Decimal), json!({"type": "string", "format": "decimal"}));
        assert_eq!(
            scalar_schema(Scalar::Bytes),
            json!({"type": "string", "contentEncoding": "base64"})
        );
        assert_eq!(scalar_schema(Scalar::Json), json!({}));
    }

    #[test]
    fn length_applies_to_the_outer_container_and_bounds_to_items() {
        let ty = Type::List(Box::new(Type::Scalar(Scalar::Int)));
        let mut s = json!({"type": "array", "items": {"type": "integer"}});
        let c = Constraints { min: Some(1.0), max_len: Some(4), ..Constraints::default() };
        apply_constraints(&mut s, &ty, &c);
        assert_eq!(s, json!({"type": "array", "items": {"type": "integer", "minimum": 1}, "maxItems": 4}));
    }

    #[test]
    fn string_constraints_become_keywords() {
        let ty = Type::Scalar(Scalar::String);
        let mut s = scalar_schema(Scalar::String);
        let c = Constraints {
            min_len: Some(2),
            max_len: Some(9),
            pattern: Some("^a$".into()),
            format: Some(StringFormat::Email),
            ..Constraints::default()
        };
        apply_constraints(&mut s, &ty, &c);
        assert_eq!(
            s,
            json!({"type": "string", "minLength": 2, "maxLength": 9, "pattern": "^a$", "format": "email"})
        );
    }

    #[test]
    fn long_literals_are_strings_on_the_wire() {
        let ty = Type::Scalar(Scalar::Long);
        assert_eq!(literal_json(&ty, &Literal::Int(5)), json!("5"));
        assert_eq!(literal_json(&Type::Scalar(Scalar::Int), &Literal::Int(5)), json!(5));
    }
}

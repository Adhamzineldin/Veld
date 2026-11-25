//! Typed run-time values and their conversion from the wire form.

use std::collections::BTreeMap;

use serde_json::Value as Json;
use veld_ir::{DeclKind, Ir, Scalar, Type};

use crate::error::InputError;
use crate::helpers::decimal::Decimal;
use crate::helpers::{instant, text};

/// A value as the evaluator handles it. Scalars are decoded; models, unions, pages and `json`
/// stay in wire form (`Raw`) and are read field by field through paths.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Long(i64),
    Float(f64),
    Decimal(Decimal),
    Str(String),
    /// A uuid in lower case.
    Uuid(String),
    /// Padded base64 text; only its byte length is ever used.
    Bytes(String),
    /// Days since the epoch.
    Date(i64),
    /// Milliseconds since the epoch, UTC.
    Datetime(i64),
    /// Milliseconds since midnight.
    Time(i64),
    /// A signed count of milliseconds.
    Duration(i64),
    /// The wire string of an enum variant.
    Enum(String),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Raw(Json),
}

impl Value {
    /// A short name for diagnostics.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Long(_) => "long",
            Value::Float(_) => "float",
            Value::Decimal(_) => "decimal",
            Value::Str(_) => "string",
            Value::Uuid(_) => "uuid",
            Value::Bytes(_) => "bytes",
            Value::Date(_) => "date",
            Value::Datetime(_) => "datetime",
            Value::Time(_) => "time",
            Value::Duration(_) => "duration",
            Value::Enum(_) => "enum",
            Value::List(_) => "list",
            Value::Map(_) => "map",
            Value::Raw(_) => "object",
        }
    }

    /// Reads the wire form `json` as a value of type `ty`.
    ///
    /// # Errors
    /// [`InputError`] when the JSON does not match the type.
    pub fn from_json(ir: &Ir, ty: &Type, json: &Json) -> Result<Value, InputError> {
        match ty {
            Type::Scalar(s) => scalar_from_json(*s, json),
            Type::List(inner) => {
                let items = json.as_array().ok_or_else(|| mismatch("list", json))?;
                let values: Result<Vec<Value>, InputError> = items
                    .iter()
                    .map(|j| Value::from_json(ir, inner, j))
                    .collect();
                Ok(Value::List(values?))
            }
            Type::Map(inner) => {
                let entries = json.as_object().ok_or_else(|| mismatch("map", json))?;
                let mut out = BTreeMap::new();
                for (k, j) in entries {
                    out.insert(k.clone(), Value::from_json(ir, inner, j)?);
                }
                Ok(Value::Map(out))
            }
            Type::Page(_) => object_raw(json),
            Type::Decl(id) => {
                let decl = ir
                    .decls
                    .get(id.index())
                    .ok_or_else(|| InputError::ir("a type refers to a missing declaration"))?;
                match &decl.kind {
                    DeclKind::Enum(e) => enum_from_json(&e.variants, json),
                    DeclKind::Model(_)
                    | DeclKind::Error(_)
                    | DeclKind::Union(_)
                    | DeclKind::Event(_) => object_raw(json),
                    DeclKind::Const(_) => Ok(Value::Raw(json.clone())),
                }
            }
        }
    }
}

fn kind_of(json: &Json) -> &'static str {
    match json {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

fn mismatch(expected: &str, found: &Json) -> InputError {
    InputError::mismatch(expected, kind_of(found))
}

fn object_raw(json: &Json) -> Result<Value, InputError> {
    if json.is_object() {
        Ok(Value::Raw(json.clone()))
    } else {
        Err(mismatch("object", json))
    }
}

fn enum_from_json(variants: &[veld_ir::Variant], json: &Json) -> Result<Value, InputError> {
    let s = json.as_str().ok_or_else(|| mismatch("enum string", json))?;
    if variants.iter().any(|v| v.wire == s) {
        Ok(Value::Enum(s.to_owned()))
    } else {
        Err(invalid("enum", s))
    }
}

fn invalid(ty: &'static str, text: &str) -> InputError {
    InputError::InvalidScalar {
        path: String::new(),
        ty,
        text: text.to_owned(),
    }
}

fn string_of<'a>(ty: &str, json: &'a Json) -> Result<&'a str, InputError> {
    json.as_str().ok_or_else(|| mismatch(ty, json))
}

fn long_from_text(s: &str) -> Option<i64> {
    let digits = s.strip_prefix('-').unwrap_or(s);
    let plain =
        !digits.is_empty() && digits.len() <= 19 && digits.bytes().all(|b| b.is_ascii_digit());
    if plain { s.parse().ok() } else { None }
}

fn scalar_from_json(s: Scalar, json: &Json) -> Result<Value, InputError> {
    let name = s.name();
    Ok(match s {
        Scalar::String => Value::Str(string_of(name, json)?.to_owned()),
        Scalar::Int => {
            let n = json.as_i64().ok_or_else(|| mismatch("int", json))?;
            let n = i32::try_from(n).map_err(|_| invalid("int", &n.to_string()))?;
            Value::Int(n)
        }
        Scalar::Long => {
            let t = string_of(name, json)?;
            Value::Long(long_from_text(t).ok_or_else(|| invalid("long", t))?)
        }
        Scalar::Float => {
            let x = json.as_f64().ok_or_else(|| mismatch("float", json))?;
            if !x.is_finite() {
                return Err(invalid("float", &x.to_string()));
            }
            Value::Float(x)
        }
        Scalar::Decimal => {
            let t = string_of(name, json)?;
            Value::Decimal(Decimal::parse(t).ok_or_else(|| invalid("decimal", t))?)
        }
        Scalar::Bool => Value::Bool(json.as_bool().ok_or_else(|| mismatch("bool", json))?),
        Scalar::Date => {
            let t = string_of(name, json)?;
            Value::Date(instant::parse_date(t).ok_or_else(|| invalid("date", t))?)
        }
        Scalar::Datetime => {
            let t = string_of(name, json)?;
            Value::Datetime(instant::parse_datetime(t).ok_or_else(|| invalid("datetime", t))?)
        }
        Scalar::Time => {
            let t = string_of(name, json)?;
            Value::Time(instant::parse_time(t).ok_or_else(|| invalid("time", t))?)
        }
        Scalar::Uuid => {
            let t = string_of(name, json)?;
            Value::Uuid(text::normalize_uuid(t).ok_or_else(|| invalid("uuid", t))?)
        }
        Scalar::Bytes => {
            let t = string_of(name, json)?;
            text::base64_byte_len(t).ok_or_else(|| invalid("bytes", t))?;
            Value::Bytes(t.to_owned())
        }
        Scalar::Json => Value::Raw(json.clone()),
    })
}

//! State and helpers shared by the per-file conversion of models, enums,
//! constants and services.

use std::collections::BTreeSet;

use super::ctx::Ctx;
use super::names::{is_prelude_name, sanitize_ident};
use super::v1::ast::{V1Type, Value};
use crate::fmt::text::quote;

/// What kind of literal a v1 type accepts.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Kind {
    Str,
    Int,
    Float,
    Bool,
    Decimal,
    Json,
    Enum,
    Other,
}

pub(super) struct FileConv<'a> {
    pub ctx: &'a Ctx,
    pub rel: &'a str,
    pub warnings: Vec<String>,
    /// Type names this file refers to.
    pub refs: BTreeSet<String>,
    /// Type names this file declares, generated ones included.
    pub locals: BTreeSet<String>,
    /// Declarations created by the migration, appended after the file's own.
    pub generated: Vec<String>,
    /// Middleware names already reported as auth.
    pub auth_noted: BTreeSet<String>,
}

/// A v1 scalar (or alias) as the v2 scalar name.
pub(super) fn scalar(name: &str) -> Option<&'static str> {
    Some(match name {
        "string" | "text" => "string",
        "int" | "int32" | "integer" => "int",
        "int64" | "long" => "long",
        "float" | "double" | "number" => "float",
        "decimal" => "decimal",
        "bool" | "boolean" => "bool",
        "date" => "date",
        "datetime" | "timestamp" => "datetime",
        "uuid" => "uuid",
        "json" | "any" | "object" => "json",
        _ => return None,
    })
}

pub(super) fn todo_comment(msg: &str) -> String {
    format!("// TODO(veld migrate): {msg}")
}

/// `//` comment lines for v1 comment text.
pub(super) fn comment_block(lines: &[String]) -> Vec<String> {
    lines.iter().map(|l| format!("//{l}")).collect()
}

/// A `///` doc line for each line of a description.
pub(super) fn doc_lines(desc: &Option<String>) -> Vec<String> {
    desc.as_deref()
        .map(|d| d.lines().map(|l| format!("/// {}", l.trim_end())).collect())
        .unwrap_or_default()
}

impl<'a> FileConv<'a> {
    pub fn new(ctx: &'a Ctx, rel: &'a str) -> FileConv<'a> {
        FileConv {
            ctx,
            rel,
            warnings: Vec::new(),
            refs: BTreeSet::new(),
            locals: BTreeSet::new(),
            generated: Vec::new(),
            auth_noted: BTreeSet::new(),
        }
    }

    pub fn warn(&mut self, msg: impl AsRef<str>) {
        self.warnings
            .push(format!("{}: {}", self.rel, msg.as_ref()));
    }

    /// The v2 text of a type name; records references to user types.
    pub fn type_name(&mut self, name: &str) -> String {
        if let Some(s) = scalar(name) {
            return s.to_string();
        }
        if !is_prelude_name(name) {
            self.refs.insert(name.to_string());
        }
        name.to_string()
    }

    pub fn type_text(&mut self, t: &V1Type) -> String {
        match t {
            V1Type::Named(n) => self.type_name(n),
            V1Type::List(i) => format!("{}[]", self.type_text(i)),
            V1Type::Map(i) => format!("Map<string, {}>", self.type_text(i)),
        }
    }

    pub fn kind_of(&self, t: &V1Type) -> Kind {
        let V1Type::Named(n) = t else {
            return Kind::Other;
        };
        match scalar(n) {
            Some("string" | "uuid" | "date" | "datetime") => Kind::Str,
            Some("int" | "long") => Kind::Int,
            Some("float") => Kind::Float,
            Some("decimal") => Kind::Decimal,
            Some("bool") => Kind::Bool,
            Some("json") => Kind::Json,
            _ if self.ctx.enums.contains(n) => Kind::Enum,
            _ => Kind::Other,
        }
    }

    /// A v1 value as a v2 literal of type `t`, or `None` when it does not fit.
    pub fn literal(&self, v: &Value, t: &V1Type) -> Option<String> {
        let raw = match v {
            Value::Str(s) | Value::Num(s) | Value::Ident(s) => s.as_str(),
        };
        match self.kind_of(t) {
            Kind::Str => Some(quote(raw)),
            Kind::Int => raw.parse::<i64>().ok().map(|n| n.to_string()),
            Kind::Float | Kind::Decimal => float_literal(raw, v, self.kind_of(t) == Kind::Decimal),
            Kind::Bool => matches!(raw, "true" | "false").then(|| raw.to_string()),
            Kind::Enum => Some(format!(".{}", sanitize_ident(raw))),
            Kind::Json => Some(match v {
                Value::Num(n) => n.clone(),
                Value::Ident(i) if matches!(i.as_str(), "true" | "false") => i.clone(),
                Value::Str(_) | Value::Ident(_) => quote(raw),
            }),
            Kind::Other => None,
        }
    }
}

fn float_literal(raw: &str, v: &Value, decimal: bool) -> Option<String> {
    if decimal && matches!(v, Value::Str(_)) {
        return Some(quote(raw));
    }
    let n: f64 = raw.parse().ok()?;
    if !n.is_finite() {
        return None;
    }
    Some(if raw.contains('.') && !raw.ends_with('.') {
        raw.to_string()
    } else {
        raw.trim_end_matches('.').to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(ctx: &Ctx) -> FileConv<'_> {
        FileConv::new(ctx, "f.veld")
    }

    fn named(n: &str) -> V1Type {
        V1Type::Named(n.into())
    }

    #[test]
    fn scalars_and_aliases_map_to_v2_names() {
        assert_eq!(scalar("int32"), Some("int"));
        assert_eq!(scalar("boolean"), Some("bool"));
        assert_eq!(scalar("Todo"), None);
    }

    #[test]
    fn type_text_maps_containers_and_records_references() {
        let ctx = Ctx::default();
        let mut c = conv(&ctx);
        let t = V1Type::Map(Box::new(V1Type::List(Box::new(named("Item")))));
        assert_eq!(c.type_text(&t), "Map<string, Item[]>");
        assert!(c.refs.contains("Item"));
        assert_eq!(c.type_text(&named("NotFound")), "NotFound");
        assert!(
            !c.refs.contains("NotFound"),
            "prelude names are not imported"
        );
    }

    #[test]
    fn literals_follow_the_field_type() {
        let mut ctx = Ctx::default();
        ctx.enums.insert("Role".into());
        let c = conv(&ctx);
        let lit = |v: Value, t: &str| c.literal(&v, &named(t));
        assert_eq!(lit(Value::Num("3".into()), "int"), Some("3".into()));
        assert_eq!(lit(Value::Str("3".into()), "int"), Some("3".into()));
        assert_eq!(lit(Value::Num("3".into()), "float"), Some("3".into()));
        assert_eq!(lit(Value::Num("2.5".into()), "decimal"), Some("2.5".into()));
        assert_eq!(lit(Value::Str("x".into()), "string"), Some("\"x\"".into()));
        assert_eq!(lit(Value::Num("7".into()), "string"), Some("\"7\"".into()));
        assert_eq!(
            lit(Value::Ident("true".into()), "bool"),
            Some("true".into())
        );
        assert_eq!(lit(Value::Ident("yes".into()), "bool"), None);
        assert_eq!(
            lit(Value::Ident("user".into()), "Role"),
            Some(".user".into())
        );
        assert_eq!(lit(Value::Ident("x".into()), "Mystery"), None);
        assert_eq!(lit(Value::Str("abc".into()), "int"), None);
    }
}

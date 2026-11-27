//! The constraints of an event, which the reference evaluator does not check offline
//! (`spec/events.md` section 12): a generated consumer rejects a value that breaks `@min`,
//! `@max`, `@length`, `@pattern` or `@format` as `shape`, and the reference accepts it.
//!
//! An event example that expects `rejected` and is delivered offline is then not a failure when
//! its value breaks a constraint: it is run live. [`findings`] says which constraints it breaks;
//! `@pattern` is decided with the portable regex engine of the reference helpers and `@format`
//! with the same checks as the generated Rust runtime.

use serde_json::Value as Json;
use veld_ir::{Constraints, DeclId, DeclKind, Ir, StringFormat, Type};

use crate::helpers::regex::Regex;

/// One constraint that a value breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The wire path of the value, `$.nights` or `$.lines[1].qty`.
    pub path: String,
    /// What it breaks, as text.
    pub what: String,
}

/// The constraints of event `event` that `value` (its wire form) breaks, in field order. A
/// constraint on a field that `value` does not have is not looked at, and neither is a `@pattern`
/// that the portable regex engine does not support.
#[must_use]
pub fn findings(ir: &Ir, event: DeclId, value: &Json) -> Vec<Finding> {
    let mut out = Vec::new();
    decl(ir, event, value, "$", &mut out);
    out
}

fn decl(ir: &Ir, id: DeclId, value: &Json, path: &str, out: &mut Vec<Finding>) {
    let Some(d) = ir.decls.get(id.index()) else {
        return;
    };
    match &d.kind {
        DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
            for f in d.fields() {
                let Some(child) = value.get(&f.wire) else {
                    continue;
                };
                let at = format!("{path}.{}", f.wire);
                constraints(&f.constraints, child, &at, out);
                walk(ir, &f.ty, child, &at, out);
            }
        }
        DeclKind::Union(u) => {
            let label = value.get(&u.discriminator).and_then(Json::as_str);
            if let Some(arm) = u.arms.iter().find(|a| Some(a.label.as_str()) == label) {
                decl(ir, arm.decl, value, path, out);
            }
        }
        DeclKind::Enum(_) | DeclKind::Const(_) => {}
    }
}

fn walk(ir: &Ir, ty: &Type, value: &Json, path: &str, out: &mut Vec<Finding>) {
    match ty {
        Type::Scalar(_) => {}
        Type::Decl(id) => decl(ir, *id, value, path, out),
        Type::List(t) => {
            for (i, item) in value.as_array().into_iter().flatten().enumerate() {
                walk(ir, t, item, &format!("{path}[{i}]"), out);
            }
        }
        Type::Map(t) => {
            for (k, item) in value.as_object().into_iter().flatten() {
                walk(ir, t, item, &format!("{path}.{k}"), out);
            }
        }
        Type::Page(t) => {
            if let Some(items) = value.get("items") {
                walk(
                    ir,
                    &Type::List(t.clone()),
                    items,
                    &format!("{path}.items"),
                    out,
                );
            }
        }
    }
}

fn number(value: &Json) -> Option<f64> {
    match value {
        Json::Number(n) => n.as_f64(),
        Json::String(s) => s.parse::<f64>().ok(),
        Json::Null | Json::Bool(_) | Json::Array(_) | Json::Object(_) => None,
    }
}

fn size(value: &Json) -> Option<u64> {
    let n = match value {
        Json::String(s) => s.chars().count(),
        Json::Array(a) => a.len(),
        Json::Object(o) => o.len(),
        Json::Null | Json::Bool(_) | Json::Number(_) => return None,
    };
    u64::try_from(n).ok()
}

fn constraints(c: &Constraints, value: &Json, path: &str, out: &mut Vec<Finding>) {
    let mut found = |what: String| {
        out.push(Finding {
            path: path.to_owned(),
            what,
        });
    };
    if let (Some(min), Some(n)) = (c.min, number(value))
        && n < min
    {
        found(format!("`@min({min})`"));
    }
    if let (Some(max), Some(n)) = (c.max, number(value))
        && n > max
    {
        found(format!("`@max({max})`"));
    }
    if let (Some(min), Some(n)) = (c.min_len, size(value))
        && n < min
    {
        found(format!("`@length(min: {min})`"));
    }
    if let (Some(max), Some(n)) = (c.max_len, size(value))
        && n > max
    {
        found(format!("`@length(max: {max})`"));
    }
    if let (Some(pattern), Json::String(text)) = (&c.pattern, value)
        && Regex::new(pattern).is_some_and(|r| !r.is_match(text))
    {
        found(format!("`@pattern(\"{pattern}\")`"));
    }
    if let (Some(format), Json::String(text)) = (c.format, value)
        && !format_ok(format, text)
    {
        found(format!("`@format({})`", format_name(format)));
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

/// The checks of the generated Rust runtime (`crates/emitters/rust/runtime/schema.rs`), which
/// the other targets follow.
fn format_ok(f: StringFormat, s: &str) -> bool {
    match f {
        StringFormat::Email => is_email(s),
        StringFormat::Uri => is_uri(s),
        StringFormat::Hostname => is_hostname(s),
        StringFormat::Ipv4 => s.parse::<std::net::Ipv4Addr>().is_ok() && is_dotted_quad(s),
        StringFormat::Ipv6 => s.parse::<std::net::Ipv6Addr>().is_ok(),
    }
}

fn is_dotted_quad(s: &str) -> bool {
    let p: Vec<&str> = s.split('.').collect();
    p.len() == 4
        && p.iter()
            .all(|x| !x.is_empty() && x.len() <= 3 && (*x == "0" || !x.starts_with('0')))
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    let bad = |c: char| c.is_whitespace() || c == '@';
    let labels: Vec<&str> = domain.split('.').collect();
    !local.is_empty()
        && !local.contains(bad)
        && labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty() && !l.contains(bad))
}

fn is_uri(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        && !rest.contains(char::is_whitespace)
}

fn is_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && !l.starts_with('-')
                && !l.ends_with('-')
        })
}

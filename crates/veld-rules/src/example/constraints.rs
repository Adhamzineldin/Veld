//! The constraints of an event, which the reference evaluator does not check offline
//! (`spec/events.md` section 12): a generated consumer rejects a value that breaks `@min`,
//! `@max`, `@length`, `@pattern` or `@format` as `shape`, and the reference accepts it.
//!
//! An event example that expects `rejected` and is delivered offline is then not a failure when
//! its value breaks (or may break) a constraint: it is run live. [`findings`] says which.

use serde_json::Value as Json;
use veld_ir::{Constraints, DeclId, DeclKind, Ir, Type};

/// One constraint that a value breaks, or may break.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The wire path of the value, `$.nights` or `$.lines[1].qty`.
    pub path: String,
    /// What it breaks, as text.
    pub what: String,
    /// False for a `@pattern` or `@format`, which the reference cannot evaluate: the value may or
    /// may not break it.
    pub decided: bool,
}

/// The constraints of event `event` that `value` (its wire form) breaks, in field order, those
/// that are decided first. A constraint on a field that `value` does not have is not looked at.
#[must_use]
pub fn findings(ir: &Ir, event: DeclId, value: &Json) -> Vec<Finding> {
    let mut out = Vec::new();
    decl(ir, event, value, "$", &mut out);
    out.sort_by_key(|f| !f.decided);
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
        _ => None,
    }
}

fn size(value: &Json) -> Option<u64> {
    let n = match value {
        Json::String(s) => s.chars().count(),
        Json::Array(a) => a.len(),
        Json::Object(o) => o.len(),
        _ => return None,
    };
    u64::try_from(n).ok()
}

fn constraints(c: &Constraints, value: &Json, path: &str, out: &mut Vec<Finding>) {
    let mut found = |what: String, decided: bool| {
        out.push(Finding {
            path: path.to_owned(),
            what,
            decided,
        });
    };
    if let (Some(min), Some(n)) = (c.min, number(value))
        && n < min
    {
        found(format!("`@min({min})`"), true);
    }
    if let (Some(max), Some(n)) = (c.max, number(value))
        && n > max
    {
        found(format!("`@max({max})`"), true);
    }
    if let (Some(min), Some(n)) = (c.min_len, size(value))
        && n < min
    {
        found(format!("`@length(min: {min})`"), true);
    }
    if let (Some(max), Some(n)) = (c.max_len, size(value))
        && n > max
    {
        found(format!("`@length(max: {max})`"), true);
    }
    if c.pattern.is_some() {
        found(
            "`@pattern`, which only a generated consumer checks".to_owned(),
            false,
        );
    }
    if c.format.is_some() {
        found(
            "`@format`, which only a generated consumer checks".to_owned(),
            false,
        );
    }
}

//! Platform phase P1 (`spec/platform.md` sections 2 to 5): field visibility, tenancy, the audit
//! log and rate limits. The server core calls into this file at fixed places of its pipeline;
//! everything here is a function of a schema, a wire value, the caller and the clock.

#![allow(unused_imports, dead_code)]

use std::collections::BTreeSet;
use std::sync::Mutex;

use serde_json::Value;

use super::schema::Schema;
use super::server::Principal;

/// What the platform features keep of one request. The clones of a request's context share it.
#[derive(Debug, Default)]
pub struct State {
    // @if visibility
    /// The wire paths that visibility dropped from what was sent, without the `$`.
    hidden: Mutex<BTreeSet<String>>,
    // @end
}

impl State {
    // @if visibility
    /// Notes what visibility dropped.
    pub(crate) fn note_hidden(&self, paths: &[String]) {
        if let Ok(mut hidden) = self.hidden.lock() {
            hidden.extend(paths.iter().map(|p| unrooted(p)));
        }
    }

    /// What visibility dropped so far, sorted, without duplicates.
    #[must_use]
    pub fn hidden(&self) -> Vec<String> {
        self.hidden.lock().map(|h| h.iter().cloned().collect()).unwrap_or_default()
    }
    // @end
}

/// A path without its root: `$.guestPhone` is `guestPhone`, `$[0].phone` is `[0].phone`.
fn unrooted(path: &str) -> String {
    let rest = path.strip_prefix('$').unwrap_or(path);
    rest.strip_prefix('.').unwrap_or(rest).to_string()
}

// @if visibility
// ---------------------------------------------------------------------------------------------
// Field visibility (`spec/platform.md` section 2)
// ---------------------------------------------------------------------------------------------

/// A declaration with `@visible` fields: the generated code decides, for one value, which of them
/// a caller may not see. A field is shown only to a caller of the principal model its expression
/// was checked against and only when the expression is true (it fails closed).
pub trait Visible {
    /// The wire names of every `@visible` field of the declaration.
    const FIELDS: &'static [&'static str];

    /// The wire names of the `@visible` fields that `caller` may not see at `now` (milliseconds
    /// since the epoch). The expression of every field reads the whole value.
    fn hidden(&self, caller: Option<&Principal>, now: i64) -> Vec<&'static str>;
}

/// The `show` function of a schema: decodes the wire record and asks the declaration. A record
/// that cannot be decoded hides all its `@visible` fields.
#[must_use]
pub fn show_record<T: Visible + super::error::VeldDecl>(wire: &Value, caller: Option<&Principal>, now: i64) -> Vec<&'static str> {
    let typed = T::schema().from_wire(wire, false);
    match serde_json::from_value::<T>(typed) {
        Ok(value) => value.hidden(caller, now),
        Err(_) => T::FIELDS.to_vec(),
    }
}

/// A value with the fields its caller may not see left out, and where they were.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub value: Value,
    /// The wire paths that were dropped, rooted at the `root` given, sorted, without duplicates.
    pub hidden: Vec<String>,
}

/// Applies visibility to `wire`, a wire value of `schema`, at any depth: nested models, lists,
/// maps, the `items` of a page and the arm of a union. Every `@visible` field of a record is
/// judged against the whole record before any is dropped; a field the record lacks is not
/// judged; members the declaration does not know are kept. `caller` is `None` for a request with
/// no caller of the principal model.
#[must_use]
pub fn show_value(schema: &Schema, wire: &Value, caller: Option<&Principal>, now: i64, root: &str) -> Shown {
    let mut dropped = BTreeSet::new();
    let value = show_node(schema, wire, (caller, now), root, &mut dropped);
    Shown { value, hidden: dropped.into_iter().collect() }
}

fn show_node(schema: &Schema, v: &Value, who: (Option<&Principal>, i64), path: &str, out: &mut BTreeSet<String>) -> Value {
    if v.is_null() {
        return Value::Null;
    }
    match schema {
        Schema::Scalar(..) | Schema::Enum(_) => v.clone(),
        Schema::List(of, _) => show_items(of, v, who, path, out),
        Schema::Map(of, _) => match v.as_object() {
            Some(entries) => Value::Object(entries.iter().map(|(k, e)| (k.clone(), show_node(of, e, who, &format!("{path}.{k}"), out))).collect()),
            None => v.clone(),
        },
        Schema::Page(of) => match v.as_object() {
            Some(obj) => {
                let mut kept = obj.clone();
                if let Some(items) = obj.get("items") {
                    kept.insert("items".to_string(), show_items(of, items, who, &format!("{path}.items"), out));
                }
                Value::Object(kept)
            }
            None => v.clone(),
        },
        Schema::Model(m) => show_record_node(m, v, who, path, out),
        Schema::Union(u) => {
            let label = v.get(u.discriminator).and_then(Value::as_str);
            match label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) {
                Some((_, arm)) => show_node(arm, v, who, path, out),
                None => v.clone(),
            }
        }
    }
}

fn show_items(of: &Schema, v: &Value, who: (Option<&Principal>, i64), path: &str, out: &mut BTreeSet<String>) -> Value {
    match v.as_array() {
        Some(list) => Value::Array(list.iter().enumerate().map(|(i, e)| show_node(of, e, who, &format!("{path}[{i}]"), out)).collect()),
        None => v.clone(),
    }
}

fn show_record_node(m: &super::schema::ModelSchema, v: &Value, who: (Option<&Principal>, i64), path: &str, out: &mut BTreeSet<String>) -> Value {
    let Some(obj) = v.as_object() else {
        return v.clone();
    };
    let hide: Vec<&str> = m.show.map_or_else(Vec::new, |show| show(v, who.0, who.1));
    let mut kept = obj.clone();
    for f in m.fields {
        let Some(child) = obj.get(f.wire) else {
            continue;
        };
        let here = format!("{path}.{}", f.wire);
        if hide.contains(&f.wire) {
            kept.remove(f.wire);
            out.insert(here);
        } else {
            kept.insert(f.wire.to_string(), show_node(f.schema, child, who, &here, out));
        }
    }
    Value::Object(kept)
}
// @end

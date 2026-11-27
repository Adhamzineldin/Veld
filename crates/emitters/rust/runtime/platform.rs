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
    // @if audit_log
    /// What the request had reached when it ended: the trail of its audit event.
    trail: Mutex<Trail>,
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
    // @if !visibility

    /// What visibility dropped so far: this server has no `@visible` field to drop.
    #[must_use]
    pub fn hidden(&self) -> Vec<String> {
        Vec::new()
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

// @if tenant_types
// ---------------------------------------------------------------------------------------------
// Tenants (`spec/platform.md` section 3)
// ---------------------------------------------------------------------------------------------

/// The type of a `@tenant` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantKind {
    Uuid,
    String,
    Int,
    Long,
}

/// The tenant of the caller: the value of the `@tenant` field of the principal, in wire form.
#[derive(Debug, Clone, PartialEq)]
pub struct Tenant {
    kind: TenantKind,
    value: Value,
}

impl Tenant {
    /// The tenant a principal's field holds; `None` when the field has no value (the caller has
    /// no tenant). For generated code.
    #[must_use]
    pub fn of<T: serde::Serialize>(kind: TenantKind, field: &T) -> Option<Tenant> {
        let value = serde_json::to_value(field).ok().filter(|v| !v.is_null())?;
        Some(Tenant { kind, value })
    }

    /// The text of the tenant: a `string` as it is, a `uuid` in lower case, an `int` or a `long`
    /// in decimal digits. It is what the key of a limit by tenant and the envelope attribute
    /// `veldtenant` carry.
    #[must_use]
    pub fn text(&self) -> String {
        tenant_text(self.kind, &self.value).unwrap_or_default()
    }

    /// Whether `have`, the tenant of a value, is this tenant, as values of the field's type.
    fn is(&self, have: &Value) -> bool {
        same_tenant(self.kind, have, &self.value)
    }
}

/// The text of a tenant value of a field of type `kind` (wire form: a `long` is a string).
#[must_use]
pub fn tenant_text(kind: TenantKind, value: &Value) -> Option<String> {
    match (kind, value) {
        (TenantKind::String | TenantKind::Long, Value::String(s)) => Some(s.clone()),
        (TenantKind::Uuid, Value::String(s)) => Some(s.to_ascii_lowercase()),
        (TenantKind::Int | TenantKind::Long, Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn number_of(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn same_tenant(kind: TenantKind, a: &Value, b: &Value) -> bool {
    match kind {
        TenantKind::String => matches!((a, b), (Value::String(x), Value::String(y)) if x == y),
        TenantKind::Uuid => matches!((a, b), (Value::String(x), Value::String(y)) if x.eq_ignore_ascii_case(y)),
        TenantKind::Int | TenantKind::Long => matches!((number_of(a), number_of(b)), (Some(x), Some(y)) if x == y),
    }
}
// @end
// @if tenancy

/// The `@tenant` field of a declaration: a value of it belongs to a caller whose tenant is
/// the value of the field.
#[derive(Debug, Clone, Copy)]
pub struct TenantMark {
    /// The wire name of the field.
    pub wire: &'static str,
    /// The id of the rule that a value of another tenant violates (`Field.tenant_id`).
    pub id: &'static str,
    pub kind: TenantKind,
}

/// Every tenant-scoped value of `wire`, a wire value of `schema`, that is not the caller's, at any
/// depth (nested models, lists, maps, the `items` of a page, the arm of a union), in the order of
/// the walk: one violation of the tenant rule of the model, with the path of the value from
/// `root` (`result[1]`, `result.items[0].guest`). A caller without a tenant owns nothing; a value
/// with none belongs to nobody.
#[must_use]
pub fn foreign_values(schema: &Schema, wire: &Value, caller: Option<&Tenant>, root: &str) -> Vec<super::rules::Violation> {
    let mut out = Vec::new();
    foreign_node(schema, wire, caller, root, &mut out);
    out
}

fn foreign_node(schema: &Schema, v: &Value, caller: Option<&Tenant>, path: &str, out: &mut Vec<super::rules::Violation>) {
    if v.is_null() {
        return;
    }
    match schema {
        Schema::Scalar(..) | Schema::Enum(_) => {}
        Schema::List(of, _) => foreign_items(of, v, caller, path, out),
        Schema::Map(of, _) => {
            for (k, e) in v.as_object().into_iter().flatten() {
                foreign_node(of, e, caller, &format!("{path}.{k}"), out);
            }
        }
        Schema::Page(of) => {
            if let Some(items) = v.get("items") {
                foreign_items(of, items, caller, &format!("{path}.items"), out);
            }
        }
        Schema::Model(m) => {
            let Some(obj) = v.as_object() else {
                return;
            };
            if let Some(mark) = &m.tenant {
                let mine = match (obj.get(mark.wire).filter(|t| !t.is_null()), caller) {
                    (Some(have), Some(caller)) => caller.is(have),
                    (_, _) => false,
                };
                if !mine {
                    out.push(super::rules::Violation {
                        rule: mark.id.to_string(),
                        path: path.to_string(),
                        fields: vec![mark.wire.to_string()],
                        message: "violates rule tenant".to_string(),
                        error: None,
                    });
                }
            }
            for f in m.fields {
                if let Some(child) = obj.get(f.wire) {
                    foreign_node(f.schema, child, caller, &format!("{path}.{}", f.wire), out);
                }
            }
        }
        Schema::Union(u) => {
            let label = v.get(u.discriminator).and_then(Value::as_str);
            if let Some((_, arm)) = label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) {
                foreign_node(arm, v, caller, path, out);
            }
        }
    }
}

fn foreign_items(of: &Schema, v: &Value, caller: Option<&Tenant>, path: &str, out: &mut Vec<super::rules::Violation>) {
    for (i, e) in v.as_array().into_iter().flatten().enumerate() {
        foreign_node(of, e, caller, &format!("{path}[{i}]"), out);
    }
}

/// Whether a loaded fact holds a tenant-scoped value that is not the caller's anywhere: such a
/// fact is **not found** for the caller. For generated code.
#[must_use]
pub fn foreign<T: serde::Serialize>(schema: &Schema, fact: &T, ctx: &super::server::RequestContext) -> bool {
    let Ok(typed) = serde_json::to_value(fact) else {
        return true;
    };
    let wire = schema.to_wire(&typed, false, false);
    !foreign_values(schema, &wire, ctx.tenant(), "$").is_empty()
}
// @end

// @if audit_log
// ---------------------------------------------------------------------------------------------
// The audit log (`spec/platform.md` section 4)
// ---------------------------------------------------------------------------------------------

use serde_json::Map;

use super::spec::EndpointSpec;

/// What the server tells about an audited endpoint (`Endpoints::audit_spec`).
#[derive(Debug, Clone, Copy)]
pub struct AuditSpec {
    /// The signatures of the errors the contract declares itself: only their data is a `result`
    /// (an error of the prelude has none: its detail is not for a log).
    pub errors: &'static [&'static str],
}

/// What happened to a request: `status`, and the name of the error when the answer is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditOutcome {
    pub status: u16,
    pub error: Option<String>,
}

/// The audit event of one request, the same value in every target (`spec/platform.md` section 4).
#[derive(Debug, Clone, PartialEq)]
pub struct AuditEvent {
    /// The operation signature.
    pub operation: String,
    /// The clock reading of the request: `YYYY-MM-DDTHH:MM:SS[.mmm]Z`.
    pub time: String,
    /// The caller, without its `@pii` and `@secret` fields; `None` when authentication failed and
    /// for an endpoint with no authentication.
    pub principal: Option<Value>,
    pub outcome: AuditOutcome,
    /// The parameters of step 4 by name, in wire form.
    pub parameters: Map<String, Value>,
    /// The facts loaded when the request ended, by name, in wire form.
    pub facts: Map<String, Value>,
    /// The body of the answer as it was sent; `None` for no body and for an error of the prelude.
    pub result: Option<Value>,
    /// The wire paths that `@visible` dropped from the result, sorted.
    pub hidden: Vec<String>,
    /// The wire paths of every `@pii` and `@secret` value left out, sorted.
    pub redacted: Vec<String>,
}

impl AuditEvent {
    /// The event as JSON: `principal` and `result` are left out when they are absent.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("operation".into(), Value::from(self.operation.as_str()));
        m.insert("time".into(), Value::from(self.time.as_str()));
        if let Some(principal) = &self.principal {
            m.insert("principal".into(), principal.clone());
        }
        let mut outcome = Map::new();
        outcome.insert("status".into(), Value::from(self.outcome.status));
        if let Some(error) = &self.outcome.error {
            outcome.insert("error".into(), Value::from(error.as_str()));
        }
        m.insert("outcome".into(), Value::Object(outcome));
        m.insert("parameters".into(), Value::Object(self.parameters.clone()));
        m.insert("facts".into(), Value::Object(self.facts.clone()));
        if let Some(result) = &self.result {
            m.insert("result".into(), result.clone());
        }
        m.insert("hidden".into(), Value::Array(self.hidden.iter().map(|p| Value::from(p.as_str())).collect()));
        m.insert("redacted".into(), Value::Array(self.redacted.iter().map(|p| Value::from(p.as_str())).collect()));
        Value::Object(m)
    }

    /// The RFC 8785 canonical JSON of the event.
    #[must_use]
    pub fn canonical(&self) -> String {
        super::canonical::canonical(&self.to_json())
    }
}

/// What a request had reached when it ended.
#[derive(Debug, Default)]
struct Trail {
    now: Option<std::time::SystemTime>,
    /// The caller in wire form, with the schema of its model.
    principal: Option<(Value, &'static Schema)>,
    parameters: Map<String, Value>,
    facts: Map<String, Value>,
    redacted: BTreeSet<String>,
    /// Why the event cannot be built: a value that does not have the shape of its type cannot be
    /// known to be clean.
    broken: Option<String>,
}

impl State {
    fn trail<R>(&self, f: impl FnOnce(&mut Trail) -> R) -> Option<R> {
        self.trail.lock().ok().map(|mut t| f(&mut t))
    }

    /// The clock reading of the request.
    pub(crate) fn note_now(&self, now: std::time::SystemTime) {
        self.trail(|t| t.now = Some(now));
    }

    /// The caller, as the authentication hook returned it, in wire form.
    pub(crate) fn note_principal(&self, wire: Value, schema: &'static Schema) {
        self.trail(|t| t.principal = Some((wire, schema)));
    }
}

/// Keeps what a value holds without its marked fields, and lists the paths that were left out.
fn keep(t: &mut Trail, schema: &Schema, wire: &Value, root: &str) -> Option<Value> {
    let mut paths = BTreeSet::new();
    match redact(schema, wire, root, &mut paths) {
        Ok(kept) => {
            t.redacted.extend(paths);
            Some(kept)
        }
        Err(why) => {
            t.broken.get_or_insert(why);
            None
        }
    }
}

/// Keeps the parameters of step 4 in wire form, redacted, for the audit event of the request.
pub(crate) fn note_parameters(ep: &EndpointSpec, args: &Map<String, Value>, state: &State) {
    state.trail(|t| {
        for p in ep.params.iter().filter(|p| !p.stream) {
            let Some(typed) = args.get(p.name) else {
                continue;
            };
            let wire = p.schema.to_wire(typed, true, false);
            if let Some(kept) = keep(t, p.schema, &wire, &format!("parameters.{}", p.name)) {
                t.parameters.insert(p.name.to_string(), kept);
            }
        }
    });
}

/// Keeps a fact in wire form, redacted, once it is loaded. For generated code.
pub fn note_fact<T: serde::Serialize>(ctx: &super::server::RequestContext, name: &str, schema: &Schema, fact: &T) {
    let Ok(typed) = serde_json::to_value(fact) else {
        return;
    };
    let wire = schema.to_wire(&typed, false, false);
    ctx.platform.trail(|t| {
        if let Some(kept) = keep(t, schema, &wire, &format!("facts.{name}")) {
            t.facts.insert(name.to_string(), kept);
        }
    });
}

/// Whether a field is marked `@pii` or `@secret`.
fn marked(f: &super::schema::Field) -> bool {
    // @if pii
    f.pii
    // @end
    // @if !pii
    {
        let _ = f;
        false
    }
    // @end
}

fn mismatch(path: &str, expected: &str, found: &Value) -> String {
    let kind = match found {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    format!("{path}: expected {expected}, found {kind}")
}

/// `wire`, a wire value of `schema`, without every `@pii` and `@secret` field at any depth
/// (nested models, lists, maps, the `items` of a page, the arm of a union), and the paths of what
/// was left out (`principal.email`, `parameters.body.card`, `result[1].name`,
/// `result.items[0].phone`) in `out`. A field is removed whole; members the declaration does not
/// know are kept. It is `veld_rules::redact_type_at`, written once for the audit log and for the
/// captures that reuse it.
///
/// # Errors
/// The path and the kinds when the value does not have the shape of its type: a value that cannot
/// be walked cannot be known to be clean.
pub fn redact(schema: &Schema, v: &Value, path: &str, out: &mut BTreeSet<String>) -> Result<Value, String> {
    if v.is_null() {
        return Ok(Value::Null);
    }
    match schema {
        Schema::Scalar(..) | Schema::Enum(_) => Ok(v.clone()),
        Schema::List(of, _) => redact_items(of, v, path, out),
        Schema::Map(of, _) => {
            let entries = v.as_object().ok_or_else(|| mismatch(path, "object", v))?;
            let mut kept = Map::new();
            for (k, e) in entries {
                kept.insert(k.clone(), redact(of, e, &format!("{path}.{k}"), out)?);
            }
            Ok(Value::Object(kept))
        }
        Schema::Page(of) => {
            let obj = v.as_object().ok_or_else(|| mismatch(path, "object", v))?;
            let mut kept = obj.clone();
            if let Some(items) = obj.get("items") {
                kept.insert("items".to_string(), redact_items(of, items, &format!("{path}.items"), out)?);
            }
            Ok(Value::Object(kept))
        }
        Schema::Model(m) => {
            let obj = v.as_object().ok_or_else(|| mismatch(path, "object", v))?;
            let mut kept = obj.clone();
            for f in m.fields {
                let Some(child) = obj.get(f.wire) else {
                    continue;
                };
                let here = format!("{path}.{}", f.wire);
                if marked(f) {
                    kept.remove(f.wire);
                    out.insert(here);
                } else {
                    kept.insert(f.wire.to_string(), redact(f.schema, child, &here, out)?);
                }
            }
            Ok(Value::Object(kept))
        }
        Schema::Union(u) => {
            let obj = v.as_object().ok_or_else(|| mismatch(path, "object", v))?;
            let label = obj.get(u.discriminator).and_then(Value::as_str);
            match label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) {
                Some((_, arm)) => redact(arm, v, path, out),
                None => Err(format!("{path}: the value names no arm of the union `{}`", u.name)),
            }
        }
    }
}

fn redact_items(of: &Schema, v: &Value, path: &str, out: &mut BTreeSet<String>) -> Result<Value, String> {
    let list = v.as_array().ok_or_else(|| mismatch(path, "array", v))?;
    let mut kept = Vec::with_capacity(list.len());
    for (i, e) in list.iter().enumerate() {
        kept.push(redact(of, e, &format!("{path}[{i}]"), out)?);
    }
    Ok(Value::Array(kept))
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A datetime as the contract writes one: the fraction only when it is not zero.
fn format_millis(ms: i64) -> String {
    let seconds = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let (y, mo, d) = civil_from_days(seconds.div_euclid(86_400));
    let rem = seconds.rem_euclid(86_400);
    let fraction = if millis == 0 { String::new() } else { format!(".{millis:03}") };
    format!("{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}{fraction}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// The audit event of a request that ended with `status` and `body` (what was sent): the trail the
/// request left, the answer it gave, and what visibility dropped.
///
/// # Errors
/// Why the event cannot be built: a value of the trail or the answer that cannot be walked.
pub(crate) fn build_event(ep: &EndpointSpec, spec: &AuditSpec, state: &State, status: u16, body: Option<&[u8]>) -> Result<AuditEvent, String> {
    let mut trail = state.trail.lock().map_err(|_| "the trail of the request is poisoned".to_string())?;
    if let Some(why) = trail.broken.take() {
        return Err(why);
    }
    let now = trail.now.unwrap_or_else(std::time::SystemTime::now);
    let document: Option<Value> = if status >= 400 { body.and_then(|b| serde_json::from_slice(b).ok()) } else { None };
    let error = document.as_ref().and_then(|d| d.get("title")).and_then(Value::as_str).map(str::to_string);
    let mut redacted = trail.redacted.clone();
    let principal = match (&trail.principal, ep.auth.is_some()) {
        (Some((wire, schema)), true) => Some(redact(schema, wire, "principal", &mut redacted)?),
        (_, _) => None,
    };
    let result = match (&document, ep.success, body) {
        (Some(doc), _, _) => {
            let sig = doc.get("type").and_then(Value::as_str).unwrap_or_default();
            match (spec.errors.contains(&sig), ep.errors.iter().find(|d| d.sig == sig), doc.get("data")) {
                (true, Some(d), Some(data)) => Some(redact(d.schema, data, "result", &mut redacted)?),
                (_, _, _) => None,
            }
        }
        (None, Some(schema), Some(bytes)) if status < 400 && !bytes.is_empty() => {
            let wire: Value = serde_json::from_slice(bytes).map_err(|e| format!("result: the answer is not JSON: {e}"))?;
            Some(redact(schema, &wire, "result", &mut redacted)?)
        }
        (None, _, _) => None,
    };
    let hidden = state.hidden();
    Ok(AuditEvent {
        operation: ep.sig.to_string(),
        time: format_millis(super::rules::clock_ms(now)),
        principal,
        outcome: AuditOutcome { status, error },
        parameters: trail.parameters.clone(),
        facts: trail.facts.clone(),
        result,
        hidden,
        redacted: redacted.into_iter().collect(),
    })
}
// @end

// @if limits
// ---------------------------------------------------------------------------------------------
// Rate limits (`spec/platform.md` section 5)
// ---------------------------------------------------------------------------------------------

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Whose calls a limit counts together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitBy {
    User,
    Tenant,
    Ip,
}

impl LimitBy {
    /// The word of the contract, which is also the `k` of the key.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            LimitBy::User => "user",
            LimitBy::Tenant => "tenant",
            LimitBy::Ip => "ip",
        }
    }
}

/// The limit of an endpoint: `@limit(10, per: minute, by: user)`.
#[derive(Debug, Clone, Copy)]
pub struct LimitSpec {
    pub count: u32,
    pub window_seconds: u32,
    pub by: LimitBy,
}

/// What a limiter answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub allowed: bool,
    /// The time left in the window, in whole seconds and at least 1, for a refused call; 0 otherwise.
    pub retry_after_seconds: u32,
}

/// The key of a limit: the operation signature, the subject and the identity, joined with `|`.
#[must_use]
pub fn limit_key(operation: &str, by: LimitBy, identity: &str) -> String {
    format!("{operation}|{}|{identity}", by.word())
}

/// A limiter: it allows at most `limit` calls of one key in any window it defines of
/// `window_seconds`. Implement it over your store (a database, a cache) for a server that runs on
/// more than one machine; the memory limiter counts per process.
pub trait Limiter: Send + Sync + 'static {
    /// One call of `key`. An `Err` is a failing limiter: the call is allowed and the failure goes
    /// to `on_error`.
    fn take<'a>(&'a self, key: &'a str, limit: u32, window_seconds: u32) -> Pin<Box<dyn Future<Output = Result<Decision, super::error::Error>> + Send + 'a>>;
}

#[derive(Debug, Clone, Copy)]
struct Window {
    start_ms: i64,
    calls: u32,
}

/// The memory limiter: a fixed window per key that starts with the first call of the key that
/// finds no open window; a window is open from its start up to, not including, its start plus its
/// length; a refused call does not count (`veld_rules::limits::MemoryLimiter`).
pub struct MemoryLimiter {
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    windows: Mutex<BTreeMap<String, Window>>,
}

impl std::fmt::Debug for MemoryLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemoryLimiter")
    }
}

impl Default for MemoryLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Windows that ended more than a day ago are swept once there are more than this many.
const SWEEP_ABOVE: usize = 10_000;
const DAY_MS: i64 = 86_400_000;

impl MemoryLimiter {
    /// A limiter on the system clock.
    #[must_use]
    pub fn new() -> Self {
        Self::with_clock(|| super::rules::clock_ms(std::time::SystemTime::now()))
    }

    /// A limiter on a clock of its own, in milliseconds since the epoch: a test drives it.
    #[must_use]
    pub fn with_clock(clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        MemoryLimiter { clock: Arc::new(clock), windows: Mutex::new(BTreeMap::new()) }
    }

    /// One call of `key` at `now_ms`.
    #[must_use]
    pub fn take_at(&self, key: &str, limit: u32, window_seconds: u32, now_ms: i64) -> Decision {
        let length = i64::from(window_seconds).saturating_mul(1000);
        let Ok(mut windows) = self.windows.lock() else {
            return Decision { allowed: true, retry_after_seconds: 0 };
        };
        if windows.len() > SWEEP_ABOVE {
            windows.retain(|_, w| now_ms < w.start_ms.saturating_add(length).saturating_add(DAY_MS));
        }
        let open = windows.get(key).filter(|w| now_ms < w.start_ms.saturating_add(length)).copied();
        let mut window = open.unwrap_or(Window { start_ms: now_ms, calls: 0 });
        let decision = if window.calls < limit {
            window.calls += 1;
            Decision { allowed: true, retry_after_seconds: 0 }
        } else {
            let left = window.start_ms.saturating_add(length).saturating_sub(now_ms);
            let seconds = (left.clamp(1, length.max(1)) + 999) / 1000;
            Decision { allowed: false, retry_after_seconds: u32::try_from(seconds).unwrap_or(u32::MAX) }
        };
        windows.insert(key.to_string(), window);
        decision
    }
}

impl Limiter for MemoryLimiter {
    fn take<'a>(&'a self, key: &'a str, limit: u32, window_seconds: u32) -> Pin<Box<dyn Future<Output = Result<Decision, super::error::Error>> + Send + 'a>> {
        let now = (self.clock)();
        Box::pin(std::future::ready(Ok(self.take_at(key, limit, window_seconds, now))))
    }
}
// @end


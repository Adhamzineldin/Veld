//! Living documentation of business rules: the `x-veld-*` extensions of schemas and operations
//! (`spec/rules.md` sections 5, 9 and 10). Nothing here is emitted for a contract without rules,
//! so the document of such a contract is unchanged.

use serde_json::{Map, Value, json};
use veld_ir::{Decl, DeclKind, Endpoint, EndpointTransition, Enum, Ir, Rule};

use super::schema::type_schema;
use crate::diff::expr_text::expr_text;

/// Wire name of a field of `d` by its source name; the source name when there is no such field.
fn wire_of(d: &Decl, name: &str) -> String {
    d.fields()
        .iter()
        .find(|f| f.name == name)
        .map_or_else(|| name.to_string(), |f| f.wire.clone())
}

/// `"rollout": "audit"` on a rule that never rejects (`@rollout(audit)`, `spec/rules.md` section
/// 17.1); nothing for an enforced one, so an export without the annotation keeps its bytes.
fn audit_mark(audited: bool, o: &mut Map<String, Value>) {
    if audited {
        o.insert("rollout".into(), Value::from("audit"));
    }
}

fn invariant(d: &Decl, r: &Rule) -> Value {
    let mut o = Map::new();
    o.insert("name".into(), Value::from(r.name.clone()));
    o.insert("id".into(), Value::from(r.id.clone()));
    if let Some(m) = &r.message {
        o.insert("message".into(), Value::from(m.clone()));
    }
    let fields: Vec<String> = r.reads.iter().map(|n| wire_of(d, n)).collect();
    o.insert("fields".into(), json!(fields));
    o.insert("expression".into(), Value::from(expr_text(&r.expr)));
    audit_mark(r.is_audited(), &mut o);
    Value::Object(o)
}

/// Adds `x-veld-rules` (own invariants, like own fields) and, for a state machine,
/// `x-veld-flows` and `x-veld-finals` to a declaration schema.
pub(super) fn decorate_decl(d: &Decl, m: &mut Map<String, Value>) {
    let own: Vec<Value> = d
        .rules()
        .iter()
        .filter(|r| r.inherited_from.is_none())
        .map(|r| invariant(d, r))
        .collect();
    if !own.is_empty() {
        m.insert("x-veld-rules".into(), Value::Array(own));
    }
    if let DeclKind::Enum(e) = &d.kind
        && e.is_state_machine()
    {
        machine(e, m);
    }
    super::conditional::decorate_decl(d, m);
}

fn variant_wire<'a>(e: &'a Enum, name: &'a str) -> &'a str {
    e.variants
        .iter()
        .find(|v| v.name == name)
        .map_or(name, |v| v.wire.as_str())
}

fn machine(e: &Enum, m: &mut Map<String, Value>) {
    let flows: Vec<Value> = e
        .flows
        .iter()
        .map(|f| json!({ "from": variant_wire(e, &f.from), "to": variant_wire(e, &f.to) }))
        .collect();
    m.insert("x-veld-flows".into(), Value::Array(flows));
    let finals: Vec<&str> = e.finals.iter().map(|n| variant_wire(e, n)).collect();
    m.insert("x-veld-finals".into(), json!(finals));
}

fn require_entry(ir: &Ir, r: &veld_ir::EndpointRequire) -> Value {
    let mut o = Map::new();
    if let Some(n) = &r.name {
        o.insert("name".into(), Value::from(n.clone()));
    }
    o.insert("id".into(), Value::from(r.id.clone()));
    o.insert("expression".into(), Value::from(expr_text(&r.expr)));
    if let Some(e) = &r.else_error {
        o.insert("else".into(), Value::from(ir.decl(e.decl).name.clone()));
    }
    audit_mark(r.rollout.is_some(), &mut o);
    if let Some(m) = &r.message {
        o.insert("message".into(), Value::from(m.clone()));
    }
    Value::Object(o)
}

fn ensure_entry(r: &veld_ir::EndpointEnsure) -> Value {
    let mut o = Map::new();
    if let Some(n) = &r.name {
        o.insert("name".into(), Value::from(n.clone()));
    }
    o.insert("id".into(), Value::from(r.id.clone()));
    o.insert("expression".into(), Value::from(expr_text(&r.expr)));
    audit_mark(r.rollout.is_some(), &mut o);
    if let Some(m) = &r.message {
        o.insert("message".into(), Value::from(m.clone()));
    }
    Value::Object(o)
}

fn transition(ir: &Ir, t: &EndpointTransition) -> Value {
    let steps: Vec<&str> = t.steps.iter().map(|s| s.wire.as_str()).collect();
    let field = format!("{}.{}", t.fact, steps.join("."));
    let wire = |name: &str| match &ir.decl(t.decl).kind {
        DeclKind::Enum(e) => variant_wire(e, name).to_string(),
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_)
        | DeclKind::Event(_) => name.to_string(),
    };
    let from: Vec<String> = t.from.iter().map(|n| wire(n)).collect();
    json!({ "id": t.id, "field": field, "from": from, "to": wire(&t.to) })
}

/// Adds `x-veld-facts`, `x-veld-rules` and `x-veld-transition` to an operation.
pub(super) fn decorate_operation(ir: &Ir, ep: &Endpoint, o: &mut Map<String, Value>) {
    let rules = &ep.rules;
    if !rules.facts.is_empty() {
        let facts: Vec<Value> = rules
            .facts
            .iter()
            .map(|f| {
                json!({ "name": f.name, "type": type_schema(ir, &f.ty), "optional": f.optional })
            })
            .collect();
        o.insert("x-veld-facts".into(), Value::Array(facts));
    }
    if !rules.requires.is_empty() || !rules.ensures.is_empty() {
        let mut r = Map::new();
        if !rules.requires.is_empty() {
            let v: Vec<Value> = rules
                .requires
                .iter()
                .map(|x| require_entry(ir, x))
                .collect();
            r.insert("requires".into(), Value::Array(v));
        }
        if !rules.ensures.is_empty() {
            let v: Vec<Value> = rules.ensures.iter().map(ensure_entry).collect();
            r.insert("ensures".into(), Value::Array(v));
        }
        o.insert("x-veld-rules".into(), Value::Object(r));
    }
    if let Some(t) = &rules.transition {
        o.insert("x-veld-transition".into(), transition(ir, t));
    }
    super::conditional::decorate_operation(ep, o);
    super::conditional::decorate_service_levels(ep, o);
    super::conditional::decorate_limit(ep, o);
}

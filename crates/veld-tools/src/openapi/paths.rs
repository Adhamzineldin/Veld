//! Operations: parameters, request bodies, responses and security.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use veld_ir::{AuthKind, Binding, DeclKind, Endpoint, Ir, Param, Service, Type};

use super::schema::{apply_constraints, field_schema, schema_ref, type_schema};

fn sanitize(header: &str) -> String {
    header
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Name of the security scheme for an auth requirement; `None` for no auth.
pub(super) fn scheme_name(kind: AuthKind, header: Option<&str>) -> Option<String> {
    match kind {
        AuthKind::None => None,
        AuthKind::Bearer => Some("bearerAuth".into()),
        AuthKind::Basic => Some("basicAuth".into()),
        AuthKind::ApiKey => Some(format!(
            "apiKey_{}",
            sanitize(header.unwrap_or("X-API-Key"))
        )),
    }
}

/// The scheme object for a scheme name.
pub(super) fn scheme_object(kind: AuthKind, header: Option<&str>) -> Value {
    match kind {
        AuthKind::Bearer => json!({ "type": "http", "scheme": "bearer" }),
        AuthKind::Basic => json!({ "type": "http", "scheme": "basic" }),
        AuthKind::None | AuthKind::ApiKey => {
            json!({ "type": "apiKey", "in": "header", "name": header.unwrap_or("X-API-Key") })
        }
    }
}

fn param_object(ir: &Ir, p: &Param) -> Option<Value> {
    let (place, name, required) = match p.binding {
        Binding::Path => ("path", p.name.clone(), true),
        Binding::Query => ("query", p.name.clone(), !p.optional),
        Binding::Header => (
            "header",
            p.header.clone().unwrap_or_else(|| p.name.clone()),
            !p.optional,
        ),
        Binding::QueryObject | Binding::Body => return None,
    };
    let mut schema = type_schema(ir, &p.ty);
    apply_constraints(&mut schema, &p.ty, &p.constraints);
    Some(json!({ "name": name, "in": place, "required": required, "schema": schema }))
}

/// A query object expands into one parameter per field.
fn query_object_params(ir: &Ir, p: &Param) -> Vec<Value> {
    let Type::Decl(id) = &p.ty else {
        return Vec::new();
    };
    ir.decl(*id)
        .fields()
        .iter()
        .map(|f| {
            let required = !f.optional && f.default.is_none();
            let mut o = Map::new();
            o.insert("name".into(), Value::from(f.wire.clone()));
            o.insert("in".into(), Value::from("query"));
            o.insert("required".into(), Value::Bool(required));
            if let Some(d) = &f.docs {
                o.insert("description".into(), Value::from(d.clone()));
            }
            o.insert("schema".into(), field_schema(ir, f));
            Value::Object(o)
        })
        .collect()
}

fn parameters(ir: &Ir, ep: &Endpoint) -> Vec<Value> {
    let mut out = Vec::new();
    for p in &ep.params {
        if p.binding == Binding::QueryObject {
            out.extend(query_object_params(ir, p));
        } else if let Some(o) = param_object(ir, p) {
            out.push(o);
        }
    }
    out
}

fn request_body(ir: &Ir, ep: &Endpoint) -> Option<Value> {
    let p = ep.body()?;
    let mut schema = type_schema(ir, &p.ty);
    apply_constraints(&mut schema, &p.ty, &p.constraints);
    Some(
        json!({ "required": !p.optional, "content": { "application/json": { "schema": schema } } }),
    )
}

fn success_response(ir: &Ir, ep: &Endpoint) -> Value {
    let Some(ty) = &ep.success.ty else {
        return json!({ "description": "No content" });
    };
    let media = if ep.success.stream {
        "text/event-stream"
    } else {
        "application/json"
    };
    let mut r = json!({
        "description": "Success",
        "content": { media: { "schema": type_schema(ir, ty) } },
    });
    if ep.success.stream
        && let Some(m) = r.as_object_mut()
    {
        m.insert("x-veld-stream".into(), Value::Bool(true));
    }
    r
}

/// RFC 9457 problem document schema for one error of one operation.
fn problem_schema(ir: &Ir, ep: &Endpoint, status: u16, decl: veld_ir::DeclId) -> Value {
    let d = ir.decl(decl);
    let mut props = Map::new();
    props.insert("type".into(), json!({ "const": d.sig }));
    props.insert("title".into(), json!({ "const": d.name }));
    props.insert("status".into(), json!({ "const": status }));
    props.insert("operation".into(), json!({ "const": ep.sig }));
    let mut required = vec!["type", "title", "status", "operation"];
    if matches!(&d.kind, DeclKind::Error(e) if !e.fields.fields.is_empty()) {
        props.insert("data".into(), schema_ref(ir, decl));
        required.push("data");
    }
    json!({ "type": "object", "properties": props, "required": required })
}

fn error_responses(ir: &Ir, ep: &Endpoint) -> BTreeMap<u16, Value> {
    let mut by_status: BTreeMap<u16, Vec<(String, Value)>> = BTreeMap::new();
    for e in &ep.errors {
        let name = ir.decl(e.decl).name.clone();
        by_status
            .entry(e.status)
            .or_default()
            .push((name, problem_schema(ir, ep, e.status, e.decl)));
    }
    by_status
        .into_iter()
        .map(|(status, items)| {
            let names: Vec<String> = items.iter().map(|(n, _)| n.clone()).collect();
            let schema = if items.len() == 1 {
                items.into_iter().next().map_or(Value::Null, |(_, s)| s)
            } else {
                json!({ "oneOf": items.into_iter().map(|(_, s)| s).collect::<Vec<_>>() })
            };
            let r = json!({
                "description": names.join(" or "),
                "content": { "application/problem+json": { "schema": schema } },
            });
            (status, r)
        })
        .collect()
}

fn responses(ir: &Ir, ep: &Endpoint) -> Value {
    let mut m = Map::new();
    m.insert(ep.success.status.to_string(), success_response(ir, ep));
    for (status, r) in error_responses(ir, ep) {
        m.insert(status.to_string(), r);
    }
    Value::Object(m)
}

/// The operation object of an endpoint.
pub(super) fn operation(ir: &Ir, svc: &Service, ep: &Endpoint) -> Value {
    let mut o = Map::new();
    o.insert(
        "operationId".into(),
        Value::from(format!("{}.{}", svc.name, ep.name)),
    );
    o.insert("tags".into(), json!([svc.name]));
    if let Some(docs) = &ep.docs {
        let summary = docs.lines().next().unwrap_or_default();
        o.insert("summary".into(), Value::from(summary));
        o.insert("description".into(), Value::from(docs.clone()));
    }
    if ep.deprecated.is_some() {
        o.insert("deprecated".into(), Value::Bool(true));
    }
    let params = parameters(ir, ep);
    if !params.is_empty() {
        o.insert("parameters".into(), Value::Array(params));
    }
    if let Some(b) = request_body(ir, ep) {
        o.insert("requestBody".into(), b);
    }
    o.insert("responses".into(), responses(ir, ep));
    if let Some(name) = scheme_name(ep.auth.kind, ep.auth.header.as_deref()) {
        o.insert("security".into(), json!([{ name: [] }]));
    }
    if !ep.middleware.is_empty() {
        o.insert("x-veld-middleware".into(), json!(ep.middleware));
    }
    o.insert("x-veld-op-sig".into(), Value::from(ep.sig.clone()));
    Value::Object(o)
}

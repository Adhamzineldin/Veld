//! OpenAPI 3.1 export. The output is deterministic: paths, methods and schemas
//! are emitted in sorted order and every object is built with a fixed key
//! order.

mod conditional;
mod paths;
mod rules;
mod schema;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};
use veld_ir::{Ir, Method, ServiceId};

/// Exports the given services (and every declaration they reach) as an
/// OpenAPI 3.1 document. With no services, every declaration of the IR is
/// exported as a schema (a types-only package). WebSocket endpoints have no OpenAPI form; they are
/// listed under `x-veld-websocket-operations`.
#[must_use]
pub fn openapi(ir: &Ir, services: &[ServiceId]) -> Value {
    let ids: BTreeSet<ServiceId> = services.iter().copied().collect();
    let ids: Vec<ServiceId> = ids.into_iter().collect();
    let mut doc = Map::new();
    doc.insert("openapi".into(), Value::from("3.1.0"));
    doc.insert("info".into(), info(ir, &ids));
    let servers = servers(ir, &ids);
    if !servers.is_empty() {
        doc.insert("servers".into(), Value::Array(servers));
    }
    let (paths, ws) = build_paths(ir, &ids);
    doc.insert("paths".into(), paths);
    doc.insert("components".into(), components(ir, &ids));
    if !ws.is_empty() {
        doc.insert("x-veld-websocket-operations".into(), json!(ws));
    }
    doc.insert(
        "x-veld-ir-version".into(),
        Value::from(ir.ir_version.clone()),
    );
    Value::Object(doc)
}

/// The schema builder of the export, for the AsyncAPI export (`spec/events.md` section 16.1):
/// the component name of a declaration, its schema (an event is built like a model), and the
/// schema of a type.
pub(crate) mod shared {
    use serde_json::Value;
    use veld_ir::{Decl, DeclId, DeclKind, Ir, Type};

    use super::schema;

    #[must_use]
    pub(crate) fn component_name(ir: &Ir, id: DeclId) -> String {
        schema::schema_name(ir, id)
    }

    /// The component schema of a declaration; an event is an object like a model.
    #[must_use]
    pub(crate) fn component_schema(ir: &Ir, d: &Decl) -> Option<Value> {
        match d.kind {
            DeclKind::Event(_) => Some(schema::model_schema(ir, d)),
            DeclKind::Model(_)
            | DeclKind::Error(_)
            | DeclKind::Enum(_)
            | DeclKind::Union(_)
            | DeclKind::Const(_) => schema::decl_schema(ir, d),
        }
    }

    #[must_use]
    pub(crate) fn type_schema(ir: &Ir, ty: &Type) -> Value {
        schema::type_schema(ir, ty)
    }
}

fn info(ir: &Ir, ids: &[ServiceId]) -> Value {
    let names: Vec<&str> = ids.iter().map(|i| ir.service(*i).name.as_str()).collect();
    let title = if names.is_empty() {
        "Veld API".to_string()
    } else {
        names.join(", ")
    };
    let desc = ids.iter().find_map(|i| ir.service(*i).docs.clone());
    let mut m = Map::new();
    m.insert("title".into(), Value::from(title));
    m.insert("version".into(), Value::from("1.0.0"));
    if let Some(d) = desc.filter(|_| ids.len() == 1) {
        m.insert("description".into(), Value::from(d));
    }
    Value::Object(m)
}

fn servers(ir: &Ir, ids: &[ServiceId]) -> Vec<Value> {
    let urls: BTreeSet<&str> = ids
        .iter()
        .filter_map(|i| ir.service(*i).base_url.as_deref())
        .collect();
    urls.into_iter().map(|u| json!({ "url": u })).collect()
}

fn build_paths(ir: &Ir, ids: &[ServiceId]) -> (Value, Vec<String>) {
    let mut paths: BTreeMap<String, BTreeMap<Method, Value>> = BTreeMap::new();
    let mut ws = Vec::new();
    for id in ids {
        let svc = ir.service(*id);
        for ep in &svc.endpoints {
            if ep.method == Method::Ws {
                ws.push(ep.sig.clone());
                continue;
            }
            paths
                .entry(ep.path.clone())
                .or_default()
                .insert(ep.method, paths::operation(ir, svc, ep));
        }
    }
    let out: Map<String, Value> = paths
        .into_iter()
        .map(|(path, ops)| {
            let item: Map<String, Value> = ops
                .into_iter()
                .map(|(m, op)| (m.lower().to_string(), op))
                .collect();
            (path, Value::Object(item))
        })
        .collect();
    (Value::Object(out), ws)
}

fn components(ir: &Ir, ids: &[ServiceId]) -> Value {
    let mut schemas: BTreeMap<String, Value> = BTreeMap::new();
    let decls = if ids.is_empty() {
        ir.all_decl_ids()
    } else {
        ir.reachable_decls(ids)
    };
    for id in decls {
        if let Some(s) = schema::decl_schema(ir, ir.decl(id)) {
            schemas.insert(schema::schema_name(ir, id), s);
        }
    }
    let mut c = Map::new();
    c.insert(
        "schemas".into(),
        Value::Object(schemas.into_iter().collect()),
    );
    let schemes = security_schemes(ir, ids);
    if !schemes.is_empty() {
        c.insert("securitySchemes".into(), Value::Object(schemes));
    }
    Value::Object(c)
}

fn security_schemes(ir: &Ir, ids: &[ServiceId]) -> Map<String, Value> {
    let mut found: BTreeMap<String, Value> = BTreeMap::new();
    for id in ids {
        for ep in &ir.service(*id).endpoints {
            let (kind, header) = (ep.auth.kind, ep.auth.header.as_deref());
            if let Some(name) = paths::scheme_name(kind, header) {
                found.insert(name, paths::scheme_object(kind, header));
            }
        }
    }
    found.into_iter().collect()
}

#[cfg(test)]
mod conditional_tests;
#[cfg(test)]
mod derived_tests;
#[cfg(test)]
mod event_tests;
#[cfg(test)]
mod pii_tests;
#[cfg(test)]
mod rollout_tests;
#[cfg(test)]
mod saga_tests;
#[cfg(test)]
mod service_levels_tests;
#[cfg(test)]
mod tests;

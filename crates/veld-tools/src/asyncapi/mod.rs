//! AsyncAPI 3.0 export (`spec/events.md` section 16.1). The output is deterministic: every
//! object is built with a fixed key order, channels and operations are sorted by name, and the
//! schemas are the ones of the OpenAPI export.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};
use thiserror::Error;
use veld_ir::{Channel, ChannelId, Decl, DeclId, DeclKind, Ir, Service, ServiceId};

use crate::openapi::shared::{component_name, component_schema, type_schema};

/// The media type of every message (`spec/events.md` section 6.2).
const MEDIA_TYPE: &str = "application/cloudevents+json; charset=UTF-8";
/// The dialect of the payload schemas: the builder of the OpenAPI export writes keywords that a
/// JSON Schema draft-07 validator reads (`type`, `properties`, `required`, `allOf`, `oneOf`,
/// `const`, `enum`, `format`); the rest is ignored as an unknown keyword.
const SCHEMA_FORMAT: &str = "application/schema+json;version=draft-07";

/// The contract has nothing to export: no channel, or no channel the selected services use.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("nothing to export: {0}")]
pub struct NoChannels(pub &'static str);

/// Exports the channels the given services publish to or subscribe to, their messages and the
/// operations of those services as an AsyncAPI 3.0 document. With no services, every service.
///
/// # Errors
/// [`NoChannels`] when the contract has no channel, or the selected services use none.
pub fn asyncapi(ir: &Ir, services: &[ServiceId]) -> Result<Value, NoChannels> {
    if ir.channels.is_empty() {
        return Err(NoChannels("the workspace declares no channel"));
    }
    let ids: BTreeSet<ServiceId> = if services.is_empty() {
        ir.services.iter().map(|s| s.id).collect()
    } else {
        services.iter().copied().collect()
    };
    let ids: Vec<ServiceId> = ids.into_iter().collect();
    let channels = ir.event_channels(&ids);
    if channels.is_empty() {
        return Err(NoChannels(
            "the selected services publish to and subscribe to no channel",
        ));
    }
    let mut doc = Map::new();
    doc.insert("asyncapi".into(), Value::from("3.0.0"));
    doc.insert("info".into(), info(ir, &ids));
    doc.insert("defaultContentType".into(), Value::from(MEDIA_TYPE));
    doc.insert("channels".into(), build_channels(ir, &channels));
    doc.insert("operations".into(), build_operations(ir, &ids));
    doc.insert("components".into(), components(ir, &ids));
    doc.insert(
        "x-veld-ir-version".into(),
        Value::from(ir.ir_version.clone()),
    );
    Ok(Value::Object(doc))
}

fn info(ir: &Ir, ids: &[ServiceId]) -> Value {
    let names: Vec<&str> = ids.iter().map(|i| ir.service(*i).name.as_str()).collect();
    json!({ "title": names.join(", "), "version": "1.0.0" })
}

/// The key of a channel in `channels`: its qualified name made safe, so that two channels of the
/// same name in two packages stay apart.
fn channel_key(c: &Channel) -> String {
    safe(&c.qualified)
}

fn safe(text: &str) -> String {
    text.trim_start_matches('@')
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

fn event_decl(ir: &Ir, id: DeclId) -> &Decl {
    ir.decl(id)
}

fn build_channels(ir: &Ir, used: &[ChannelId]) -> Value {
    let sorted: BTreeMap<String, &Channel> = used
        .iter()
        .map(|c| (channel_key(ir.channel(*c)), ir.channel(*c)))
        .collect();
    let out: Map<String, Value> = sorted
        .into_iter()
        .map(|(key, c)| (key, channel(ir, c)))
        .collect();
    Value::Object(out)
}

fn channel(ir: &Ir, c: &Channel) -> Value {
    let mut m = Map::new();
    m.insert("address".into(), Value::from(c.topic.clone()));
    if let Some(d) = &c.docs {
        m.insert("description".into(), Value::from(d.clone()));
    }
    m.insert("messages".into(), messages(ir, c));
    if let Some(k) = &c.key {
        m.insert(
            "x-veld-key".into(),
            json!({ "name": k.name, "wire": k.wire, "type": type_schema(ir, &k.ty) }),
        );
    }
    if let Some(r) = &c.retry {
        m.insert(
            "x-veld-retry".into(),
            serde_json::to_value(r).unwrap_or(Value::Null),
        );
    }
    if c.dead_letter {
        m.insert(
            "x-veld-dead-letter".into(),
            json!({ "topic": format!("{}.dlq", c.topic) }),
        );
    }
    if c.ordered {
        m.insert("x-veld-ordered".into(), Value::Bool(true));
    }
    if let Some(d) = &c.deprecated {
        m.insert("x-veld-deprecated".into(), Value::from(d.clone()));
    }
    Value::Object(m)
}

fn messages(ir: &Ir, c: &Channel) -> Value {
    let sorted: BTreeMap<String, Value> = c
        .events
        .iter()
        .map(|e| (message_key(ir, *e), message(ir, c, *e)))
        .collect();
    Value::Object(sorted.into_iter().collect())
}

fn message_key(ir: &Ir, event: DeclId) -> String {
    component_name(ir, event)
}

fn message(ir: &Ir, c: &Channel, event: DeclId) -> Value {
    let d = event_decl(ir, event);
    let mut m = Map::new();
    m.insert("name".into(), Value::from(d.name.clone()));
    if let Some(docs) = &d.docs {
        m.insert("summary".into(), Value::from(docs.clone()));
    }
    m.insert("contentType".into(), Value::from(MEDIA_TYPE));
    m.insert(
        "payload".into(),
        json!({ "schemaFormat": SCHEMA_FORMAT, "schema": envelope(ir, c, event) }),
    );
    m.insert("x-veld-signature".into(), Value::from(d.sig.clone()));
    if let DeclKind::Event(e) = &d.kind
        && let Some(digest) = &e.rules_digest
    {
        m.insert("x-veld-rules-digest".into(), Value::from(digest.clone()));
    }
    if let Some(dep) = &d.deprecated {
        m.insert("x-veld-deprecated".into(), Value::from(dep.clone()));
    }
    Value::Object(m)
}

/// The CloudEvents envelope of `spec/events.md` section 6.2, with `data` the event.
fn envelope(ir: &Ir, c: &Channel, event: DeclId) -> Value {
    let mut required = vec![
        "specversion",
        "id",
        "source",
        "type",
        "time",
        "datacontenttype",
    ];
    if c.key.is_some() {
        required.push("subject");
    }
    required.push("data");
    json!({
        "type": "object",
        "required": required,
        "properties": {
            "specversion": { "const": "1.0" },
            "id": { "type": "string", "minLength": 1 },
            "source": { "type": "string" },
            "type": { "type": "string", "description": "the declaration signature of the event" },
            "subject": { "type": "string", "description": "the key text of the channel" },
            "time": { "type": "string", "format": "date-time" },
            "datacontenttype": { "const": "application/json" },
            "veldrules": { "type": "string", "description": "the rules digest of the event, when it has one" },
            "data": { "$ref": format!("#/components/schemas/{}", component_name(ir, event)) },
        },
    })
}

fn build_operations(ir: &Ir, ids: &[ServiceId]) -> Value {
    let mut ops: BTreeMap<String, Value> = BTreeMap::new();
    for id in ids {
        let s = ir.service(*id);
        let service = format!("{}.{}", safe(&ir.package(s.package).name), s.name);
        for p in &s.publishes {
            let c = ir.channel(p.channel);
            ops.insert(
                format!("{service}.publish.{}", channel_key(c)),
                send(ir, ir_source(ir, s), c),
            );
        }
        for sub in &s.subscriptions {
            let c = ir.channel(sub.channel);
            ops.insert(
                format!("{service}.subscribe.{}", channel_key(c)),
                receive(ir, c, sub),
            );
        }
    }
    Value::Object(ops.into_iter().collect())
}

fn ir_source(ir: &Ir, s: &Service) -> String {
    format!("veld:{}/{}", ir.package(s.package).name, s.name)
}

fn message_refs(ir: &Ir, c: &Channel, events: &[DeclId]) -> Vec<Value> {
    let key = channel_key(c);
    let names: BTreeSet<String> = events.iter().map(|e| message_key(ir, *e)).collect();
    names
        .into_iter()
        .map(|n| json!({ "$ref": format!("#/channels/{key}/messages/{n}") }))
        .collect()
}

fn send(ir: &Ir, source: String, c: &Channel) -> Value {
    json!({
        "action": "send",
        "channel": { "$ref": format!("#/channels/{}", channel_key(c)) },
        "messages": message_refs(ir, c, &c.events),
        "x-veld-source": source,
    })
}

fn receive(ir: &Ir, c: &Channel, sub: &veld_ir::Subscription) -> Value {
    let events: Vec<DeclId> = sub.handlers.iter().map(|h| h.event).collect();
    let handlers: BTreeMap<String, Value> = sub
        .handlers
        .iter()
        .map(|h| (message_key(ir, h.event), Value::from(h.name.clone())))
        .collect();
    json!({
        "action": "receive",
        "channel": { "$ref": format!("#/channels/{}", channel_key(c)) },
        "messages": message_refs(ir, c, &events),
        "x-veld-group": sub.group,
        "x-veld-handlers": Value::Object(handlers.into_iter().collect()),
    })
}

fn components(ir: &Ir, ids: &[ServiceId]) -> Value {
    let mut decls = ir.events_of(ids);
    decls.extend(ir.event_reachable_decls(ids));
    let schemas: BTreeMap<String, Value> = decls
        .into_iter()
        .filter_map(|id| component_schema(ir, ir.decl(id)).map(|s| (component_name(ir, id), s)))
        .collect();
    json!({ "schemas": Value::Object(schemas.into_iter().collect()) })
}

#[cfg(test)]
mod tests;

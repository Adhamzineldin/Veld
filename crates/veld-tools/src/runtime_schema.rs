//! The JSON Schema (draft 2020-12) of one target's runtime config (`spec/runtime-config.md`
//! section 6), built from the shared model in `veld_codegen::runtime_config`.
//!
//! The output is deterministic: every object's keys are sorted, and the `required` lists are in
//! path order.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use veld_codegen::runtime_config::{RuntimeModel, Setting, SettingKind};

/// The schema's dialect.
pub const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// The case-insensitive scheme test of the URL rule (`normalize_url`). The schema cannot say
/// the rest of the rule, so a value that passes the schema can still fail the generated check.
const URL_PATTERN: &str = "^[Hh][Tt][Tt][Pp][Ss]?://";

/// A node of the nested settings: a group of children or one setting.
enum Node<'a> {
    Group(BTreeMap<&'a str, Node<'a>>),
    Leaf(&'a Setting),
}

/// The schema of the runtime config of `model`: `title` is `<Target>Runtime`, every object has
/// `additionalProperties: false`, URLs have `format: "uri"` and lists have `enum`.
#[must_use]
pub fn runtime_schema(model: &RuntimeModel) -> Value {
    let mut root = BTreeMap::new();
    for setting in model.settings() {
        insert(&mut root, &setting.segments, setting);
    }
    let Value::Object(mut doc) = group(&root) else {
        return Value::Null;
    };
    doc.insert("$schema".into(), Value::from(DIALECT));
    doc.insert("title".into(), Value::from(model.type_name()));
    sorted(doc)
}

fn insert<'a>(
    level: &mut BTreeMap<&'a str, Node<'a>>,
    segments: &'a [String],
    setting: &'a Setting,
) {
    let Some((first, rest)) = segments.split_first() else {
        return;
    };
    if rest.is_empty() {
        level.insert(first.as_str(), Node::Leaf(setting));
        return;
    }
    let entry = level
        .entry(first.as_str())
        .or_insert_with(|| Node::Group(BTreeMap::new()));
    if let Node::Group(inner) = entry {
        insert(inner, rest, setting);
    }
}

fn node(n: &Node<'_>) -> Value {
    match n {
        Node::Group(children) => group(children),
        Node::Leaf(setting) => leaf(setting),
    }
}

fn group(children: &BTreeMap<&str, Node<'_>>) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for (key, child) in children {
        properties.insert((*key).to_string(), node(child));
        if requires(child) {
            required.push(Value::from(*key));
        }
    }
    sorted(
        [
            ("additionalProperties", Value::Bool(false)),
            ("properties", Value::Object(properties)),
            ("required", Value::Array(required)),
            ("type", Value::from("object")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect(),
    )
}

/// Whether the child must be present: a required setting, or a group that holds one.
fn requires(n: &Node<'_>) -> bool {
    match n {
        Node::Leaf(s) => s.required,
        Node::Group(children) => children.values().any(requires),
    }
}

fn leaf(s: &Setting) -> Value {
    let mut m = Map::new();
    match s.kind {
        SettingKind::Url => {
            m.insert("format".into(), json!("uri"));
            m.insert("minLength".into(), json!(1));
            m.insert("pattern".into(), json!(URL_PATTERN));
        }
        SettingKind::OneOf(values) => {
            m.insert("enum".into(), json!(values));
        }
    }
    if let Some(d) = &s.default {
        m.insert("default".into(), json!(d));
    }
    m.insert("type".into(), json!("string"));
    sorted(m)
}

/// The same object with its keys in sorted order (the map keeps insertion order).
fn sorted(map: Map<String, Value>) -> Value {
    let ordered: BTreeMap<String, Value> = map.into_iter().collect();
    Value::Object(ordered.into_iter().collect())
}

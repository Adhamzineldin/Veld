//! `@version` and `@idempotent` in the OpenAPI export (`spec/rules.md` sections 11.7 to 11.9): the
//! `If-Match` and idempotency headers of a request, the `ETag` of a success, the replay header,
//! and the `x-veld-version` and `x-veld-idempotent` extensions. The `412` and `409` answers are
//! the endpoint's own errors and come from the IR like any other. Nothing here is written for a
//! contract without the annotations, so its document is unchanged.

use serde_json::{Map, Value, json};
use veld_ir::{Decl, DeclKind, Endpoint, Ir};

/// The header that says a stored answer was sent again.
const REPLAY: &str = "Veld-Idempotent-Replay";

/// The request headers of an endpoint's guard and key.
pub(super) fn headers(ep: &Endpoint) -> Vec<Value> {
    let mut out = Vec::new();
    if ep.rules.version.is_some() {
        out.push(json!({
            "name": "If-Match",
            "in": "header",
            "required": false,
            "description": "The version the caller read: `*`, or one strong entity tag such as `\"3\"`. A different version answers 412; a malformed value answers 422.",
            "schema": { "type": "string", "pattern": "^(\\*|\"(0|[1-9][0-9]*)\")$" },
        }));
    }
    if let Some(i) = &ep.idempotency {
        out.push(json!({
            "name": i.header,
            "in": "header",
            "required": i.required,
            "description": "An idempotency key. The answer of the first request with this key is sent again for the same parameters; other parameters, or a request still running, answer 409.",
            "schema": { "type": "string", "minLength": 1, "maxLength": 255 },
        }));
    }
    out
}

fn header_entry(description: &str) -> Value {
    json!({ "description": description, "schema": { "type": "string" } })
}

fn put_header(response: &mut Value, name: &str, entry: Value) {
    let Some(r) = response.as_object_mut() else {
        return;
    };
    let headers = r
        .entry("headers")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(h) = headers.as_object_mut() {
        h.insert(name.to_owned(), entry);
    }
}

/// `ETag` on a success that is a model with a `@version` field, and the replay header.
pub(super) fn success_headers(ir: &Ir, ep: &Endpoint, response: &mut Value) {
    if ir.etag_field(ep).is_some() {
        let text = "The version of the returned value, as a strong entity tag: `\"3\"`.";
        put_header(response, "ETag", header_entry(text));
    }
    if ep.idempotency.is_some() {
        put_header(response, REPLAY, replay_entry());
    }
}

/// The replay header on an error answer that is kept: below `500`, and every answer of a saga,
/// `SagaFailed` included (`spec/platform.md` section 18).
pub(super) fn error_headers(ep: &Endpoint, status: u16, response: &mut Value) {
    if ep.idempotency.is_some() && (status < 500 || ep.saga.is_some()) {
        put_header(response, REPLAY, replay_entry());
    }
}

fn replay_entry() -> Value {
    header_entry("`true` when this is the stored answer of an earlier request with the same key.")
}

/// `x-veld-version` and `x-veld-idempotent` on an operation.
pub(super) fn decorate_operation(ep: &Endpoint, o: &mut Map<String, Value>) {
    if let Some(v) = &ep.rules.version {
        o.insert(
            "x-veld-version".into(),
            json!({ "id": v.id, "fact": v.fact, "field": v.wire }),
        );
    }
    if ep.saga.is_some() {
        o.insert("x-veld-saga".into(), Value::Bool(true));
    }
    if let Some(i) = &ep.idempotency {
        o.insert(
            "x-veld-idempotent".into(),
            json!({ "header": i.header, "required": i.required, "caller": i.caller }),
        );
    }
}

/// `x-veld-version` on the schema of a model: the wire name of its version field.
pub(super) fn decorate_decl(d: &Decl, m: &mut Map<String, Value>) {
    if let DeclKind::Model(model) = &d.kind
        && let Some(f) = model.fields.iter().find(|f| f.version)
    {
        m.insert("x-veld-version".into(), Value::from(f.wire.clone()));
    }
}

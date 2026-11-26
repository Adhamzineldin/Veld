//! `@version` and `@idempotent` in a live run (`spec/rules.md` sections 11.7 to 11.9).
//!
//! * The request carries `If-Match`, the idempotency header with the key of the example, and, for
//!   a store that is not empty, `Veld-Test-Idempotency`: the unpadded base64url of
//!   `{ "state": "stored", "status": 409, "type": "<error signature>" | null, "data": .. }`,
//!   `{ "state": "other" }` or `{ "state": "running" }`. Without the header the store is empty.
//! * The answer is held to what the reference evaluator says: `Veld-Idempotent-Replay: true` on a
//!   stored answer that was sent again and on nothing else, and `ETag` on a success whose type is a
//!   model with a `@version` field.

use serde_json::{Value as Json, json};
use veld_ir::{DeclKind, Endpoint, Ir};
use veld_rules::{Difference, Mismatch, Reported, StoreState, StoredAnswer};

use crate::b64;
use crate::judge::Answer;

/// The request header of a test handler that carries what the store holds for the key.
pub const IDEMPOTENCY: &str = "Veld-Test-Idempotency";
/// The response header of a stored answer that was sent again.
pub const REPLAY: &str = "Veld-Idempotent-Replay";
/// `If-Match`.
pub const IF_MATCH: &str = "If-Match";

/// The headers a filled example adds for the two features.
#[must_use]
pub fn headers(
    ir: &Ir,
    endpoint: &Endpoint,
    request: &veld_rules::Request,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(tag) = &request.if_match {
        out.push((IF_MATCH.to_owned(), tag.clone()));
    }
    let (Some(idem), Some(key)) = (&endpoint.idempotency, &request.key) else {
        return out;
    };
    out.push((idem.header.clone(), key.value.clone()));
    if let Some(state) = state_header(ir, endpoint, &key.store) {
        out.push((IDEMPOTENCY.to_owned(), state));
    }
    out
}

/// The value of `Veld-Test-Idempotency`; `None` for an empty store.
fn state_header(ir: &Ir, endpoint: &Endpoint, store: &StoreState) -> Option<String> {
    let json = match store {
        StoreState::Empty => return None,
        StoreState::Other => json!({ "state": "other" }),
        StoreState::Running => json!({ "state": "running" }),
        StoreState::Stored(a) => stored_json(ir, endpoint, a),
    };
    Some(b64::encode(json.to_string().as_bytes()))
}

fn stored_json(ir: &Ir, endpoint: &Endpoint, a: &StoredAnswer) -> Json {
    let signature = a.error.as_ref().and_then(|name| {
        let declared = endpoint.errors.iter().map(|e| e.decl);
        declared
            .filter_map(|id| ir.decls.get(id.index()))
            .chain(ir.decls.iter())
            .find(|d| d.name == *name && matches!(d.kind, DeclKind::Error(_)))
            .map(|d| d.sig.clone())
    });
    json!({
        "state": "stored",
        "status": a.status,
        "type": signature,
        "data": a.body.clone().unwrap_or(Json::Null),
    })
}

/// Where the answer differs from what the reference evaluator says about the replay and the
/// `ETag`. `None` when they agree.
#[must_use]
pub fn differences(reported: &Reported, answer: &Answer) -> Option<Mismatch> {
    let mut found = Vec::new();
    let replay = answer.header(REPLAY);
    match (reported.replayed, replay) {
        (true, Some("true")) | (false, None) => {}
        (true, other) => found.push(difference(REPLAY, "true", other.unwrap_or("(no header)"))),
        (false, Some(other)) => found.push(difference(REPLAY, "(no header)", other)),
    }
    if (200..300).contains(&answer.status)
        && let Some(expected) = &reported.etag
        && answer.header("etag") != Some(expected.as_str())
    {
        let actual = answer.header("etag").unwrap_or("(no header)");
        found.push(difference("ETag", expected, actual));
    }
    if found.is_empty() {
        return None;
    }
    Some(Mismatch {
        expected: "the headers the reference evaluator gives for the same request".to_owned(),
        actual: format!("HTTP {}", answer.status),
        differences: found,
    })
}

fn difference(header: &str, expected: &str, actual: &str) -> Difference {
    Difference {
        path: format!("header {header}"),
        expected: expected.to_owned(),
        actual: actual.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use veld_rules::{IdempotencyKey, Request};

    use super::*;

    fn ir_and_endpoint() -> (Ir, String) {
        let src = "error Full 409 { capacity: int }\nmodel M { n: int }\nservice S at \"/s\" {\n  @idempotent(header: \"X-Req\")\n  post \"/\" make(body: M) -> 201 M | Full\n}\n";
        let out = veld_compiler::compile(&veld_compiler::CompileInput {
            packages: vec![veld_compiler::PackageInput {
                name: "t".into(),
                kind: veld_ir::PackageKind::Service,
                files: vec![veld_compiler::SourceFile {
                    path: "t.veld".into(),
                    text: src.into(),
                }],
                base_url: None,
            }],
            consumes: Default::default(),
        });
        (out.ir.unwrap(), "make".into())
    }

    fn request(store: StoreState) -> Request {
        Request {
            if_match: Some("\"3\"".into()),
            key: Some(IdempotencyKey {
                value: "k-1".into(),
                store,
            }),
            ..Request::default()
        }
    }

    fn decoded(headers: &[(String, String)]) -> Json {
        let (_, v) = headers.iter().find(|(n, _)| n == IDEMPOTENCY).unwrap();
        serde_json::from_slice(&b64::decode(v).unwrap()).unwrap()
    }

    #[test]
    fn the_headers_of_a_stored_answer_name_the_error_by_its_signature() {
        let (ir, name) = ir_and_endpoint();
        let ep = &ir.services[0]
            .endpoints
            .iter()
            .find(|e| e.name == name)
            .unwrap();
        let stored = StoredAnswer {
            status: 409,
            error: Some("Full".into()),
            body: Some(json!({ "capacity": 2 })),
        };
        let h = headers(&ir, ep, &request(StoreState::Stored(stored)));
        assert!(h.contains(&("If-Match".into(), "\"3\"".into())));
        assert!(h.contains(&("X-Req".into(), "k-1".into())));
        let state = decoded(&h);
        assert_eq!(state["state"], "stored");
        assert_eq!(state["status"], 409);
        assert_eq!(state["data"], json!({ "capacity": 2 }));
        let sig = &ir.decls.iter().find(|d| d.name == "Full").unwrap().sig;
        assert_eq!(state["type"], json!(sig));
    }

    #[test]
    fn a_stored_success_has_no_type_and_other_and_running_have_only_a_state() {
        let (ir, name) = ir_and_endpoint();
        let ep = &ir.services[0]
            .endpoints
            .iter()
            .find(|e| e.name == name)
            .unwrap();
        let ok = StoredAnswer {
            status: 201,
            error: None,
            body: Some(json!({ "n": 1 })),
        };
        let h = headers(&ir, ep, &request(StoreState::Stored(ok)));
        assert_eq!(decoded(&h)["type"], Json::Null);
        for (store, word) in [
            (StoreState::Other, "other"),
            (StoreState::Running, "running"),
        ] {
            let h = headers(&ir, ep, &request(store));
            assert_eq!(decoded(&h), json!({ "state": word }));
        }
    }

    #[test]
    fn an_empty_store_has_no_test_header_and_no_key_means_no_key_header() {
        let (ir, name) = ir_and_endpoint();
        let ep = &ir.services[0]
            .endpoints
            .iter()
            .find(|e| e.name == name)
            .unwrap();
        let h = headers(&ir, ep, &request(StoreState::Empty));
        assert!(h.iter().all(|(n, _)| n != IDEMPOTENCY));
        assert!(h.contains(&("X-Req".into(), "k-1".into())));
        let none = Request {
            if_match: None,
            ..Request::default()
        };
        assert!(headers(&ir, ep, &none).is_empty());
    }

    fn reported(replayed: bool, etag: Option<&str>) -> Reported {
        Reported {
            outcome: veld_rules::Outcome::Passes,
            audited: Vec::new(),
            hook: Vec::new(),
            replayed,
            store: veld_rules::StoreEffect::Untouched,
            checked_version: None,
            body: None,
            etag: etag.map(str::to_owned),
            fingerprint: None,
        }
    }

    fn answer(status: u16, headers: &[(&str, &str)]) -> Answer {
        Answer {
            status,
            headers: headers
                .iter()
                .map(|(n, v)| (n.to_string(), v.to_string()))
                .collect(),
            body: String::new(),
        }
    }

    #[test]
    fn the_replay_header_is_on_a_replay_and_on_nothing_else() {
        let replayed = reported(true, None);
        assert!(
            differences(
                &replayed,
                &answer(201, &[("veld-idempotent-replay", "true")])
            )
            .is_none()
        );
        assert!(differences(&replayed, &answer(201, &[])).is_some());
        let fresh = reported(false, None);
        assert!(differences(&fresh, &answer(201, &[])).is_none());
        assert!(differences(&fresh, &answer(201, &[("veld-idempotent-replay", "true")])).is_some());
    }

    #[test]
    fn a_success_carries_the_etag_the_reference_gives() {
        let r = reported(false, Some("\"7\""));
        assert!(differences(&r, &answer(200, &[("etag", "\"7\"")])).is_none());
        assert!(differences(&r, &answer(200, &[("etag", "\"8\"")])).is_some());
        assert!(differences(&r, &answer(200, &[])).is_some());
        // An error answer has none, and the reference has none for it.
        assert!(differences(&reported(false, None), &answer(412, &[])).is_none());
    }
}

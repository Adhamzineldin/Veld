//! `@idempotent` (`spec/rules.md` sections 11.8 and 11.9): the key, what the store holds for it,
//! the fingerprint of a request and the scope of a key.
//!
//! The reference evaluator has no store. A request names the state the store is in for its key
//! ([`StoreState`]), the way an example does with a `key` line, and the pipeline says what it does
//! about it and what the store holds afterwards ([`StoreEffect`]).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{Map, Value as Json};
use sha2::{Digest, Sha256};
use veld_ir::{DeclId, DeclKind, Endpoint, Ir, Scalar, Type};

/// The longest key a request may carry, in characters.
pub const MAX_KEY_LEN: usize = 255;

/// The key of a request and what the store holds for it.
#[derive(Debug, Clone, PartialEq)]
pub struct IdempotencyKey {
    /// The value of the idempotency header.
    pub value: String,
    pub store: StoreState,
}

/// What the store holds for one key in one scope.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum StoreState {
    /// Nothing.
    #[default]
    Empty,
    /// An answer, stored with the fingerprint of this request.
    Stored(StoredAnswer),
    /// The key is held with another fingerprint.
    Other,
    /// The first request has not finished.
    Running,
}

/// A stored answer: the status and the body. The server also keeps the content type and the
/// `ETag` and `Location` headers when the answer had them; they follow from these two.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredAnswer {
    pub status: u16,
    /// The name of the error, `None` for a success.
    pub error: Option<String>,
    /// The body as wire JSON: the fields of the error, or the value of the success. `None` for an
    /// answer without a body.
    pub body: Option<Json>,
}

/// What the store holds for the key once the request is over.
#[derive(Debug, Clone, PartialEq)]
pub enum StoreEffect {
    /// The request had no key, or was a replay, or was refused with `409 Conflict`: nothing
    /// changes.
    Untouched,
    /// The key was new and the request ran to its answer, which is below `500`: it is stored.
    Stored(StoredAnswer),
    /// The key was new and the request ended with a `5xx` answer: the entry is removed.
    Removed,
    /// The key was new and the run stopped before the handler (an offline run without `returns`):
    /// the entry is still "running".
    Running,
}

/// The scope of a key: the operation, and the caller when the endpoint has auth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// The operation signature.
    pub operation: String,
    /// The text the application gives for the caller, `None` for an endpoint without auth.
    pub caller: Option<String>,
}

impl Scope {
    /// The scope of `endpoint` for `caller`. A caller is part of the scope only when the endpoint's
    /// idempotency says so.
    #[must_use]
    pub fn of(endpoint: &Endpoint, caller: Option<&str>) -> Scope {
        let uses_caller = endpoint.idempotency.as_ref().is_some_and(|i| i.caller);
        Scope {
            operation: endpoint.sig.clone(),
            caller: caller.filter(|_| uses_caller).map(str::to_owned),
        }
    }
}

/// True for a key a request can carry: 1 to [`MAX_KEY_LEN`] visible ASCII characters.
#[must_use]
pub fn is_valid_key(key: &str) -> bool {
    (1..=MAX_KEY_LEN).contains(&key.len()) && key.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

/// The fingerprint of a request: the first 16 hex characters of the SHA-256 of the RFC 8785 JSON of
/// the bound parameters in their canonical form ([`hashed_params`]), keyed by parameter name. The header of the key is not a
/// parameter and is not part of it.
#[must_use]
pub fn fingerprint(params: &BTreeMap<String, Json>) -> String {
    let object: Map<String, Json> = params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let text = veld_codegen::canonical::canonical(&Json::Object(object));
    let digest = Sha256::digest(text.as_bytes());
    let mut out = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The bound parameters as the fingerprint sees them (`spec/rules.md` section 11.10): what the
/// server understands of the request. A member the model does not have and a `@readonly` member
/// are dropped at any depth, a `long` is the canonical decimal string of its value, and a `uuid`
/// and a `datetime` are in the canonical form of an event's `data` (`envelope::canon`).
pub(crate) fn hashed_params(
    ir: &Ir,
    endpoint: &Endpoint,
    params: &BTreeMap<String, Json>,
) -> BTreeMap<String, Json> {
    let mut out = params.clone();
    for p in &endpoint.params {
        if let Some(value) = out.get_mut(&p.name) {
            prune(ir, &p.ty, value);
            crate::envelope::canon::scalars(ir, &p.ty, value);
        }
    }
    out
}

fn prune(ir: &Ir, ty: &Type, value: &mut Json) {
    match ty {
        Type::Scalar(Scalar::Long) => {
            if let Some(n) = value.as_str().and_then(|s| s.parse::<i64>().ok()) {
                *value = Json::String(n.to_string());
            }
        }
        Type::Scalar(_) => {}
        Type::List(t) => {
            for item in value.as_array_mut().into_iter().flatten() {
                prune(ir, t, item);
            }
        }
        Type::Map(t) => {
            for item in value
                .as_object_mut()
                .into_iter()
                .flat_map(|m| m.values_mut())
            {
                prune(ir, t, item);
            }
        }
        Type::Page(t) => {
            if let Some(items) = value.get_mut("items") {
                prune(ir, &Type::List(t.clone()), items);
            }
        }
        Type::Decl(id) => prune_decl(ir, *id, value, None),
    }
}

/// `keep` is a member to keep besides the fields: the discriminator of a union.
fn prune_decl(ir: &Ir, id: DeclId, value: &mut Json, keep: Option<&str>) {
    let Some(decl) = ir.decls.get(id.index()) else {
        return;
    };
    match &decl.kind {
        DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
            let Some(obj) = value.as_object_mut() else {
                return;
            };
            let fields = decl.fields();
            obj.retain(|k, _| {
                Some(k.as_str()) == keep || fields.iter().any(|f| !f.readonly && &f.wire == k)
            });
            for f in fields.iter().filter(|f| !f.readonly) {
                if let Some(child) = obj.get_mut(&f.wire) {
                    prune(ir, &f.ty, child);
                }
            }
        }
        DeclKind::Union(u) => {
            let label = value.get(&u.discriminator).and_then(Json::as_str);
            if let Some(arm) = u.arms.iter().find(|a| Some(a.label.as_str()) == label) {
                let (arm, disc) = (arm.decl, u.discriminator.clone());
                prune_decl(ir, arm, value, Some(&disc));
            }
        }
        DeclKind::Enum(_) | DeclKind::Const(_) => {}
    }
}

/// The `message` of the issue at the header of the key when it is not a key.
pub const KEY_MESSAGE: &str = "must be 1 to 255 visible ASCII characters";

/// The `message` of the `409 Conflict` for a key held with another fingerprint.
pub const OTHER_MESSAGE: &str = "this idempotency key was used with other parameters";

/// The `message` of the `409 Conflict` for a key whose first request is still running.
pub const RUNNING_MESSAGE: &str = "a request with this idempotency key is still running";

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn params(v: &Json) -> BTreeMap<String, Json> {
        v.as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    /// The fingerprint is defined on the canonical text, so these are goldens of the format:
    /// `sha256` of the RFC 8785 JSON, first 8 bytes, lowercase hex.
    #[test]
    fn the_fingerprint_is_the_first_16_hex_characters_of_the_sha256_of_the_canonical_json() {
        assert_eq!(fingerprint(&BTreeMap::new()), "44136fa355b3678a");
        let p = params(&json!({ "b": 2, "a": "x" }));
        let same = params(&json!({ "a": "x", "b": 2 }));
        assert_eq!(fingerprint(&p), fingerprint(&same));
        assert_eq!(fingerprint(&p).len(), 16);
    }

    /// Goldens with real values: `sha256` of the canonical text, written down by hand.
    #[test]
    fn the_fingerprint_has_goldens() {
        let two = params(&json!({ "body": { "guests": 2 } }));
        assert_eq!(fingerprint(&two), "74ff6d8fbb076cfc");
        let three = params(&json!({ "body": { "guests": 3 } }));
        assert_eq!(fingerprint(&three), "19b1b33ecd669a6f");
    }

    #[test]
    fn the_fingerprint_follows_the_parameters_and_nothing_else() {
        let a = fingerprint(&params(&json!({ "body": { "guests": 2 } })));
        let b = fingerprint(&params(&json!({ "body": { "guests": 3 } })));
        let c = fingerprint(&params(&json!({ "body": { "guests": 2 }, "extra": 1 })));
        assert_ne!(a, b);
        assert_ne!(a, c);
        // Key order inside a value does not matter either.
        let d = fingerprint(&params(&json!({ "body": { "x": 1, "y": 2 } })));
        let e = fingerprint(&params(&json!({ "body": { "y": 2, "x": 1 } })));
        assert_eq!(d, e);
    }

    #[test]
    fn a_key_is_one_to_255_visible_ascii_characters() {
        assert!(is_valid_key("k"));
        assert!(is_valid_key(&"k".repeat(255)));
        assert!(!is_valid_key(""));
        assert!(!is_valid_key(&"k".repeat(256)));
        for bad in ["a b", "a\tb", "é", "a\u{7f}", "a\nb"] {
            assert!(!is_valid_key(bad), "{bad:?}");
        }
    }
}

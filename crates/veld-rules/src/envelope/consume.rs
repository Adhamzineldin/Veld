//! Decoding: steps 1 to 4 of `spec/events.md` section 8.

use serde_json::{Map, Value as Json};
use thiserror::Error;
use veld_ir::{DeclId, DeclKind, Ir};

use super::{DATA_CONTENT_TYPE, RULES_ATTRIBUTE, TENANT_ATTRIBUTE, type_name};
use crate::check::{Options, Violation, check_decl_full};

/// Why a message is not handed to a handler (`spec/events.md` section 8).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ConsumeError {
    /// Step 1: not a CloudEvents 1.0 JSON message of Veld. Rejected as `malformed`.
    #[error("malformed message: {0}")]
    Malformed(String),
    /// Step 2: the `type` names no event this subscription handles. Acknowledged and counted;
    /// nothing else happens. Not a failure of the message.
    #[error("the type `{0}` is not handled here")]
    UnknownType(String),
    /// Step 3: the data does not have the shape of the consumer's own type. Rejected as `shape`.
    #[error("the data does not have the shape of the event: {0}")]
    Shape(String),
    /// Step 4: an invariant is violated or could not be evaluated. Rejected as `rule`.
    #[error("an invariant of the event is violated")]
    Rule(Vec<Violation>),
}

impl ConsumeError {
    /// The reason of a rejected message (`malformed`, `shape` or `rule`); `None` for a message
    /// that is merely ignored.
    #[must_use]
    pub fn rejection(&self) -> Option<&'static str> {
        match self {
            ConsumeError::Malformed(_) => Some("malformed"),
            ConsumeError::Shape(_) => Some("shape"),
            ConsumeError::Rule(_) => Some("rule"),
            ConsumeError::UnknownType(_) => None,
        }
    }
}

/// A message that reached the handler: the parts a consumer needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Received {
    /// The consumer's own declaration of the event.
    pub event: DeclId,
    pub id: String,
    pub source: String,
    /// The `type` attribute as the publisher wrote it.
    pub type_: String,
    pub subject: Option<String>,
    pub time: Option<String>,
    /// The `veldrules` attribute as the publisher wrote it.
    pub veldrules: Option<String>,
    /// The `veldtenant` attribute: the tenant text of an event with a `@tenant` field
    /// (`spec/platform.md` section 3). A consumer does not check it against `data`.
    pub veldtenant: Option<String>,
    /// The event in wire form with the consumer's defaults applied. Fields the consumer does
    /// not know stay in it; a typed consumer ignores them.
    pub data: Json,
    /// True when step 4 ran: the digests were equal (both absent counts as equal).
    pub rules_checked: bool,
    /// The hash of `type` is not the consumer's own (`onContractMismatch`).
    pub type_differs: bool,
}

impl Received {
    /// The digest differs from the consumer's own: step 4 was skipped (`onContractMismatch`).
    #[must_use]
    pub fn rules_differ(&self) -> bool {
        !self.rules_checked
    }
}

fn malformed<T>(why: impl Into<String>) -> Result<T, ConsumeError> {
    Err(ConsumeError::Malformed(why.into()))
}

/// A required attribute that is a non-empty string.
fn required<'a>(m: &'a Map<String, Json>, name: &str) -> Result<&'a str, ConsumeError> {
    match m.get(name) {
        Some(Json::String(s)) if !s.is_empty() => Ok(s),
        Some(Json::String(_)) => malformed(format!("`{name}` is empty")),
        Some(_) => malformed(format!("`{name}` is not a string")),
        None => malformed(format!("`{name}` is missing")),
    }
}

/// An attribute that may be absent and is a string when present.
fn optional(m: &Map<String, Json>, name: &str) -> Result<Option<String>, ConsumeError> {
    match m.get(name) {
        None => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.clone())),
        Some(_) => malformed(format!("`{name}` is not a string")),
    }
}

/// Step 1.
fn envelope(bytes: &[u8]) -> Result<Map<String, Json>, ConsumeError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| ConsumeError::Malformed("not UTF-8".into()))?;
    let Json::Object(m) = serde_json::from_str::<Json>(text)
        .map_err(|e| ConsumeError::Malformed(format!("not JSON: {e}")))?
    else {
        return malformed("not a JSON object");
    };
    if m.get("specversion") != Some(&Json::from("1.0")) {
        return malformed("`specversion` is not \"1.0\"");
    }
    for name in ["id", "source", "type"] {
        required(&m, name)?;
    }
    for name in [
        "subject",
        "time",
        "datacontenttype",
        RULES_ATTRIBUTE,
        TENANT_ATTRIBUTE,
    ] {
        optional(&m, name)?;
    }
    if let Some(Json::String(ct)) = m.get("datacontenttype")
        && ct != DATA_CONTENT_TYPE
    {
        return malformed(format!("`datacontenttype` is `{ct}`"));
    }
    if m.contains_key("data_base64") {
        return malformed("`data_base64` is not allowed");
    }
    if !matches!(m.get("data"), Some(Json::Object(_))) {
        return malformed("`data` is not an object");
    }
    Ok(m)
}

/// Step 2: the declaration among `handled` whose name is the one of `type`.
fn find_event(ir: &Ir, handled: &[DeclId], type_: &str) -> Result<DeclId, ConsumeError> {
    let name = type_name(type_);
    handled
        .iter()
        .copied()
        .find(|id| {
            ir.decls
                .get(id.index())
                .is_some_and(|d| d.is_event() && type_name(&d.sig) == name)
        })
        .ok_or_else(|| ConsumeError::UnknownType(type_.to_owned()))
}

/// The consumer: `spec/events.md` section 8, steps 1 to 4, for the events `handled` (the ones the
/// subscription has an `on` line for).
///
/// # Errors
/// [`ConsumeError`]: `Malformed` (step 1; also a `datacontenttype` that is not
/// `application/json`, or an attribute the spec names that is not a string), `UnknownType`
/// (step 2, to be ignored), `Shape` (step 3) and `Rule` (step 4).
pub fn decode(ir: &Ir, handled: &[DeclId], bytes: &[u8]) -> Result<Received, ConsumeError> {
    let m = envelope(bytes)?;
    let type_ = required(&m, "type")?.to_owned();
    let event = find_event(ir, handled, &type_)?;
    let Some(decl) = ir.decls.get(event.index()) else {
        return Err(ConsumeError::UnknownType(type_));
    };
    let veldrules = optional(&m, RULES_ATTRIBUTE)?;
    let own = match &decl.kind {
        DeclKind::Event(e) => e.rules_digest.clone(),
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Enum(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_) => None,
    };
    let data = m.get("data").cloned().unwrap_or(Json::Null);
    let (data, violations) = check_decl_full(ir, decl, &data, Options::default())
        .map_err(|e| ConsumeError::Shape(e.to_string()))?;
    let rules_checked = veldrules == own;
    if rules_checked && !violations.is_empty() {
        return Err(ConsumeError::Rule(violations));
    }
    Ok(Received {
        event,
        id: required(&m, "id")?.to_owned(),
        source: required(&m, "source")?.to_owned(),
        type_differs: type_ != decl.sig,
        type_,
        subject: optional(&m, "subject")?,
        time: optional(&m, "time")?,
        veldrules,
        veldtenant: optional(&m, TENANT_ATTRIBUTE)?,
        data,
        rules_checked,
    })
}

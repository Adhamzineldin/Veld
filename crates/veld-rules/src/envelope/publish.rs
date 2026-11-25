//! Encoding: steps 1 to 5 of `spec/events.md` section 7.

use serde_json::{Map, Value as Json};
use thiserror::Error;
use veld_codegen::canonical::canonical_bytes;
use veld_ir::{ChannelId, DeclId, DeclKind, Ir, Scalar, ServiceId, Type};

use super::{DATA_CONTENT_TYPE, RULES_ATTRIBUTE};
use crate::check::{Options, Violation, check_decl_full};
use crate::error::InputError;
use crate::wire::format_datetime;

/// What a publisher supplies besides the event: the id source and the clock (section 7).
#[derive(Debug, Clone, Copy)]
pub struct Inputs<'a> {
    /// A non-empty string, unique among the events of one `source`.
    pub id: &'a str,
    /// `veld:<package>/<Service>`, see [`source_of`].
    pub source: &'a str,
    /// The publisher's clock at `publish`, in milliseconds since the epoch.
    pub time_ms: i64,
}

/// Why nothing is sent (`spec/events.md` section 7: the typed `ValidationError`, or a misuse).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PublishError {
    #[error("the channel is not in the IR")]
    NoSuchChannel,
    #[error("`{0}` is not an event carried by the channel")]
    NotCarried(String),
    #[error("the value does not have the shape of the event: {0}")]
    Shape(InputError),
    #[error("an invariant of the event is violated")]
    Invalid(Vec<Violation>),
    #[error("the key `{0}` of the channel is empty")]
    EmptyKey(String),
    #[error("the id is empty")]
    EmptyId,
    #[error("the source is empty")]
    EmptySource,
    #[error("the key `{0}` is not a string, uuid, int or long on the wire")]
    BadKey(String),
}

/// The `source` attribute of a service: `veld:` package `/` Service.
#[must_use]
pub fn source_of(ir: &Ir, service: ServiceId) -> String {
    let s = ir.service(service);
    format!("veld:{}/{}", ir.package(s.package).name, s.name)
}

/// The key text of the event (`spec/events.md` section 4): a `string` as it is, a `uuid` in
/// lowercase, an `int` or `long` in decimal digits. `None` when the channel has no key.
fn key_text(ir: &Ir, channel: ChannelId, data: &Json) -> Result<Option<String>, PublishError> {
    let Some(key) = ir.channel(channel).key.as_ref() else {
        return Ok(None);
    };
    let bad = || PublishError::BadKey(key.wire.clone());
    let value = data.get(&key.wire).ok_or_else(bad)?;
    let text = match (&key.ty, value) {
        (Type::Scalar(Scalar::String), Json::String(s)) => s.clone(),
        (Type::Scalar(Scalar::Uuid), Json::String(s)) => s.clone(),
        (Type::Scalar(Scalar::Int), Json::Number(n)) => n.to_string(),
        (Type::Scalar(Scalar::Long), Json::String(s)) => s.clone(),
        (Type::Scalar(Scalar::Long), Json::Number(n)) => n.to_string(),
        _ => return Err(bad()),
    };
    if text.is_empty() {
        return Err(PublishError::EmptyKey(key.wire.clone()));
    }
    Ok(Some(text))
}

/// The canonical bytes of the message that publishing `value` (the event in wire form) as
/// `event` on `channel` sends: the shape and the invariants are checked first, defaults are
/// applied to `data`, and nothing is produced for a value that would not be sent.
///
/// # Errors
/// [`PublishError`] for a value that does not have the shape of the event, one that violates an
/// invariant (audited rules never reject), an empty key, an empty id or source, or an event the
/// channel does not carry.
pub fn encode(
    ir: &Ir,
    channel: ChannelId,
    event: DeclId,
    value: &Json,
    inputs: &Inputs<'_>,
) -> Result<Vec<u8>, PublishError> {
    let ch = ir
        .channels
        .get(channel.index())
        .ok_or(PublishError::NoSuchChannel)?;
    let decl = ir.decls.get(event.index());
    let (decl, rules_digest) = match decl.map(|d| (d, &d.kind)) {
        Some((d, DeclKind::Event(e))) if ch.events.contains(&event) => (d, e.rules_digest.clone()),
        _ => return Err(PublishError::NotCarried(event.0.to_string())),
    };
    let mut canonical_value = value.clone();
    super::canon::scalars(ir, &Type::Decl(event), &mut canonical_value);
    let (data, found) = check_decl_full(ir, decl, &canonical_value, Options::default())
        .map_err(PublishError::Shape)?;
    if !found.is_empty() {
        return Err(PublishError::Invalid(found));
    }
    if inputs.id.is_empty() {
        return Err(PublishError::EmptyId);
    }
    if inputs.source.is_empty() {
        return Err(PublishError::EmptySource);
    }
    let mut m = Map::new();
    m.insert("specversion".into(), Json::from("1.0"));
    m.insert("id".into(), Json::from(inputs.id));
    m.insert("source".into(), Json::from(inputs.source));
    m.insert("type".into(), Json::from(decl.sig.as_str()));
    if let Some(subject) = key_text(ir, channel, &data)? {
        m.insert("subject".into(), Json::from(subject));
    }
    m.insert("time".into(), Json::from(format_datetime(inputs.time_ms)));
    m.insert("datacontenttype".into(), Json::from(DATA_CONTENT_TYPE));
    if let Some(d) = rules_digest {
        m.insert(RULES_ATTRIBUTE.into(), Json::from(d));
    }
    m.insert("data".into(), data);
    Ok(canonical_bytes(&Json::Object(m)))
}

//! The dead-letter copy of `spec/events.md` section 10.5, as a pure function.
//!
//! The copy is published to `<topic>.dlq` with the key of the original message. When the original
//! bytes are a JSON object, the copy is that object with `veldreason`, `veldgroup`,
//! `veldattempts` and, when they are not empty, `veldissues` and `veldviolations`, written as
//! canonical JSON (RFC 8785). Anything else is copied as it is. The goldens are
//! `tests/wire/events/dead-letter.json`.

use serde::Serialize;
use serde_json::{Map, Value as Json};
use veld_codegen::canonical::{canonical, canonical_bytes};

use crate::check::Violation;
use crate::error::InputError;

use super::ConsumeError;

/// The attributes the copy adds. A member of the original with one of these names is replaced,
/// or removed when the copy has nothing to say for it, so a copy that fails again describes its
/// own failure (`spec/events.md` section 10.5).
pub const REASON_MEMBER: &str = "veldreason";
pub const GROUP_MEMBER: &str = "veldgroup";
pub const ATTEMPTS_MEMBER: &str = "veldattempts";
pub const ISSUES_MEMBER: &str = "veldissues";
pub const VIOLATIONS_MEMBER: &str = "veldviolations";

/// Why a message is copied to the dead-letter topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Step 1 of `spec/events.md` section 8.
    Malformed,
    /// Step 3.
    Shape,
    /// Step 4.
    Rule,
    /// The retries of the handler are used up (section 10.4).
    Handler,
}

impl Reason {
    /// The word on the wire.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Reason::Malformed => "malformed",
            Reason::Shape => "shape",
            Reason::Rule => "rule",
            Reason::Handler => "handler",
        }
    }

    /// The reason of a rejected message; `None` for a message that is only ignored.
    #[must_use]
    pub fn of_rejection(e: &ConsumeError) -> Option<Reason> {
        match e {
            ConsumeError::Malformed(_) => Some(Reason::Malformed),
            ConsumeError::Shape(_) => Some(Reason::Shape),
            ConsumeError::Rule(_) => Some(Reason::Rule),
            ConsumeError::UnknownType(_) => None,
        }
    }
}

/// One issue of a rejected message, the `ValidationIssue` of the prelude: a path, a code and a
/// message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub path: String,
    pub code: String,
    pub message: String,
}

impl Issue {
    /// The issue that tells a consumer why a value does not have the shape of its event.
    #[must_use]
    pub fn of_input_error(e: &InputError) -> Issue {
        Issue {
            path: "$".to_owned(),
            code: "type".to_owned(),
            message: e.to_string(),
        }
    }
}

/// The bytes of the dead-letter copy of `original`.
///
/// `attempts` is the number of runs of the handler (0 for a rejected message). `issues` and
/// `violations` are written only when they are not empty, each as a string that holds the
/// canonical JSON of the list. Nothing of a handler's error is copied.
#[must_use]
pub fn dead_letter_copy(
    original: &[u8],
    reason: Reason,
    group: &str,
    attempts: u32,
    issues: &[Issue],
    violations: &[Violation],
) -> Vec<u8> {
    let Some(Json::Object(mut object)) = parse_object(original) else {
        return original.to_vec();
    };
    for name in [ISSUES_MEMBER, VIOLATIONS_MEMBER] {
        object.remove(name);
    }
    object.insert(REASON_MEMBER.to_owned(), Json::from(reason.name()));
    object.insert(GROUP_MEMBER.to_owned(), Json::from(group));
    object.insert(ATTEMPTS_MEMBER.to_owned(), Json::from(attempts));
    list_member(&mut object, ISSUES_MEMBER, issues);
    list_member(&mut object, VIOLATIONS_MEMBER, violations);
    canonical_bytes(&Json::Object(object))
}

fn parse_object(bytes: &[u8]) -> Option<Json> {
    let text = std::str::from_utf8(bytes).ok()?;
    let value = serde_json::from_str::<Json>(text).ok()?;
    value.is_object().then_some(value)
}

/// `name`: the canonical JSON of `items` as a string, when there are items.
fn list_member<T: Serialize>(object: &mut Map<String, Json>, name: &str, items: &[T]) {
    if items.is_empty() {
        return;
    }
    let list = Json::Array(
        items
            .iter()
            .map(|i| serde_json::to_value(i).unwrap_or(Json::Null))
            .collect(),
    );
    object.insert(name.to_owned(), Json::from(canonical(&list)));
}

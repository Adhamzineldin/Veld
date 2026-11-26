//! `@version` (`spec/rules.md` sections 11.7 to 11.10): the `If-Match` header and the `ETag` of a
//! response.

use std::fmt;

use serde::{Serialize, Serializer};

/// What a caller says in `If-Match`: any current version, or the one version it read.
///
/// A client method takes it as `if_match`; the server reads it from the request. `V` is the type
/// of the version field of the model (`i64` for a `long`, `i32` for an `int`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IfMatch<V = i64> {
    /// `*`: any current version passes.
    Any,
    /// One strong entity tag holding the version the caller read: `"3"`.
    Version(V),
}

impl<V: fmt::Display> IfMatch<V> {
    /// The value of the header: `*` or `"3"`.
    #[must_use]
    pub fn header(&self) -> String {
        match self {
            IfMatch::Any => "*".to_string(),
            IfMatch::Version(v) => format!("\"{v}\""),
        }
    }
}

impl<V: fmt::Display> Serialize for IfMatch<V> {
    /// The header text, so that the client core sends it as it sends any header parameter.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.header())
    }
}

/// The message of the issue at `header.If-Match` when the header is malformed.
pub const IF_MATCH_MESSAGE: &str = "must be * or one strong entity tag holding a version, such as \"3\"";

/// Reads the value of an `If-Match` header: `*`, or one strong entity tag whose text is a version
/// in decimal digits (`0`, or `1-9` and more digits, no sign, within `long`). Anything else, a weak
/// tag or a list included, is `None`.
#[must_use]
pub fn parse_if_match(text: &str) -> Option<IfMatch<i64>> {
    if text == "*" {
        return Some(IfMatch::Any);
    }
    let digits = text.strip_prefix('"')?.strip_suffix('"')?;
    let canonical = digits == "0"
        || (digits.starts_with(|c: char| ('1'..='9').contains(&c)) && digits.bytes().all(|b| b.is_ascii_digit()));
    if canonical { digits.parse().ok().map(IfMatch::Version) } else { None }
}

/// The `ETag` of a success body in wire form: the version in field `field`, as a strong entity tag.
/// A `long` is a string on the wire and an `int` a number.
#[must_use]
pub fn etag_of(wire: &serde_json::Value, field: &str) -> Option<String> {
    match wire.get(field)? {
        serde_json::Value::Number(n) => Some(format!("\"{n}\"")),
        serde_json::Value::String(s) => Some(format!("\"{s}\"")),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => None,
    }
}

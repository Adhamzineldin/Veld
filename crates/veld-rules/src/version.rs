//! `@version` (`spec/rules.md` sections 11.7 and 11.9): the `If-Match` header, the version of a
//! fact and the `ETag` of a response.

use serde_json::Value as Json;
use veld_ir::{Endpoint, Ir};

/// The `message` of the issue at `header.If-Match` when the header is malformed.
pub const IF_MATCH_MESSAGE: &str =
    "must be * or one strong entity tag holding a version, such as \"3\"";

/// What `If-Match` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IfMatch {
    /// `*`: any current version passes.
    Any,
    /// One strong entity tag, `"3"`: the version the caller read.
    Version(i64),
}

impl IfMatch {
    /// The header value: `*` or `"3"`.
    #[must_use]
    pub fn header(self) -> String {
        match self {
            IfMatch::Any => "*".to_owned(),
            IfMatch::Version(n) => etag_text(&n.to_string()),
        }
    }
}

/// Reads the value of an `If-Match` header: `*`, or one strong entity tag whose text is a version
/// in decimal digits (no sign, no leading zero, within `long`). Anything else, a weak tag or a
/// list included, is `None`, and the request is `422 ValidationError` (section 11.9).
#[must_use]
pub fn parse_if_match(text: &str) -> Option<IfMatch> {
    if text == "*" {
        return Some(IfMatch::Any);
    }
    let digits = text.strip_prefix('"')?.strip_suffix('"')?;
    parse_version(digits).map(IfMatch::Version)
}

/// A version written in decimal digits: `0`, or a digit from 1 to 9 and more digits; at most the
/// largest `long`.
#[must_use]
pub fn parse_version(digits: &str) -> Option<i64> {
    let canonical = digits == "0"
        || (digits.starts_with(|c: char| ('1'..='9').contains(&c))
            && digits.bytes().all(|b| b.is_ascii_digit()));
    if canonical { digits.parse().ok() } else { None }
}

/// The version stored in `value`, a field of a model by its wire name: a number (an `int`), or a
/// string of digits (a `long`).
#[must_use]
pub fn version_of(value: &Json, wire: &str) -> Option<i64> {
    match value.get(wire)? {
        Json::Number(n) => n.as_i64(),
        Json::String(s) => s.parse().ok(),
        Json::Null | Json::Bool(_) | Json::Array(_) | Json::Object(_) => None,
    }
}

/// `"3"`: a version as a strong entity tag.
#[must_use]
pub fn etag_text(version: &str) -> String {
    format!("\"{version}\"")
}

/// The `ETag` of a success response: the version of the returned value, as a strong entity tag.
/// `None` when the endpoint's success is not a model with a `@version` field, or the value has no
/// version.
#[must_use]
pub fn etag(ir: &Ir, endpoint: &Endpoint, body: &Json) -> Option<String> {
    let field = ir.etag_field(endpoint)?;
    version_of(body, &field.wire).map(|v| etag_text(&v.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_if_match_is_a_star_or_one_strong_tag_of_digits() {
        assert_eq!(parse_if_match("*"), Some(IfMatch::Any));
        assert_eq!(parse_if_match("\"3\""), Some(IfMatch::Version(3)));
        assert_eq!(parse_if_match("\"0\""), Some(IfMatch::Version(0)));
        let max = i64::MAX.to_string();
        assert_eq!(
            parse_if_match(&format!("\"{max}\"")),
            Some(IfMatch::Version(i64::MAX))
        );
    }

    #[test]
    fn everything_else_is_malformed() {
        for bad in [
            "",
            "3",
            "\"\"",
            "\"03\"",
            "\"-1\"",
            "\"+1\"",
            "\"1.5\"",
            "\"a\"",
            "W/\"3\"",
            "\"3\", \"4\"",
            "* ",
            " \"3\"",
            "\"3",
            "3\"",
            "'3'",
            "**",
            "\"9223372036854775808\"",
            "\"٣\"",
        ] {
            assert_eq!(parse_if_match(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_header_of_a_version_reads_back() {
        for v in [IfMatch::Any, IfMatch::Version(0), IfMatch::Version(41)] {
            assert_eq!(parse_if_match(&v.header()), Some(v));
        }
    }

    #[test]
    fn a_version_is_read_from_a_number_or_a_string() {
        let v = serde_json::json!({ "rev": 4, "big": "9007199254740993", "name": "x" });
        assert_eq!(version_of(&v, "rev"), Some(4));
        assert_eq!(version_of(&v, "big"), Some(9_007_199_254_740_993));
        assert_eq!(version_of(&v, "name"), None);
        assert_eq!(version_of(&v, "missing"), None);
    }
}

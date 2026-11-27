//! The HTTP request a live example stands for.
//!
//! It is the request a generated client sends (`spec/targets/typescript.md`): the method and the
//! full path of the endpoint with the path parameters encoded, `@query` parameters and the
//! members of a query object as query pairs (a list repeats its key, absent optionals are left
//! out, keys are sorted), `@header` parameters as headers, and the body as JSON in wire form.
//! On top of that come the test headers of `spec/rules.md` section 12.4.

use std::fmt::Write as _;

use serde_json::{Map, Value as Json};
use veld_ir::{Binding, Endpoint, Ir, Method, Param};
use veld_rules::FilledExample;

use crate::b64;
use crate::conditional;
use crate::error::LiveError;

/// The header that carries the facts: base64url of a JSON object.
pub const FACTS: &str = "Veld-Test-Facts";
/// The header that carries the principal: base64url of its wire JSON. Absent without one.
pub const PRINCIPAL: &str = "Veld-Test-Principal";
/// The header that carries the clock: an RFC 3339 datetime, as text.
pub const NOW: &str = "Veld-Test-Now";
/// The header that carries the flags that are on: base64url of a JSON array of names. Absent
/// when no flag is on (`spec/platform.md` section 11).
pub const FLAGS: &str = "Veld-Test-Flags";
/// The header that carries the handler's result: base64url of its wire JSON.
pub const RESULT: &str = "Veld-Test-Result";
/// `1` asks the test handler to stop after step 6 and answer `204`.
pub const DRY_RUN: &str = "Veld-Test-Dry-Run";
/// The response header of a dry run that passed.
pub const PASSED: &str = "Veld-Test-Passed";
/// The path a test handler answers to tell itself from a production server.
pub const PROBE_PATH: &str = "/__veld/test";

/// A request, as plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// `GET`, `POST`, ...
    pub method: String,
    /// The path with its query string, as sent after the base URL.
    pub target: String,
    /// In the order they are sent.
    pub headers: Vec<(String, String)>,
    /// The JSON text of the body, when the endpoint has one.
    pub body: Option<String>,
}

impl HttpRequest {
    /// The value of the header `name` (ASCII case-insensitive), if the request has it.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// The service endpoint a filled example calls.
pub(crate) fn endpoint_of<'a>(ir: &'a Ir, filled: &FilledExample) -> Option<&'a Endpoint> {
    ir.services
        .get(filled.service.index())?
        .endpoints
        .iter()
        .find(|e| e.name == filled.endpoint)
}

/// Builds the request of a filled example. With `dry_run`, an example without `returns` also
/// sends `Veld-Test-Dry-Run: 1`.
///
/// # Errors
/// [`LiveError::Unsupported`] for a WebSocket or streaming endpoint, and for a parameter whose
/// value cannot be written into a path, a query string or a header.
pub fn build_request(
    ir: &Ir,
    filled: &FilledExample,
    dry_run: bool,
) -> Result<HttpRequest, LiveError> {
    let unsupported = |reason: String| LiveError::Unsupported {
        example: filled.name.clone(),
        reason,
    };
    let endpoint = endpoint_of(ir, filled)
        .ok_or_else(|| unsupported(format!("no endpoint `{}`", filled.endpoint)))?;
    if endpoint.method == Method::Ws || endpoint.success.stream {
        return Err(unsupported(format!(
            "`{}` is a stream endpoint",
            endpoint.name
        )));
    }
    let bound = Bound::collect(endpoint, filled).map_err(unsupported)?;
    let mut headers = bound.headers;
    if bound.body.is_some() {
        headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
    }
    headers.push(("Accept".to_owned(), "application/json".to_owned()));
    headers.extend(test_headers(filled, dry_run));
    headers.extend(conditional::headers(ir, endpoint, &filled.request));
    Ok(HttpRequest {
        method: endpoint.method.as_str().to_owned(),
        target: bound.target,
        headers,
        body: bound.body,
    })
}

/// The parameters of a request, placed.
struct Bound {
    target: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

impl Bound {
    fn collect(endpoint: &Endpoint, filled: &FilledExample) -> Result<Bound, String> {
        let mut path = endpoint.path.clone();
        let mut query: Vec<(String, String)> = Vec::new();
        let mut headers = Vec::new();
        let mut body = None;
        for p in endpoint.params.iter().filter(|p| !p.stream) {
            let value = filled.request.params.get(&p.name);
            match (p.binding, value) {
                (Binding::Path, Some(v)) => path = put_in_path(&path, p, v)?,
                (Binding::Path, None) => {
                    return Err(format!("path parameter `{}` has no value", p.name));
                }
                (Binding::Query, Some(v)) => push_pairs(&mut query, &p.name, v, &p.name)?,
                (Binding::QueryObject, Some(v)) => push_object(&mut query, v, &p.name)?,
                (Binding::Header, Some(v)) => {
                    let text = scalar_text(v)
                        .ok_or_else(|| format!("header `{}` is not a scalar", p.name))?;
                    let name = p.header.clone().unwrap_or_else(|| p.name.clone());
                    headers.push((name, text));
                }
                (Binding::Body, Some(v)) => body = Some(v.to_string()),
                (Binding::Query | Binding::QueryObject | Binding::Header | Binding::Body, None) => {
                }
            }
        }
        query.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Bound {
            target: with_query(path, &query),
            headers,
            body,
        })
    }
}

/// Replaces `{name}` in the path template by the encoded value.
fn put_in_path(path: &str, p: &Param, v: &Json) -> Result<String, String> {
    let text =
        scalar_text(v).ok_or_else(|| format!("path parameter `{}` is not a scalar", p.name))?;
    Ok(path.replace(&format!("{{{}}}", p.name), &encode_component(&text)))
}

fn with_query(mut path: String, query: &[(String, String)]) -> String {
    for (i, (k, v)) in query.iter().enumerate() {
        path.push(if i == 0 { '?' } else { '&' });
        path.push_str(&encode_component(k));
        path.push('=');
        path.push_str(&encode_component(v));
    }
    path
}

/// A query parameter: one pair for a scalar, one per item for a list.
fn push_pairs(
    out: &mut Vec<(String, String)>,
    key: &str,
    v: &Json,
    what: &str,
) -> Result<(), String> {
    if let Json::Array(items) = v {
        for item in items {
            let text = scalar_text(item)
                .ok_or_else(|| format!("query parameter `{what}` has a nested value"))?;
            out.push((key.to_owned(), text));
        }
        return Ok(());
    }
    let text = scalar_text(v).ok_or_else(|| format!("query parameter `{what}` is not a scalar"))?;
    out.push((key.to_owned(), text));
    Ok(())
}

/// A query object: one key per member, by its wire name.
fn push_object(out: &mut Vec<(String, String)>, v: &Json, what: &str) -> Result<(), String> {
    let Json::Object(members) = v else {
        return Err(format!("query object `{what}` is not an object"));
    };
    for (key, member) in members {
        if !member.is_null() {
            push_pairs(out, key, member, what)?;
        }
    }
    Ok(())
}

/// The text of a scalar wire value, as it appears in a URL or a header.
fn scalar_text(v: &Json) -> Option<String> {
    match v {
        Json::String(s) => Some(s.clone()),
        Json::Number(n) => Some(n.to_string()),
        Json::Bool(b) => Some(b.to_string()),
        Json::Null | Json::Array(_) | Json::Object(_) => None,
    }
}

/// `encodeURIComponent`: everything but letters, digits and `-_.~` is percent-encoded.
fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// The test headers of section 12.4.
fn test_headers(filled: &FilledExample, dry_run: bool) -> Vec<(String, String)> {
    let req = &filled.request;
    let facts: Map<String, Json> = req
        .facts
        .iter()
        .map(|(k, v)| (k.clone(), v.clone().unwrap_or(Json::Null)))
        .collect();
    let b64_json = |v: &Json| b64::encode(v.to_string().as_bytes());
    let mut out = vec![(FACTS.to_owned(), b64_json(&Json::Object(facts)))];
    if let Some(p) = &req.principal {
        out.push((PRINCIPAL.to_owned(), b64_json(p)));
    }
    out.push((NOW.to_owned(), filled.clock.clone()));
    if !filled.flags.is_empty() {
        out.push((
            FLAGS.to_owned(),
            b64_json(&Json::from(filled.flags.clone())),
        ));
    }
    match &req.result {
        Some(r) => out.push((RESULT.to_owned(), b64_json(r))),
        None if dry_run => out.push((DRY_RUN.to_owned(), "1".to_owned())),
        None => {}
    }
    out
}

//! The audited rules of a live answer (`spec/rules.md` sections 12.4 and 17.1).
//!
//! A test handler says which audited rules (`@rollout(audit)`) a request made it report in the
//! response header `Veld-Test-Audited`: the unpadded base64url of a compact JSON array, at most
//! the first 100 reports, absent when there is none. Without that header a server that never
//! reports an audited rule would pass every example, because letting the request through is all
//! an example can see. The entries are compared with what the reference evaluator reports for the
//! same example, as a set of `(rule, path, error)`.

use std::collections::BTreeSet;

use serde_json::Value as Json;
use veld_rules::{Difference, Mismatch, Violation};

use crate::b64;
use crate::judge::Answer;

/// The response header of a test handler that reported audited rules.
pub const AUDITED: &str = "Veld-Test-Audited";

/// A test handler reports at most this many audited rules per answer.
pub const LIMIT: usize = 100;

/// One report: the rule id, the path and the evaluation error, if any.
type Key = (String, String, Option<String>);

fn key_of(v: &Violation) -> Key {
    let error = v.error.map(|e| e.as_str().to_owned());
    (v.rule_id.clone(), v.path.clone(), error)
}

fn describe(key: &Key) -> String {
    match &key.2 {
        Some(e) => format!("{} at {} ({e})", key.0, key.1),
        None => format!("{} at {}", key.0, key.1),
    }
}

/// The entries of the header: `Err` with the reason when it is not what section 12.4 says.
fn reported(answer: &Answer) -> Result<Vec<Key>, String> {
    let Some(text) = answer.header(AUDITED) else {
        return Ok(Vec::new());
    };
    let bytes = b64::decode(text).ok_or("the header is not base64url")?;
    let json: Json = serde_json::from_slice(&bytes).map_err(|e| format!("not JSON: {e}"))?;
    let list = json.as_array().ok_or("not a JSON array")?;
    let mut out = Vec::new();
    for entry in list {
        let text = |name: &str| entry.get(name).and_then(Json::as_str).map(str::to_owned);
        let (Some(rule), Some(path)) = (text("rule"), text("path")) else {
            return Err("an entry has no `rule` or `path`".to_owned());
        };
        out.push((rule, path, text("error")));
    }
    Ok(out)
}

/// Where the audited rules the server reported differ from the ones the reference evaluator
/// reports. `None` when they agree. A reference that reports more than [`LIMIT`] rules only
/// requires every entry of the header to be one of them, and the header to hold at most [`LIMIT`].
/// Without `exact` the server may report more than the reference knows (it ran the application's
/// handler on an answer that the reference could not see): it must report at least what the
/// reference does.
#[must_use]
pub fn differences(reference: &[Violation], answer: &Answer, exact: bool) -> Option<Mismatch> {
    let want: BTreeSet<Key> = reference.iter().map(key_of).collect();
    let got = match reported(answer) {
        Ok(list) => list,
        Err(why) => return Some(unreadable(&want, &why)),
    };
    let sent: BTreeSet<Key> = got.iter().cloned().collect();
    let (missing, unexpected): (Vec<&Key>, Vec<&Key>) = if reference.len() > LIMIT {
        let extra = sent.difference(&want).collect();
        let too_many = if got.len() > LIMIT {
            sent.iter().collect()
        } else {
            vec![]
        };
        (too_many, extra)
    } else {
        let extra = if exact {
            sent.difference(&want).collect()
        } else {
            Vec::new()
        };
        (want.difference(&sent).collect(), extra)
    };
    if missing.is_empty() && unexpected.is_empty() {
        return None;
    }
    let mut differences = Vec::new();
    for key in missing {
        differences.push(Difference {
            path: format!("header {AUDITED}"),
            expected: describe(key),
            actual: "not reported".to_owned(),
        });
    }
    for key in unexpected {
        differences.push(Difference {
            path: format!("header {AUDITED}"),
            expected: "not reported".to_owned(),
            actual: describe(key),
        });
    }
    Some(Mismatch {
        expected: format!("audited rules: {}", list(&want)),
        actual: format!("audited rules: {}", list(&sent)),
        differences,
    })
}

fn unreadable(want: &BTreeSet<Key>, why: &str) -> Mismatch {
    Mismatch {
        expected: format!("audited rules: {}", list(want)),
        actual: format!("a `{AUDITED}` header that cannot be read ({why})"),
        differences: vec![Difference {
            path: format!("header {AUDITED}"),
            expected: "the unpadded base64url of a JSON array".to_owned(),
            actual: why.to_owned(),
        }],
    }
}

fn list(keys: &BTreeSet<Key>) -> String {
    if keys.is_empty() {
        return "none".to_owned();
    }
    keys.iter().map(describe).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(header: Option<&str>) -> Answer {
        Answer {
            status: 200,
            headers: header
                .map(|h| vec![("veld-test-audited".to_owned(), h.to_owned())])
                .unwrap_or_default(),
            body: String::new(),
        }
    }

    fn encoded(json: &str) -> String {
        b64::encode(json.as_bytes())
    }

    #[test]
    fn no_header_and_nothing_audited_agree() {
        assert!(differences(&[], &answer(None), true).is_none());
    }

    #[test]
    fn a_header_the_reference_does_not_expect_differs() {
        let h = encoded(r#"[{"rule":"r1","path":"body","fields":[],"message":"m"}]"#);
        let m = differences(&[], &answer(Some(&h)), true).unwrap();
        assert_eq!(m.differences.len(), 1);
        assert!(m.differences[0].actual.contains("r1 at body"));
    }

    #[test]
    fn a_report_the_server_never_made_differs() {
        let reference = [violation("r1", "body")];
        let m = differences(&reference, &answer(None), true).unwrap();
        assert!(m.differences[0].expected.contains("r1 at body"));
        assert_eq!(m.differences[0].actual, "not reported");
    }

    #[test]
    fn the_same_set_agrees_whatever_its_order() {
        let reference = [violation("r1", "body"), violation("r2", "request")];
        let h = encoded(
            r#"[{"rule":"r2","path":"request","fields":[],"message":""},{"rule":"r1","path":"body","fields":[],"message":""}]"#,
        );
        assert!(differences(&reference, &answer(Some(&h)), true).is_none());
    }

    #[test]
    fn an_unreadable_header_fails() {
        let m = differences(&[], &answer(Some("%%%")), true).unwrap();
        assert!(m.actual.contains("cannot be read"));
    }

    #[test]
    fn past_the_limit_every_entry_must_be_one_the_reference_knows() {
        let reference: Vec<Violation> = (0..LIMIT + 5)
            .map(|i| violation(&format!("r{i}"), "body"))
            .collect();
        let known: Vec<String> = (0..LIMIT)
            .map(|i| format!(r#"{{"rule":"r{i}","path":"body","fields":[],"message":""}}"#))
            .collect();
        let h = encoded(&format!("[{}]", known.join(",")));
        assert!(differences(&reference, &answer(Some(&h)), true).is_none());
        let stranger = encoded(r#"[{"rule":"zz","path":"body","fields":[],"message":""}]"#);
        assert!(differences(&reference, &answer(Some(&stranger)), true).is_some());
    }

    #[test]
    fn without_exact_the_server_may_report_more_but_not_less() {
        let reference = [violation("r1", "body")];
        let h = encoded(
            r#"[{"rule":"r1","path":"body","fields":[],"message":""},{"rule":"r2","path":"result","fields":[],"message":""}]"#,
        );
        assert!(differences(&reference, &answer(Some(&h)), false).is_none());
        assert!(differences(&reference, &answer(Some(&h)), true).is_some());
        assert!(differences(&reference, &answer(None), false).is_some());
    }

    fn violation(rule: &str, path: &str) -> Violation {
        Violation {
            rule_name: rule.to_owned(),
            rule_id: rule.to_owned(),
            path: path.to_owned(),
            fields: Vec::new(),
            message: "m".to_owned(),
            error: None,
        }
    }
}

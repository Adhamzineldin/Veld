//! The events a live answer says were written (`spec/events.md` section 11.1).
//!
//! The test handler of a server with an emitting endpoint gives every request a memory outbox and
//! puts the response header `Veld-Test-Emitted` on its answer: the declaration names of the
//! events written, separated by `,` without spaces, in the order they were written. The header is
//! absent when nothing was written. When the request has `Veld-Test-Result` the handler is not
//! called and nothing is written; the header then names the events of the edge taken, which a
//! handler that succeeded must have written. `expect emitted A` and `expect emitted B` say the
//! header is exactly `A,B` in any order; `expect not emitted` says it is absent.

use std::collections::BTreeSet;

use veld_rules::{Difference, Mismatch};

use crate::judge::Answer;

/// The response header that names the events written.
pub const EMITTED: &str = "Veld-Test-Emitted";

/// The names the header lists, in order: `Err` with the reason when it is not a list of names.
fn listed(answer: &Answer) -> Result<Vec<String>, String> {
    let Some(text) = answer.header(EMITTED) else {
        return Ok(Vec::new());
    };
    let names: Vec<String> = text.split(',').map(str::to_owned).collect();
    if names.iter().any(|n| n.is_empty() || n.trim() != n) {
        return Err(format!(
            "`{text}` is not a list of names separated by `,` without spaces"
        ));
    }
    Ok(names)
}

fn text(names: &[String]) -> String {
    if names.is_empty() {
        "nothing".to_owned()
    } else {
        names.join(", ")
    }
}

/// Where the events the answer lists differ from the ones the example expects, by declaration
/// name. `None` when they are the same set and none is listed twice.
#[must_use]
pub fn differences(expected: &[String], answer: &Answer) -> Option<Mismatch> {
    let mut want: Vec<String> = expected.to_vec();
    want.sort();
    let got = match listed(answer) {
        Ok(list) => list,
        Err(why) => return Some(unreadable(&want, &why)),
    };
    let mut sorted = got.clone();
    sorted.sort();
    if sorted == want {
        return None;
    }
    let got_set: BTreeSet<&String> = got.iter().collect();
    let want_set: BTreeSet<&String> = want.iter().collect();
    let mut differences = Vec::new();
    for name in want_set.difference(&got_set) {
        differences.push(difference(name, "written", "not written"));
    }
    for name in got_set.difference(&want_set) {
        differences.push(difference(name, "not written", "written"));
    }
    for name in &got_set {
        if got.iter().filter(|g| g == name).count() > 1 {
            differences.push(difference(name, "written once", "written more than once"));
        }
    }
    Some(Mismatch {
        expected: format!("events written: {}", text(&want)),
        actual: format!("events written: {}", text(&got)),
        differences,
    })
}

fn difference(name: &str, expected: &str, actual: &str) -> Difference {
    Difference {
        path: format!("event {name}"),
        expected: expected.to_owned(),
        actual: actual.to_owned(),
    }
}

fn unreadable(want: &[String], why: &str) -> Mismatch {
    Mismatch {
        expected: format!("events written: {}", text(want)),
        actual: format!("a `{EMITTED}` header that cannot be read ({why})"),
        differences: vec![Difference {
            path: format!("header {EMITTED}"),
            expected: "the names of the events, separated by `,`".to_owned(),
            actual: why.to_owned(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(header: Option<&str>) -> Answer {
        Answer {
            status: 200,
            headers: header
                .map(|h| vec![("veld-test-emitted".to_owned(), h.to_owned())])
                .unwrap_or_default(),
            body: String::new(),
        }
    }

    #[test]
    fn no_header_agrees_with_not_emitted() {
        assert!(differences(&[], &answer(None)).is_none());
    }

    #[test]
    fn the_same_events_agree_whatever_their_order() {
        let want = ["BookingPaid".to_owned(), "RefundDue".to_owned()];
        assert!(differences(&want, &answer(Some("RefundDue,BookingPaid"))).is_none());
        let m = differences(&want, &answer(Some("RefundDue"))).unwrap();
        assert_eq!(m.differences[0].expected, "written");
        assert_eq!(m.differences[0].actual, "not written");
        assert!(differences(&want, &answer(None)).is_some());
    }

    #[test]
    fn a_listed_event_the_example_does_not_expect_differs() {
        let m = differences(&[], &answer(Some("BookingPaid"))).unwrap();
        assert_eq!(m.differences.len(), 1);
        assert_eq!(m.differences[0].actual, "written");
    }

    #[test]
    fn an_event_listed_twice_or_with_spaces_is_not_accepted() {
        let twice = differences(&[], &answer(Some("A,A"))).unwrap();
        assert!(
            twice
                .differences
                .iter()
                .any(|d| d.actual.contains("more than once"))
        );
        assert!(differences(&[], &answer(Some("A, B"))).is_some());
        assert!(differences(&[], &answer(Some("A,,B"))).is_some());
    }
}

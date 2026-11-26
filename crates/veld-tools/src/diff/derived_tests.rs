//! `veld diff` and derived fields (`spec/rules.md` section 17.2): a field that is added, becomes
//! derived, stops being derived, or whose expression changes.

use super::ChangeKind::{self, *};
use super::Severity::{self, Breaking as B, NonBreaking as N, PossiblyBreaking as P};
use super::diff;
use crate::testutil::ir;

fn kinds(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| (c.kind, c.severity))
        .collect();
    v.sort();
    v
}

fn expect(old: &str, new: &str, want: &[(ChangeKind, Severity)]) {
    let mut w = want.to_vec();
    w.sort();
    assert_eq!(kinds(old, new), w, "old:\n{old}\nnew:\n{new}");
}

/// `M` with the fields of `body`, answered by one endpoint and taken by another.
fn both(body: &str) -> String {
    format!(
        "model M {{\n  a: int\n{body}\n}}\nservice S at \"/s\" {{\n  post \"/\" put(body: M) -> 201 M\n}}"
    )
}

fn response(body: &str) -> String {
    format!("model M {{\n  a: int\n{body}\n}}\nservice S at \"/s\" {{\n  get \"/\" one() -> M\n}}")
}

fn request(body: &str) -> String {
    format!(
        "model M {{\n  a: int\n{body}\n}}\nservice S at \"/s\" {{\n  post \"/\" put(body: M) -> 204\n}}"
    )
}

#[test]
fn a_derived_field_added_is_not_a_required_field_added() {
    // A caller never sends it, so adding it to a request type breaks nobody.
    expect(
        &request(""),
        &request("  b: int = a + 1"),
        &[(FieldAdded, N)],
    );
    expect(&both(""), &both("  b: int = a + 1"), &[(FieldAdded, N)]);
    expect(
        &response(""),
        &response("  b: int = a + 1"),
        &[(FieldAdded, N)],
    );
}

#[test]
fn a_plain_required_field_added_to_a_request_is_still_breaking() {
    expect(
        &request(""),
        &request("  b: int"),
        &[(RequiredFieldAdded, B)],
    );
}

#[test]
fn a_changed_expression_is_possibly_breaking_in_every_position() {
    for wrap in [request, response, both] {
        expect(
            &wrap("  b: int = a + 1"),
            &wrap("  b: int = a + 2"),
            &[(DerivedChanged, P)],
        );
    }
}

#[test]
fn a_changed_expression_names_both_expressions() {
    let changes = diff(
        &ir(&response("  b: int = a + 1")),
        &ir(&response("  b: int = a * 2")),
    );
    assert_eq!(changes.len(), 1);
    let m = &changes[0].message;
    assert!(
        m.contains("a + 1") && m.contains("a * 2") && m.contains("`b`"),
        "{m}"
    );
    assert!(m.starts_with("possibly breaking:"), "{m}");
}

#[test]
fn the_same_expression_written_differently_is_no_change() {
    expect(&both("  b: int = a+1"), &both("  b: int = (a + 1)"), &[]);
}

#[test]
fn a_field_that_becomes_derived_is_possibly_breaking_where_callers_send_it() {
    expect(
        &request("  b: int"),
        &request("  b: int = a + 1"),
        &[(FieldBecameDerived, P)],
    );
    expect(
        &both("  b: int"),
        &both("  b: int = a + 1"),
        &[(FieldBecameDerived, P)],
    );
    // A default that becomes an expression: the same.
    expect(
        &request("  b: int = 0"),
        &request("  b: int = a + 1"),
        &[(FieldBecameDerived, P)],
    );
}

#[test]
fn a_field_that_becomes_derived_in_a_response_only_changes_nothing_a_consumer_sees() {
    expect(
        &response("  b: int"),
        &response("  b: int = a + 1"),
        &[(FieldBecameDerived, N)],
    );
}

#[test]
fn a_field_that_stops_being_derived_is_required_of_callers_and_not_of_consumers() {
    // The field is "required" now, which only matters where callers send the type: a note here.
    expect(
        &response("  b: int = a + 1"),
        &response("  b: int"),
        &[(FieldNoLongerDerived, N), (FieldBecameRequired, N)],
    );
    expect(
        &request("  b: int = a + 1"),
        &request("  b: int"),
        &[(FieldNoLongerDerived, N), (FieldBecameRequired, B)],
    );
}

#[test]
fn a_derived_field_that_is_removed_is_a_removed_field() {
    expect(
        &response("  b: int = a + 1"),
        &response(""),
        &[(FieldRemoved, B)],
    );
}

#[test]
fn a_contract_without_derived_fields_reports_what_it_always_did() {
    expect(&both("  b: int"), &both("  b: int"), &[]);
    expect(&both(""), &both("  b?: int"), &[(FieldAdded, N)]);
}

// --- an event with a derived field --------------------------------------------------------------------

fn event(body: &str) -> String {
    format!(
        "event Paid {{\n  id: uuid\n  net: int\n{body}\n}}\nchannel C {{ carries Paid }}\n\
         service P at \"/p\" {{\n  publishes C\n  get \"/\" ping() -> 204\n}}\n"
    )
}

#[test]
fn a_changed_expression_of_an_event_changes_the_event() {
    // An event is compared by its signature and its rules digest, which covers the ids of its
    // derived fields: a rule and an expression are alike in that.
    expect(
        &event("  gross: int = net + 1"),
        &event("  gross: int = net + 2"),
        &[(EventChanged, P)],
    );
    expect(
        &event("  gross: int = net + 1"),
        &event("  gross: int = (net + 1)"),
        &[],
    );
}

#[test]
fn a_derived_field_added_to_an_event_changes_the_event_like_any_field() {
    expect(
        &event(""),
        &event("  gross: int = net + 1"),
        &[(EventChanged, P)],
    );
}

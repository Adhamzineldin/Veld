//! `veld diff` and `@rollout(audit)` (`spec/rules.md` section 17.1): adding the annotation to an
//! existing rule is judged like removing the rule, removing it like adding the rule, and a new
//! rule that arrives audited is compatible.

use super::ChangeKind::{self, *};
use super::Severity::{self, Breaking as B, NonBreaking as N, PossiblyBreaking as P};
use super::{Change, diff, has_breaking};
use crate::testutil::ir;

fn changes(old: &str, new: &str) -> Vec<Change> {
    diff(&ir(old), &ir(new))
}

fn kinds(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = changes(old, new)
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

const REQUEST: &str = "service S at \"/s\" { post \"/\" make(body: M) -> 204 }";
const RESPONSE: &str = "service S at \"/s\" { get \"/\" fetch() -> M }";

fn model(rules: &str, svc: &str) -> String {
    format!("model M {{\n a: int\n{rules}\n}}\n{svc}")
}

const ENFORCED: &str = " rule positive: a > 0";
const AUDITED: &str = " @rollout(audit)\n rule positive: a > 0";
const OTHER: &str = " rule big: a < 100";

// --- invariants -------------------------------------------------------------------------------

#[test]
fn auditing_an_enforced_invariant_is_judged_like_removing_it() {
    // Requests only: removing a rule loosens them.
    expect(
        &model(ENFORCED, REQUEST),
        &model(AUDITED, REQUEST),
        &[(InvariantRemoved, N)],
    );
    // Responses: consumers lose a guarantee.
    expect(
        &model(ENFORCED, RESPONSE),
        &model(AUDITED, RESPONSE),
        &[(InvariantRemoved, B)],
    );
}

#[test]
fn enforcing_an_audited_invariant_is_judged_like_adding_it() {
    expect(
        &model(AUDITED, REQUEST),
        &model(ENFORCED, REQUEST),
        &[(InvariantAdded, B)],
    );
    expect(
        &model(AUDITED, RESPONSE),
        &model(ENFORCED, RESPONSE),
        &[(InvariantAdded, N)],
    );
}

#[test]
fn a_new_invariant_that_arrives_audited_is_compatible() {
    expect(&model("", REQUEST), &model(AUDITED, REQUEST), &[]);
    expect(&model("", RESPONSE), &model(AUDITED, RESPONSE), &[]);
    assert!(!has_breaking(&changes(
        &model("", REQUEST),
        &model(AUDITED, REQUEST)
    )));
}

#[test]
fn an_audited_invariant_that_goes_away_or_changes_is_compatible() {
    expect(&model(AUDITED, REQUEST), &model("", REQUEST), &[]);
    expect(
        &model(AUDITED, REQUEST),
        &model(" @rollout(audit)\n rule positive: a > 5", REQUEST),
        &[],
    );
}

#[test]
fn the_message_says_why_the_change_is_judged_that_way() {
    let c = changes(&model(ENFORCED, REQUEST), &model(AUDITED, REQUEST));
    assert!(c[0].message.contains("now audited"), "{}", c[0].message);
    let c = changes(&model(AUDITED, REQUEST), &model(ENFORCED, REQUEST));
    assert!(
        c[0].message.contains("no longer audited"),
        "{}",
        c[0].message
    );
}

#[test]
fn other_rules_of_the_declaration_are_judged_as_before() {
    expect(
        &model(&format!("{ENFORCED}\n{OTHER}"), REQUEST),
        &model(&format!("{AUDITED}\n{OTHER}"), REQUEST),
        &[(InvariantRemoved, N)],
    );
    expect(
        &model(&format!("{AUDITED}\n{OTHER}"), REQUEST),
        &model(&format!("{AUDITED}\n rule big: a < 50"), REQUEST),
        &[(InvariantChanged, P)],
    );
}

#[test]
fn a_contract_without_the_annotation_is_diffed_as_before() {
    expect(
        &model("", REQUEST),
        &model(ENFORCED, REQUEST),
        &[(InvariantAdded, B)],
    );
    expect(
        &model(ENFORCED, REQUEST),
        &model("", REQUEST),
        &[(InvariantRemoved, N)],
    );
}

// --- require and ensure -----------------------------------------------------------------------

fn endpoint(block: &str) -> String {
    format!(
        "model M {{ a: int }}\nservice S at \"/s\" {{ post \"/\" make(body: M) -> 201 M {{\n{block}\n}} }}\n"
    )
}

const REQ: &str = "  require named: body.a > 0";
const REQ_AUDITED: &str = "  @rollout(audit)\n  require named: body.a > 0";
const ENS: &str = "  ensure named: result.a > 0";
const ENS_AUDITED: &str = "  @rollout(audit)\n  ensure named: result.a > 0";

#[test]
fn auditing_a_require_is_judged_like_removing_it_and_enforcing_it_like_adding_it() {
    expect(
        &endpoint(REQ),
        &endpoint(REQ_AUDITED),
        &[(RequireRemoved, N)],
    );
    expect(&endpoint(REQ_AUDITED), &endpoint(REQ), &[(RequireAdded, B)]);
    expect(&endpoint(""), &endpoint(REQ_AUDITED), &[]);
}

#[test]
fn auditing_an_ensure_is_judged_like_removing_it_and_enforcing_it_like_adding_it() {
    expect(
        &endpoint(ENS),
        &endpoint(ENS_AUDITED),
        &[(EnsureRemoved, B)],
    );
    expect(&endpoint(ENS_AUDITED), &endpoint(ENS), &[(EnsureAdded, N)]);
    expect(&endpoint(""), &endpoint(ENS_AUDITED), &[]);
}

#[test]
fn an_unnamed_statement_is_matched_by_its_expression() {
    let plain = "  require body.a > 0";
    let audited = "  @rollout(audit)\n  require body.a > 0";
    expect(&endpoint(plain), &endpoint(audited), &[(RequireRemoved, N)]);
    expect(&endpoint(audited), &endpoint(plain), &[(RequireAdded, B)]);
}

#[test]
fn an_audited_statement_that_changes_is_compatible_and_its_else_does_not_count() {
    let a = "  @rollout(audit)\n  require named: body.a > 0";
    let b = "  @rollout(audit)\n  require named: body.a > 9";
    expect(&endpoint(a), &endpoint(b), &[]);
    let ensure_a = "  @rollout(audit)\n  ensure named: result.a > 0";
    let ensure_b = "  @rollout(audit)\n  ensure named: result.a > 9";
    expect(&endpoint(ensure_a), &endpoint(ensure_b), &[]);
}

#[test]
fn the_message_of_a_statement_says_it_is_audited() {
    let c = changes(&endpoint(REQ), &endpoint(REQ_AUDITED));
    assert!(c[0].message.contains("now audited"), "{}", c[0].message);
    let c = changes(&endpoint(ENS_AUDITED), &endpoint(ENS));
    assert!(
        c[0].message.contains("no longer audited"),
        "{}",
        c[0].message
    );
}

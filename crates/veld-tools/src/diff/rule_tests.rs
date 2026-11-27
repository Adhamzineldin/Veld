//! Rule-aware diff: invariants, endpoint rules and state machines.

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

// --- invariants -------------------------------------------------------------------------

const REQUEST: &str = "service S at \"/s\" { post \"/\" make(body: M) -> 204 }";
const RESPONSE: &str = "service S at \"/s\" { get \"/\" fetch() -> M }";
const BOTH: &str = "service S at \"/s\" { post \"/\" make(body: M) -> 201 M }";

fn model(rules: &str) -> String {
    format!("model M {{\n a: int\n b: int\n{rules}\n}}\n")
}

fn with(rules: &str, svc: &str) -> String {
    format!("{}{svc}", model(rules))
}

const RULE: &str = " rule positive: a > 0 message \"a must be positive\"";

#[test]
fn a_rule_added_is_breaking_when_requests_use_it() {
    expect(
        &with("", REQUEST),
        &with(RULE, REQUEST),
        &[(InvariantAdded, B)],
    );
    expect(&with("", BOTH), &with(RULE, BOTH), &[(InvariantAdded, B)]);
}

#[test]
fn a_rule_added_to_a_response_only_type_is_a_note() {
    expect(
        &with("", RESPONSE),
        &with(RULE, RESPONSE),
        &[(InvariantAdded, N)],
    );
}

#[test]
fn a_rule_removed_is_breaking_when_responses_use_it() {
    expect(
        &with(RULE, RESPONSE),
        &with("", RESPONSE),
        &[(InvariantRemoved, B)],
    );
    expect(&with(RULE, BOTH), &with("", BOTH), &[(InvariantRemoved, B)]);
}

#[test]
fn a_rule_removed_from_a_request_only_type_is_looser() {
    expect(
        &with(RULE, REQUEST),
        &with("", REQUEST),
        &[(InvariantRemoved, N)],
    );
}

#[test]
fn a_changed_expression_is_possibly_breaking_in_every_position() {
    let tighter = " rule positive: a > 5 message \"a must be positive\"";
    for svc in [REQUEST, RESPONSE, BOTH] {
        expect(
            &with(RULE, svc),
            &with(tighter, svc),
            &[(InvariantChanged, P)],
        );
    }
}

#[test]
fn a_changed_expression_reports_the_old_and_the_new_text() {
    let new = " rule positive: a > 5 && b > 0 message \"a must be positive\"";
    let c = changes(&with(RULE, REQUEST), &with(new, REQUEST));
    assert_eq!(c.len(), 1, "{c:?}");
    let m = &c[0].message;
    assert!(m.starts_with("possibly breaking: "), "{m}");
    assert!(
        m.contains("`a > 0`") && m.contains("`a > 5 && b > 0`"),
        "{m}"
    );
    assert!(m.contains("requests"), "{m}");
    assert_eq!(c[0].subject, "app:app.M");
}

#[test]
fn a_changed_message_is_a_note() {
    let reworded = " rule positive: a > 0 message \"please send a positive a\"";
    for svc in [REQUEST, RESPONSE, BOTH] {
        expect(
            &with(RULE, svc),
            &with(reworded, svc),
            &[(RuleMessageChanged, N)],
        );
    }
}

#[test]
fn a_rule_that_does_not_change_is_not_reported() {
    expect(&with(RULE, BOTH), &with(RULE, BOTH), &[]);
}

#[test]
fn rules_of_errors_are_response_rules() {
    let err = |rules: &str| {
        format!(
            "error Low 409 {{\n n: int\n{rules}\n}}\nservice S at \"/s\" {{ get \"/\" f() -> 204 | Low }}"
        )
    };
    let rule = " rule nonNegative: n >= 0";
    expect(&err(""), &err(rule), &[(InvariantAdded, N)]);
    expect(&err(rule), &err(""), &[(InvariantRemoved, B)]);
}

#[test]
fn an_inherited_rule_is_reported_once_on_the_parent() {
    let src = |rules: &str| {
        format!(
            "model P {{\n a: int\n{rules}\n}}\nmodel C extends P {{ c: int }}\nservice S at \"/s\" {{ post \"/\" make(body: C) -> 204 }}"
        )
    };
    let c = changes(&src(""), &src(RULE));
    assert_eq!(c.len(), 1, "{c:?}");
    assert_eq!(c[0].subject, "app:app.P");
}

#[test]
fn a_declaration_no_endpoint_uses_is_treated_as_used_both_ways() {
    expect(&model(""), &model(RULE), &[(InvariantAdded, B)]);
    expect(&model(RULE), &model(""), &[(InvariantRemoved, B)]);
}

// --- endpoint rules ---------------------------------------------------------------------

fn endpoint(stmts: &str) -> String {
    format!(
        "model B {{ total: int }}\nerror Bad 422 {{}}\nerror Worse 409 {{}}\nservice S at \"/s\" {{\n post \"/{{id}}/x\" act(id: uuid, body: B) -> B | Bad | Worse {{\n  fact b: B = id\n{stmts}\n }}\n}}\n"
    )
}

const REQ_R: &str = "  require r: body.total > 0 else Bad";
const ENS_E: &str = "  ensure e: result.total == old(b.total)";

#[test]
fn a_require_added_is_breaking_and_removed_is_not() {
    expect(&endpoint(""), &endpoint(REQ_R), &[(RequireAdded, B)]);
    expect(&endpoint(REQ_R), &endpoint(""), &[(RequireRemoved, N)]);
    let unnamed = "  require body.total > 0";
    expect(&endpoint(""), &endpoint(unnamed), &[(RequireAdded, B)]);
    expect(&endpoint(unnamed), &endpoint(""), &[(RequireRemoved, N)]);
}

#[test]
fn a_require_with_a_changed_expression_is_possibly_breaking() {
    let tighter = "  require r: body.total > 5 else Bad";
    expect(&endpoint(REQ_R), &endpoint(tighter), &[(RequireChanged, P)]);
    let c = changes(&endpoint(REQ_R), &endpoint(tighter));
    assert!(
        c[0].message.contains("`body.total > 0`") && c[0].message.contains("`body.total > 5`"),
        "{}",
        c[0].message
    );
    // unnamed statements pair in source order
    expect(
        &endpoint("  require body.total > 0"),
        &endpoint("  require body.total > 5"),
        &[(RequireChanged, P)],
    );
}

#[test]
fn a_changed_else_error_or_payload_is_breaking() {
    expect(
        &endpoint(REQ_R),
        &endpoint("  require r: body.total > 0 else Worse"),
        &[(RequireErrorChanged, B)],
    );
    expect(
        &endpoint(REQ_R),
        &endpoint("  require r: body.total > 0"),
        &[(RequireErrorChanged, B)],
    );
}

#[test]
fn a_changed_require_message_is_a_note() {
    expect(
        &endpoint(&format!("{REQ_R} message \"one\"")),
        &endpoint(&format!("{REQ_R} message \"two\"")),
        &[(RuleMessageChanged, N)],
    );
}

#[test]
fn ensure_rules() {
    expect(&endpoint(ENS_E), &endpoint(""), &[(EnsureRemoved, B)]);
    expect(&endpoint(""), &endpoint(ENS_E), &[(EnsureAdded, N)]);
    expect(
        &endpoint(ENS_E),
        &endpoint("  ensure e: result.total >= old(b.total)"),
        &[(EnsureChanged, P)],
    );
    let c = changes(
        &endpoint(ENS_E),
        &endpoint("  ensure e: result.total >= old(b.total)"),
    );
    assert!(
        c[0].message.contains("`result.total == old(b.total)`"),
        "{}",
        c[0].message
    );
}

#[test]
fn facts_are_notes() {
    let two = "  fact c: B = id";
    let fact_note = |old: &str, new: &str| {
        let c = changes(old, new);
        assert!(!c.is_empty());
        assert!(
            c.iter()
                .filter(|c| c.kind == FactChanged)
                .all(|c| c.severity == N),
            "{c:?}"
        );
        assert_eq!(
            c.iter().filter(|c| c.kind == FactChanged).count(),
            1,
            "{c:?}"
        );
    };
    fact_note(&endpoint(""), &endpoint(two));
    fact_note(&endpoint(two), &endpoint(""));
    fact_note(&endpoint(two), &endpoint("  fact c?: B = id"));
}

#[test]
fn rules_only_changes_are_noted_when_nothing_else_moves() {
    assert!(!has_breaking(&changes(&endpoint(REQ_R), &endpoint(REQ_R))));
}

// --- state machines ---------------------------------------------------------------------

fn machine(flows: &str, transition: &str) -> String {
    format!(
        "enum St {{\n a, b, c, d\n{flows}\n}}\nmodel Item {{ id: uuid\n st: St }}\nservice S at \"/s\" {{\n post \"/{{id}}/go\" go(id: uuid) -> Item {{\n  fact item: Item = id\n{transition}\n }}\n}}\n"
    )
}

const FLOWS: &str = " flow a -> b\n flow b -> c\n flow a -> c";

#[test]
fn flows_removed_are_breaking_and_added_are_not() {
    let fewer = " flow a -> b\n flow b -> c";
    let t = "  transition item.st: a -> b";
    expect(&machine(FLOWS, t), &machine(fewer, t), &[(FlowRemoved, B)]);
    expect(&machine(fewer, t), &machine(FLOWS, t), &[(FlowAdded, N)]);
}

#[test]
fn a_state_made_final_is_reported_through_its_removed_flows() {
    let before = " flow a -> b\n flow b -> c";
    let after = " flow a -> b\n final b";
    let t = "  transition item.st: a -> b";
    let k = kinds(&machine(before, t), &machine(after, t));
    assert_eq!(k, vec![(FlowRemoved, B)]);
}

#[test]
fn a_transition_with_fewer_sources_or_another_target_is_breaking() {
    let f = " flow a -> c\n flow b -> c\n flow a -> b";
    let wide = "  transition item.st: a, b -> c";
    expect(
        &machine(f, wide),
        &machine(f, "  transition item.st: a -> c"),
        &[(TransitionChanged, B)],
    );
    expect(
        &machine(f, "  transition item.st: a -> c"),
        &machine(f, "  transition item.st: a -> b"),
        &[(TransitionChanged, B)],
    );
}

#[test]
fn a_transition_with_more_sources_is_not_breaking() {
    let f = " flow a -> c\n flow b -> c";
    expect(
        &machine(f, "  transition item.st: a -> c"),
        &machine(f, "  transition item.st: a, b -> c"),
        &[(TransitionChanged, N)],
    );
}

#[test]
fn adding_and_removing_a_transition_guard() {
    let f = " flow a -> c";
    let guard = "  transition item.st: a -> c";
    let k = kinds(&machine(f, ""), &machine(f, guard));
    assert!(k.contains(&(TransitionChanged, B)), "{k:?}");
    let k = kinds(&machine(f, guard), &machine(f, ""));
    assert!(k.contains(&(TransitionChanged, N)), "{k:?}");
}

#[test]
fn the_report_for_a_tightened_invariant_is_sorted_and_deterministic() {
    let tight = " rule positive: a > 5 message \"a must be positive\"";
    let a = changes(&with(RULE, BOTH), &with(tight, BOTH));
    let b = changes(&with(RULE, BOTH), &with(tight, BOTH));
    assert_eq!(a, b);
    assert!(has_breaking(&a));
}

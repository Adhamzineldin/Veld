//! The table of `spec/events.md` section 16: a change to an event is judged for both sides of
//! the stream, a channel by what a consumer sees, and `emits` by what the service sends.

use super::ChangeKind::{self, *};
use super::Severity::{self, Breaking as B, NonBreaking as N, PossiblyBreaking as P};
use super::{diff, has_breaking};
use crate::testutil::ir;

fn run(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| (c.kind, c.severity))
        .collect();
    v.sort();
    v
}

fn event(body: &str) -> String {
    format!("event Paid {{\n{body}\n}}\nchannel C {{ carries Paid }}\n")
}

// ---- the fields of an event -----------------------------------------------------------------

#[test]
fn an_optional_field_or_one_with_a_default_is_compatible() {
    let old = event("  id: uuid");
    for added in ["  note?: string", "  nights: int = 1"] {
        let new = event(&format!("  id: uuid\n{added}"));
        assert_eq!(run(&old, &new), [(FieldAdded, N)], "{added}");
        assert!(!has_breaking(&diff(&ir(&old), &ir(&new))));
    }
}

#[test]
fn a_required_field_without_a_default_breaks_a_new_consumer() {
    let old = event("  id: uuid");
    let new = event("  id: uuid\n  amount: decimal");
    assert_eq!(run(&old, &new), [(RequiredFieldAdded, B)]);
    assert!(has_breaking(&diff(&ir(&old), &ir(&new))));
}

#[test]
fn a_required_field_that_is_removed_breaks_an_old_consumer() {
    let old = event("  id: uuid\n  amount: decimal");
    let new = event("  id: uuid");
    assert_eq!(run(&old, &new), [(FieldRemoved, B)]);
}

#[test]
fn an_optional_field_or_one_with_a_default_that_is_removed_is_possibly_breaking() {
    for gone in ["  note?: string", "  nights: int = 1"] {
        let old = event(&format!("  id: uuid\n{gone}"));
        let new = event("  id: uuid");
        assert_eq!(run(&old, &new), [(FieldRemoved, P)], "{gone}");
        assert!(has_breaking(&diff(&ir(&old), &ir(&new))));
    }
}

#[test]
fn a_changed_field_type_is_breaking_both_ways() {
    let old = event("  id: uuid\n  n: int");
    let new = event("  id: uuid\n  n: string");
    assert_eq!(run(&old, &new), [(FieldTypeChanged, B)]);
    assert_eq!(run(&new, &old), [(FieldTypeChanged, B)]);
}

#[test]
fn a_field_that_becomes_required_or_stops_being_required_is_breaking() {
    let optional = event("  id: uuid\n  note?: string");
    let required = event("  id: uuid\n  note: string");
    assert_eq!(run(&optional, &required), [(FieldBecameRequired, B)]);
    assert_eq!(run(&required, &optional), [(FieldBecameOptional, B)]);
    // A default makes a required field optional for a consumer, and takes the same way back.
    let defaulted = event("  id: uuid\n  note: string = \"x\"");
    assert_eq!(run(&required, &defaulted), [(FieldBecameOptional, B)]);
}

#[test]
fn a_tightened_constraint_is_breaking_and_a_loosened_one_possibly_breaking() {
    let loose = event("  id: uuid\n  n: int @min(0)");
    let tight = event("  id: uuid\n  n: int @min(1)");
    assert_eq!(run(&loose, &tight), [(ConstraintTightened, B)]);
    assert_eq!(run(&tight, &loose), [(ConstraintLoosened, P)]);
}

#[test]
fn docs_and_deprecation_of_a_field_are_compatible() {
    let old = event("  id: uuid\n  note?: string");
    let new = event("  id: uuid\n  /// what it is\n  note?: string @deprecated(\"use text\")");
    let changes = run(&old, &new);
    assert!(changes.iter().all(|(_, s)| *s == N), "{changes:?}");
    assert!(!changes.is_empty());
}

// ---- rules ------------------------------------------------------------------------------------

#[test]
fn a_rule_that_is_added_removed_or_changed_is_possibly_breaking() {
    let none = event("  n: int");
    let one = event("  n: int\n  rule r: n > 0");
    let other = event("  n: int\n  rule r: n > 1");
    assert_eq!(run(&none, &one), [(EventChanged, P)]);
    assert_eq!(run(&one, &none), [(EventChanged, P)]);
    assert_eq!(run(&one, &other), [(EventChanged, P)]);
    assert!(run(&one, &one).is_empty());
}

#[test]
fn a_rule_of_a_declaration_the_event_reaches_changes_the_event_too() {
    let with = |rule: &str| {
        format!(
            "model Money {{ amount: int\n {rule} }}\n{}",
            event("  money: Money")
        )
    };
    let changes = run(&with("rule r: amount > 0"), &with("rule r: amount > 1"));
    assert!(changes.contains(&(EventChanged, P)), "{changes:?}");
}

// ---- what an event reaches is read by both sides ------------------------------------------------

#[test]
fn a_value_added_to_an_enum_only_an_event_uses_breaks_an_old_consumer() {
    let with = |values: &str| format!("enum Reason {{ {values} }}\n{}", event("  reason: Reason"));
    let changes = run(&with("a, b"), &with("a, b, c"));
    assert_eq!(changes, [(EnumVariantAdded, B)]);
}

#[test]
fn a_field_added_to_a_model_only_an_event_uses_is_judged_for_both_sides() {
    let with = |fields: &str| format!("model Money {{ {fields} }}\n{}", event("  money: Money"));
    assert_eq!(
        run(&with("amount: int"), &with("amount: int\n note?: string")),
        [(FieldAdded, N)]
    );
    assert!(
        run(
            &with("amount: int"),
            &with("amount: int\n currency: string")
        )
        .contains(&(RequiredFieldAdded, B))
    );
}

// ---- channels -----------------------------------------------------------------------------------

#[test]
fn an_event_removed_from_a_channel_the_topic_or_the_key_is_breaking_and_an_added_event_is_not() {
    let two = "event A { id: uuid }\nevent B { id: uuid }\n";
    let old = format!("{two}channel C {{ carries A, B }}\n");
    let fewer = format!("{two}channel C {{ carries A }}\n");
    let more = format!("{two}channel C {{ carries A, B }}\nchannel D {{ carries A }}\n");
    assert_eq!(run(&old, &fewer), [(ChannelChanged, B)]);
    assert_eq!(run(&fewer, &old), [(ChannelChanged, N)]);
    assert_eq!(run(&old, &more), [(ChannelAdded, N)]);
    let topic = format!("{two}@topic(\"x.y\")\nchannel C {{ carries A, B }}\n");
    assert_eq!(run(&old, &topic), [(ChannelChanged, B)]);
    let key = format!("{two}channel C {{ carries A, B\n key id }}\n");
    assert_eq!(run(&old, &key), [(ChannelChanged, B)]);
    assert_eq!(run(&key, &old), [(ChannelChanged, B)]);
}

#[test]
fn the_messages_name_what_changed() {
    let two = "event A { id: uuid }\nevent B { id: uuid }\n";
    let old = format!("{two}channel C {{ carries A, B }}\n");
    let topic = format!("{two}@topic(\"x.y\")\nchannel C {{ carries A, B }}\n");
    let changes = diff(&ir(&old), &ir(&topic));
    assert!(changes[0].message.contains("x.y"), "{}", changes[0].message);
    let fewer = format!("{two}channel C {{ carries A }}\n");
    let changes = diff(&ir(&old), &ir(&fewer));
    assert!(
        changes[0].message.contains("no longer carries"),
        "{}",
        changes[0].message
    );
    assert!(
        changes[0].message.ends_with("never receives it"),
        "{}",
        changes[0].message
    );
}

// ---- emits --------------------------------------------------------------------------------------

fn machine(flows: &str, publishes: &str) -> String {
    format!(
        "event Paid {{ id: uuid }}\nevent Gone {{ id: uuid }}\n\
         @topic(\"one.v1\")\nchannel C {{ carries Paid, Gone }}\n\
         @topic(\"two.v1\")\nchannel D {{ carries Paid, Gone }}\n\
         enum S {{ a, b, c\n{flows}\n final b, c }}\n\
         model M {{ id: uuid, s: S }}\n\
         service P at \"/p\" {{\n  publishes {publishes}\n  \
         post \"/x\" go(id: uuid) -> M {{\n    fact m: M = id\n    transition m.s: a -> b\n  }}\n}}\n"
    )
}

#[test]
fn a_flow_that_gains_emits_is_compatible_and_one_that_loses_it_is_possibly_breaking() {
    let plain = machine("  flow a -> b\n  flow a -> c", "C");
    let emits = machine("  flow a -> b emits Paid\n  flow a -> c", "C");
    assert_eq!(run(&plain, &emits), [(EmitsAdded, N)]);
    assert!(!has_breaking(&diff(&ir(&plain), &ir(&emits))));
    assert_eq!(run(&emits, &plain), [(EmitsRemoved, P)]);
}

#[test]
fn a_flow_that_emits_another_event_loses_one_and_gains_one() {
    let paid = machine("  flow a -> b emits Paid\n  flow a -> c", "C");
    let gone = machine("  flow a -> b emits Gone\n  flow a -> c", "C");
    assert_eq!(run(&paid, &gone), [(EmitsAdded, N), (EmitsRemoved, P)]);
}

#[test]
fn emits_never_reads_as_a_flow_that_was_removed_and_added() {
    let plain = machine("  flow a -> b\n  flow a -> c", "C");
    let emits = machine("  flow a -> b emits Paid\n  flow a -> c", "C");
    for (a, b) in [(&plain, &emits), (&emits, &plain)] {
        let kinds: Vec<ChangeKind> = run(a, b).into_iter().map(|(k, _)| k).collect();
        assert!(
            !kinds.contains(&FlowRemoved) && !kinds.contains(&FlowAdded),
            "{kinds:?}"
        );
    }
}

#[test]
fn an_edge_that_is_added_or_removed_is_a_flow_and_not_an_emits() {
    let one = machine("  flow a -> b emits Paid", "C");
    let two = machine("  flow a -> b emits Paid\n  flow a -> c emits Gone", "C");
    assert_eq!(run(&one, &two), [(FlowAdded, N)]);
    assert_eq!(run(&two, &one), [(FlowRemoved, B)]);
}

#[test]
fn a_move_that_goes_to_another_topic_is_breaking() {
    let flows = "  flow a -> b emits Paid\n  flow a -> c";
    let first_c = machine(flows, "C, D");
    let first_d = machine(flows, "D, C");
    assert_eq!(run(&first_c, &first_d), [(EmitsTopicChanged, B)]);
    let changes = diff(&ir(&first_c), &ir(&first_d));
    assert!(
        changes[0].message.contains("two.v1"),
        "{}",
        changes[0].message
    );
    assert!(
        changes[0].message.contains("one.v1"),
        "{}",
        changes[0].message
    );
}

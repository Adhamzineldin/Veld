//! `veld diff` and flags (`spec/platform.md` section 11): every row of the table in `flags.rs`.

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

fn messages(old: &str, new: &str) -> Vec<String> {
    diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| c.message)
        .collect()
}

/// A model that a request carries, with one invariant that may be flagged.
fn guest(suffix: &str) -> String {
    format!(
        "model Guest {{\n  guests: int\n  rule small: guests <= 4{suffix}\n}}\n\
         service S at \"/s\" {{ post \"/\" book(body: Guest) -> 204 }}"
    )
}

fn plain_guest() -> String {
    "model Guest {\n  guests: int\n}\nservice S at \"/s\" { post \"/\" book(body: Guest) -> 204 }"
        .to_string()
}

#[test]
fn a_new_flagged_invariant_on_a_request_is_possibly_breaking_with_the_text_of_the_spec() {
    let got = kinds(&plain_guest(), &guest(" when flag(\"small\")"));
    assert_eq!(got, [(FlagChanged, P)]);
    let text = messages(&plain_guest(), &guest(" when flag(\"small\")"));
    assert!(
        text[0].contains("breaking once the flag `small` is on"),
        "{text:?}"
    );
}

#[test]
fn adding_a_flag_to_a_restricting_rule_is_compatible_and_removing_it_is_adding_the_rule() {
    let flagged = guest(" when flag(\"small\")");
    let plain = guest("");
    assert_eq!(kinds(&plain, &flagged), [(FlagChanged, N)]);
    assert_eq!(kinds(&flagged, &plain), [(FlagChanged, B)]);
}

#[test]
fn another_flag_is_possibly_breaking() {
    let (a, b) = (guest(" when flag(\"a\")"), guest(" when flag(\"b\")"));
    assert_eq!(kinds(&a, &b), [(FlagChanged, P)]);
}

#[test]
fn a_flagged_rule_that_is_removed_is_compatible() {
    assert_eq!(
        kinds(&guest(" when flag(\"small\")"), &plain_guest()),
        [(FlagChanged, N)]
    );
}

#[test]
fn a_flagged_invariant_on_a_response_is_a_guarantee() {
    let model = |rule: &str| {
        format!("model Out {{ n: int{rule} }}\nservice S at \"/s\" {{ get \"/\" one() -> Out }}")
    };
    let flagged = model("\n rule pos: n > 0 when flag(\"f\")");
    assert_eq!(kinds(&model(""), &flagged), [(FlagChanged, N)]);
}

const STATES: &str = "enum St {\n  a, b\n  flow a -> b";

fn machine(flow_suffix: &str) -> String {
    format!(
        "{STATES}{flow_suffix}\n  final b\n}}\nmodel M {{ id: uuid, s: St }}\n\
         service S at \"/s\" {{ post \"/{{id}}\" go(id: uuid) -> M {{ fact m: M = id\n transition m.s: a -> b }} }}"
    )
}

#[test]
fn a_flag_on_a_flow_may_take_the_edge_away() {
    let (plain, flagged) = (machine(""), machine(" when flag(\"f\")"));
    assert_eq!(kinds(&plain, &flagged), [(FlagChanged, B)]);
    assert_eq!(kinds(&flagged, &plain), [(FlagChanged, N)]);
    // It is not reported as a removed and an added flow.
    assert!(
        !kinds(&plain, &flagged)
            .iter()
            .any(|(k, _)| matches!(k, FlowRemoved | FlowAdded))
    );
}

fn endpoint(require: &str) -> String {
    format!(
        "model Room {{ id: uuid, open: bool }}\n\
         service S at \"/s\" {{ post \"/{{id}}\" book(id: uuid) -> 204 {{ fact room: Room = id\n{require} }} }}"
    )
}

#[test]
fn a_flagged_require_follows_the_same_table() {
    let plain = endpoint("");
    let flagged = endpoint("require room.open when flag(\"f\")");
    let always = endpoint("require room.open");
    assert_eq!(kinds(&plain, &flagged), [(FlagChanged, P)]);
    assert_eq!(kinds(&always, &flagged), [(FlagChanged, N)]);
    assert_eq!(kinds(&flagged, &always), [(FlagChanged, B)]);
}

#[test]
fn a_contract_without_flags_is_judged_as_before() {
    assert!(kinds(&plain_guest(), &plain_guest()).is_empty());
}

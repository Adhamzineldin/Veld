//! The rules digest (`spec/rules.md` section 9.7).

use crate::signature::hex16;
use crate::testing::ir;

fn digest_of(src: &str) -> Option<String> {
    ir(src).services[0].rules_digest.clone()
}

#[test]
fn a_service_that_reaches_no_invariant_has_no_digest() {
    assert_eq!(
        digest_of("model M { a: int }\nservice S at \"/\" { get \"/\" f() -> 200 M }"),
        None
    );
    let ir = ir(
        "model N { v: int }\nservice S at \"/\" { post \"/\" f(body: N) -> 201 N { require body.v > 0 } }",
    );
    assert_eq!(
        ir.services[0].rules_digest, None,
        "endpoint rules are not invariants"
    );
}

#[test]
fn the_digest_is_the_hash_of_the_sorted_distinct_rule_ids() {
    let ir = ir(
        "model B { a: int\n rule r: a > 0 }\nmodel A { b: B\n rule s: b.a < 9 }\nservice S at \"/\" { get \"/\" f() -> 200 A }",
    );
    let mut ids: Vec<String> = ir
        .decls
        .iter()
        .flat_map(|d| d.rules().iter().map(|r| r.id.clone()))
        .collect();
    ids.sort();
    let json = format!(
        "[{}]",
        ids.iter()
            .map(|i| format!("\"{i}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        ir.services[0].rules_digest.as_deref(),
        Some(hex16(json.as_bytes()).as_str())
    );
}

#[test]
fn an_inherited_rule_counts_once() {
    let ir = ir(
        "model P { a: int\n rule r: a > 0 }\nmodel C extends P { b: int\n rule s: b > 0 }\nservice S at \"/\" { get \"/\" f() -> 200 C }",
    );
    let mut ids: Vec<String> = ir
        .decls
        .iter()
        .flat_map(|d| d.rules().iter().map(|r| r.id.clone()))
        .collect();
    ids.sort();
    ids.dedup();
    let json = format!(
        "[{}]",
        ids.iter()
            .map(|i| format!("\"{i}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        ir.services[0].rules_digest.as_deref(),
        Some(hex16(json.as_bytes()).as_str())
    );
}

#[test]
fn only_what_the_service_reaches_counts() {
    let base = "model Used { a: int\n rule r: a > 0 }\nmodel Other { a: int\n rule q: a > 0 }\n";
    let a = digest_of(&format!(
        "{base}service S at \"/\" {{ get \"/\" f() -> 200 Used }}"
    ));
    let b = digest_of(&format!(
        "{base}service S at \"/\" {{ get \"/\" f() -> 200 Other }}"
    ));
    assert!(a.is_some() && b.is_some());
    assert_ne!(a, b);
}

#[test]
fn a_changed_invariant_changes_the_digest_and_a_changed_message_does_not() {
    let svc = "service S at \"/\" { get \"/\" f() -> 200 M }";
    let a = digest_of(&format!("model M {{ a: int\n rule r: a > 0 }}\n{svc}"));
    let tight = digest_of(&format!("model M {{ a: int\n rule r: a > 1 }}\n{svc}"));
    let said = digest_of(&format!(
        "model M {{ a: int\n rule r: a > 0 message \"m\" }}\n{svc}"
    ));
    assert_ne!(a, tight);
    assert_eq!(a, said);
}

#[test]
fn an_error_with_an_invariant_counts() {
    let d = digest_of(
        "error E 409 { n: int\n rule r: n > 0 }\nmodel M { a: int }\nservice S at \"/\" { get \"/\" f() -> 200 M | E }",
    );
    assert!(d.is_some());
}

#[test]
fn the_digest_is_sixteen_lowercase_hex_characters() {
    let d = digest_of(
        "model M { a: int\n rule r: a > 0 }\nservice S at \"/\" { get \"/\" f() -> 200 M }",
    )
    .unwrap_or_default();
    assert_eq!(d.len(), 16);
    assert!(d.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
}

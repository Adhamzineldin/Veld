//! Parser tests for policies, the principal and `allow` (`spec/rules.md` section 11).

use super::*;
use crate::syntax::ast::DeclKind;

fn p(src: &str) -> Parsed {
    parse(src, FileId(0))
}

fn codes(src: &str) -> Vec<&'static str> {
    p(src).diagnostics.iter().map(|d| d.code).collect()
}

fn ok(src: &str) -> File {
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{src}: {:?}", r.diagnostics);
    r.file
}

const POLICY: &str = r#"
/// Who may manage a booking.
policy ManageBooking(user: User, booking: Booking, ids: uuid[]) {
  permit when booking.guestId == user.id
  permit when "support" in user.roles
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
}
"#;

#[test]
fn parses_a_policy_with_clauses_and_messages() {
    let f = ok(POLICY);
    assert_eq!(f.policies.len(), 1);
    let pol = &f.policies[0];
    assert_eq!(pol.name.name, "ManageBooking");
    assert_eq!(pol.docs.len(), 1);
    let params: Vec<&str> = pol.params.iter().map(|x| x.name.name.as_str()).collect();
    assert_eq!(params, ["user", "booking", "ids"]);
    let kinds: Vec<bool> = pol.clauses.iter().map(|c| c.forbid).collect();
    assert_eq!(kinds, [false, false, true]);
    assert!(pol.clauses[0].message.is_none());
    assert_eq!(
        pol.clauses[2].message.as_ref().map(|m| m.value.as_str()),
        Some("A suspended account cannot manage bookings")
    );
    assert!(f.decls.is_empty());
}

#[test]
fn a_trailing_comma_in_the_parameters_is_fine() {
    assert_eq!(ok("policy P(a: A,) { permit when a.x }").policies.len(), 1);
}

#[test]
fn parses_a_principal_with_and_without_a_name() {
    let f = ok("principal User\nprincipal Other as caller");
    assert_eq!(f.principals.len(), 2);
    assert!(f.principals[0].name.is_none());
    assert_eq!(
        f.principals[1].name.as_ref().map(|n| n.name.as_str()),
        Some("caller")
    );
}

#[test]
fn the_name_of_a_principal_must_be_on_the_same_line() {
    // `as` on the next line is not part of the declaration.
    let r = p("principal User\nas");
    assert!(!r.diagnostics.is_empty());
    assert_eq!(r.file.principals.len(), 1);
    assert!(r.file.principals[0].name.is_none());
}

#[test]
fn an_unclosed_or_broken_policy_is_reported_and_parsing_goes_on() {
    for src in [
        "policy P() { permit when a }\nmodel M { x: int }",
        "policy P(a: A) { permit a }\nmodel M { x: int }",
        "policy P(a: A) { allow when a }\nmodel M { x: int }",
        "policy P(a A) { permit when a }\nmodel M { x: int }",
    ] {
        let r = p(src);
        assert!(r.diagnostics.iter().any(|d| d.code == "E0003"), "{src}");
        assert!(
            r.file
                .decls
                .iter()
                .any(|d| matches!(d.kind, DeclKind::Model(_))),
            "the model after it is still parsed: {src}"
        );
    }
    assert!(!codes("policy P(a: A) { permit when a").is_empty());
}

#[test]
fn a_broken_clause_does_not_swallow_the_next_one() {
    let r = p("policy P(a: A) {\n  permit nope\n  permit when a.ok\n}");
    assert_eq!(
        r.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["E0003"]
    );
    assert_eq!(r.file.policies[0].clauses.len(), 1);
}

#[test]
fn a_policy_and_a_principal_take_no_annotations() {
    assert_eq!(
        codes("@deprecated\npolicy P(a: A) { permit when a.x }"),
        ["E0003"]
    );
    assert_eq!(codes("@deprecated\nprincipal User"), ["E0003"]);
}

#[test]
fn the_policy_words_are_names_everywhere_else() {
    let f = ok(
        "model policy { permit: int\n forbid: int\n when: int }\nmodel principal { value: int }\nenum E { policy, permit, allow }\nconst allow: int = 1",
    );
    assert_eq!(f.decls.len(), 4);
    assert!(f.policies.is_empty() && f.principals.is_empty());
    // `policy` or `principal` followed by something that is not a declaration stays a name.
    let f = ok("policy P(a: A) { permit when a.x }\nmodel M { policy: int\n principal: int }");
    assert_eq!(f.policies.len(), 1);
}

#[test]
fn a_policy_and_a_principal_after_a_use_are_fine_but_use_after_them_is_not() {
    let f = ok("use a.{A}\npolicy P(a: A) { permit when a.x }\nprincipal A");
    assert_eq!(
        (f.uses.len(), f.policies.len(), f.principals.len()),
        (1, 1, 1)
    );
    assert_eq!(
        codes("principal A\nuse a.{A}"),
        ["E0003"],
        "`use` must come before every other declaration"
    );
}

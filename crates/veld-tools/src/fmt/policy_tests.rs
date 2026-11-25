//! Formatter behaviour for policies, the principal and `allow` (`spec/rules.md` section 11).

use super::format;

fn f(src: &str) -> String {
    format(src)
        .unwrap_or_else(|d| format!("ERR {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>()))
}

/// Formats once, checks the result is a fixed point, and returns it.
fn stable(src: &str) -> String {
    let once = f(src);
    assert_eq!(
        f(&once),
        once,
        "not idempotent for:\n{src}\n--- first pass:\n{once}"
    );
    once
}

#[test]
fn a_principal_prints_with_and_without_a_name() {
    assert_eq!(stable("principal   User"), "principal User\n");
    assert_eq!(
        stable("principal User   as   caller"),
        "principal User as caller\n"
    );
}

#[test]
fn a_policy_prints_one_clause_per_line_with_the_message_below() {
    let out = stable(
        "policy   ManageBooking( user:User,booking : Booking ,){\n  permit   when booking.guestId==user.id\n permit when \"support\" in user.roles\n  forbid when user.suspended   message \"A suspended account cannot manage bookings\"\n}",
    );
    assert_eq!(
        out,
        r#"policy ManageBooking(user: User, booking: Booking) {
  permit when booking.guestId == user.id
  permit when "support" in user.roles
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
}
"#
    );
}

#[test]
fn an_empty_policy_stays_empty_and_docs_stay_on_the_declaration() {
    assert_eq!(stable("policy P(a: A) {}"), "policy P(a: A) {}\n");
    let src = "/// Who may.\npolicy P(a: A) {\n  permit when a.ok\n}\n";
    assert_eq!(stable(src), src);
    let docs = "/// The caller.\nprincipal User\n";
    assert_eq!(stable(docs), docs);
}

#[test]
fn allow_goes_after_the_facts_and_before_the_rest() {
    let src = "service S at \"/\" {\n  post \"/{id}\" pay(id: uuid) -> B {\n    ensure   true\n    require a   else E\n    allow   P( user,booking , id )\n    transition booking.status: a -> b\n    fact booking: B = id\n    allow Q()\n  }\n}\n";
    assert_eq!(
        stable(src),
        "service S at \"/\" {\n  post \"/{id}\" pay(id: uuid) -> B {\n    fact booking: B = id\n    allow P(user, booking, id)\n    allow Q()\n    transition booking.status: a -> b\n    require a else E\n    ensure true\n  }\n}\n"
    );
}

#[test]
fn comments_stay_with_policies_principals_and_allows() {
    let src = r#"// file header

// the caller
principal User // trailing principal

// about the policy
policy P(u: U) { // header
  // about permit
  permit when u.ok // trailing permit
  forbid when u.no
    message "no" // trailing message
  // tail
} // after

service S at "/" {
  get "/" f() -> U {
    // about allow
    allow P(user) // trailing allow
    require true
  }
}
"#;
    let out = stable(src);
    for c in [
        "// file header",
        "// the caller",
        "// trailing principal",
        "// about the policy",
        "// header",
        "// about permit",
        "// trailing permit",
        "// trailing message",
        "// tail",
        "// after",
        "// about allow",
        "// trailing allow",
    ] {
        assert!(out.contains(c), "lost {c}:\n{out}");
    }
}

#[test]
fn policies_principals_and_declarations_keep_their_source_order() {
    let src = "model A { a: int }\npolicy P(a: A) { permit when a.a > 1 }\nprincipal A\nmodel B { b: int }\n";
    assert_eq!(
        stable(src),
        "model A {\n  a: int\n}\n\npolicy P(a: A) {\n  permit when a.a > 1\n}\n\nprincipal A\n\nmodel B {\n  b: int\n}\n"
    );
}

#[test]
fn the_policy_words_stay_names_through_the_formatter() {
    let src = "model policy {\n  permit: int\n  forbid: int\n  when: int\n}\n\nservice allow at \"/\" {\n  get \"/\" allow(@query principal: int) -> policy\n}\n";
    assert_eq!(stable(src), src);
}

#[test]
fn a_policy_can_be_named_like_its_words() {
    let src = "policy permit(forbid: when) {\n  permit when forbid.allow\n}\n\nprincipal principal as allow\n";
    assert_eq!(stable(src), src);
}

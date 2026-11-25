//! Formatter behaviour for rules and expressions (`spec/rules.md` sections 3 and 4).

use super::format;

fn f(src: &str) -> String {
    format(src)
        .unwrap_or_else(|d| format!("ERR {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>()))
}

/// Formats once, checks the result is a fixed point, and returns it.
fn stable(src: &str) -> String {
    let once = f(src);
    assert_eq!(f(&once), once, "not idempotent for:\n{src}");
    once
}

/// The printed expression of `rule r: <expr>` in a model with the fields of `FIELDS`.
fn expr(src: &str) -> String {
    let out = stable(&format!("model M {{\n rule r: {src}\n}}"));
    out.strip_prefix("model M {\n  rule r: ")
        .and_then(|s| s.strip_suffix("\n}\n"))
        .unwrap_or(&out)
        .to_string()
}

#[test]
fn fields_come_first_then_one_blank_line_then_rules_in_source_order() {
    let out = stable(
        "model Booking {\n  rule b: n > 1\n  n: int\n  rule a: n < 5\n  m: int\n  rule c: m > 0\n}",
    );
    assert_eq!(
        out,
        "model Booking {\n  n: int\n  m: int\n\n  rule b: n > 1\n  rule a: n < 5\n  rule c: m > 0\n}\n"
    );
}

#[test]
fn a_body_of_only_rules_has_no_blank_line_and_an_empty_body_stays_empty() {
    assert_eq!(
        stable("model M { rule r: true }"),
        "model M {\n  rule r: true\n}\n"
    );
    assert_eq!(stable("model M {}"), "model M {}\n");
    assert_eq!(
        stable("error E 409 { rule r: true\n code: int }"),
        "error E 409 {\n  code: int\n\n  rule r: true\n}\n"
    );
}

#[test]
fn the_message_goes_on_a_continuation_line() {
    let out = stable("model M { n: int\n rule r: n > 0 message \"positive\" }");
    assert_eq!(
        out,
        "model M {\n  n: int\n\n  rule r: n > 0\n    message \"positive\"\n}\n"
    );
}

#[test]
fn docs_annotations_and_comments_stay_with_their_rule() {
    let src = "model M {\n  n: int\n  // about a\n  /// Shown to users.\n  @deprecated(\"use b\")\n  rule a: n > 0 // trailing a\n  rule b: n > 1 /* inline */\n  // tail\n}";
    let out = stable(src);
    assert_eq!(
        out,
        "model M {\n  n: int\n\n  // about a\n  /// Shown to users.\n  @deprecated(\"use b\")\n  rule a: n > 0 // trailing a\n  rule b: n > 1 /* inline */\n  // tail\n}\n"
    );
}

#[test]
fn comments_move_with_a_rule_that_is_regrouped() {
    let out = stable("model M {\n  rule a: n > 0\n  // about n\n  n: int // the n\n}");
    assert_eq!(
        out,
        "model M {\n  // about n\n  n: int // the n\n\n  rule a: n > 0\n}\n"
    );
}

#[test]
fn rule_is_still_a_field_name_when_a_colon_follows() {
    let out = stable("model M { rule: string\n rule?: int\n rule r: len(rule) > 0 }");
    assert_eq!(
        out,
        "model M {\n  rule: string\n  rule?: int\n\n  rule r: len(rule) > 0\n}\n"
    );
}

#[test]
fn parentheses_are_kept_only_where_they_matter() {
    assert_eq!(expr("(a && b) && c"), "a && b && c");
    assert_eq!(expr("a && (b && c)"), "a && (b && c)");
    assert_eq!(expr("(a || b) && c"), "(a || b) && c");
    assert_eq!(expr("a || (b && c)"), "a || b && c");
    assert_eq!(expr("(a implies b) implies c"), "(a implies b) implies c");
    assert_eq!(expr("a implies (b implies c)"), "a implies b implies c");
    assert_eq!(expr("(a < b) == c"), "(a < b) == c");
    assert_eq!(expr("(a + b) * c"), "(a + b) * c");
    assert_eq!(expr("a + (b * c)"), "a + b * c");
    assert_eq!(expr("a - (b - c)"), "a - (b - c)");
    assert_eq!(expr("(a - b) - c"), "a - b - c");
    assert_eq!(expr("(((a)))"), "a");
}

#[test]
fn not_and_bang_are_kept_as_written() {
    assert_eq!(expr("not a == b"), "not a == b");
    assert_eq!(expr("!a == b"), "!a == b");
    assert_eq!(expr("not (a == b)"), "not (a == b)");
    assert_eq!(expr("!(a == b)"), "!(a == b)");
    assert_eq!(expr("not not a"), "not not a");
    assert_eq!(expr("!!a && not b"), "!!a && not b");
}

#[test]
fn the_minus_rule_survives_formatting() {
    assert_eq!(expr("a > -1"), "a > -1");
    assert_eq!(expr("a > - 1"), "a > -1");
    assert_eq!(expr("a > -(1)"), "a > -(1)");
    assert_eq!(expr("a > -(-1)"), "a > -(-1)");
    assert_eq!(expr("a > -(1.50)"), "a > -(1.50)");
    assert_eq!(expr("a > - -(1)"), "a > - -(1)");
    assert_eq!(expr("a -1 > 0"), "a - 1 > 0");
    assert_eq!(expr("a - -1 > 0"), "a - -1 > 0");
    assert_eq!(expr("a - - 1 > 0"), "a - -1 > 0");
    assert_eq!(expr("-a > 0"), "-a > 0");
    assert_eq!(expr("- -a > 0"), "- -a > 0");
    assert_eq!(expr("-(a + b) > 0"), "-(a + b) > 0");
    assert_eq!(expr("n % 2 == -1"), "n % 2 == -1");
    assert_eq!(expr("x in [-1, - 2]"), "x in [-1, -2]");
}

#[test]
fn number_literals_keep_their_source_text() {
    assert_eq!(expr("price == 1.50"), "price == 1.50");
    assert_eq!(expr("ratio < 1.0e308"), "ratio < 1.0e308");
    assert_eq!(expr("price > -0.250"), "price > -0.250");
}

#[test]
fn every_primary_form_prints_back() {
    assert_eq!(expr("x == .paid"), "x == .paid");
    assert_eq!(expr("s == \"a\\\"b\\n\""), "s == \"a\\\"b\\n\"");
    assert_eq!(expr("a.b.c > 0"), "a.b.c > 0");
    assert_eq!(expr("(a + b).c > 0"), "(a + b).c > 0");
    assert_eq!(expr("x in [.a, .b,]"), "x in [.a, .b]");
    assert_eq!(
        expr("all(xs, i => i.q > 0 && has(i.p))"),
        "all(xs, i => i.q > 0 && has(i.p))"
    );
    assert_eq!(expr("matches(s,\"^a$\")"), "matches(s, \"^a$\")");
    assert_eq!(expr("a in b implies c"), "a in b implies c");
}

#[test]
fn contextual_words_keep_their_meaning() {
    assert_eq!(expr("not implies implies"), "not implies implies");
    assert_eq!(expr("in > has"), "in > has");
}

#[test]
fn a_chained_comparison_or_syntax_error_is_refused() {
    assert!(f("model M { rule r: a < b < c }").starts_with("ERR"));
    assert!(f("model M { rule r: a > }").starts_with("ERR"));
}

#[test]
fn a_negative_literal_after_a_word_operator_needs_no_parentheses() {
    // The parser joins the minus to the literal, so the parentheses are not needed (and are
    // dropped when only the literal was inside them).
    assert_eq!(expr("x implies (-1 < a)"), "x implies -1 < a");
    assert_eq!(expr("x implies -1 < a"), "x implies -1 < a");
    assert_eq!(
        expr("x implies -9223372036854775808 < l"),
        "x implies -9223372036854775808 < l"
    );
    assert_eq!(expr("a in [-2147483648, 0]"), "a in [-2147483648, 0]");
    assert_eq!(expr("x implies -(1) < a"), "x implies -(1) < a");
    assert_eq!(expr("x implies (-0.5 < d)"), "x implies -0.5 < d");
    assert_eq!(expr("x implies (- 1 < a)"), "x implies -1 < a");
    assert_eq!(expr("x implies (a < -1)"), "x implies a < -1");
    assert_eq!(expr("a in [-1]"), "a in [-1]");
}

//! Formatter behaviour for `@rollout(audit)` (`spec/rules.md` section 17.1): the annotation goes
//! on its own line before `rule`, `require` and `ensure`, and formatting is idempotent.

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
fn an_annotation_on_a_rule_goes_on_its_own_line() {
    let out = stable("model M { n: int\n @rollout( audit )   rule r: n>0 message \"m\" }");
    assert_eq!(
        out,
        "model M {\n  n: int\n\n  @rollout(audit)\n  rule r: n > 0\n    message \"m\"\n}\n"
    );
}

#[test]
fn docs_come_first_then_the_annotations_then_the_rule() {
    let out = stable(
        "model M { n: int\n /// Positive.\n @rollout(audit) @deprecated(\"x\")\n rule r: n > 0 }",
    );
    assert!(
        out.contains("  @rollout(audit)\n  @deprecated(\"x\")\n  rule r: n > 0"),
        "{out}"
    );
}

#[test]
fn the_annotation_of_an_error_and_an_event_rule_is_kept() {
    let out = stable(
        "error E 409 { n: int\n @rollout(audit) rule r: n > 0 }\nevent V { n: int\n @rollout(audit) rule q: n > 0 }",
    );
    assert_eq!(out.matches("@rollout(audit)\n  rule").count(), 2, "{out}");
}

const BLOCK: &str = r#"
service S at "/s" {
  post "/" f(body: B) -> 201 B | E {
    ensure   result.n>0
    @rollout( audit )   require   body.n<5 else E
    @rollout(audit)
    require named :body.n>0   message "m"
    fact x:X
    /// Doc.
    @rollout(audit)
    ensure last: result.n<9
  }
}
"#;

#[test]
fn require_and_ensure_print_the_annotation_on_its_own_line_in_their_group() {
    let out = stable(BLOCK);
    assert_eq!(
        out,
        "service S at \"/s\" {\n  post \"/\" f(body: B) -> 201 B | E {\n    fact x: X\n    @rollout(audit)\n    require body.n < 5 else E\n    @rollout(audit)\n    require named: body.n > 0\n      message \"m\"\n    ensure result.n > 0\n    /// Doc.\n    @rollout(audit)\n    ensure last: result.n < 9\n  }\n}\n"
    );
}

#[test]
fn a_comment_before_an_annotated_statement_stays_before_it() {
    let out = stable(
        "service S at \"/s\" {\n  post \"/\" f() {\n    // watch it\n    @rollout(audit)\n    require true // trailing\n  }\n}\n",
    );
    assert!(
        out.contains("    // watch it\n    @rollout(audit)\n    require true // trailing\n"),
        "{out}"
    );
}

#[test]
fn a_statement_without_the_annotation_prints_as_before() {
    assert_eq!(
        stable("service S at \"/s\" { get \"/\" f() { require true\n ensure true } }"),
        "service S at \"/s\" {\n  get \"/\" f() {\n    require true\n    ensure true\n  }\n}\n"
    );
}

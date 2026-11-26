//! Formatter behaviour for `flow ... after` and `transition ... since` (`spec/platform.md`
//! section 10): they survive a round trip, formatting is idempotent, comments stay.

use super::format;

fn f(src: &str) -> String {
    format(src)
        .unwrap_or_else(|d| format!("ERR {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>()))
}

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
fn a_timed_flow_keeps_its_duration() {
    let out = stable(
        "enum S {\n a, b, c\n flow a -> b\n flow   a -> c   after   hours( 24 )\n final b, c\n}",
    );
    assert!(out.contains("  flow a -> b\n"), "{out}");
    assert!(out.contains("  flow a -> c after hours(24)\n"), "{out}");
}

#[test]
fn an_arithmetic_duration_is_printed_as_an_expression() {
    let out = stable("enum S { a, b\n flow a -> b after days(1)+hours(2)\n}");
    assert!(
        out.contains("flow a -> b after days(1) + hours(2)"),
        "{out}"
    );
}

#[test]
fn a_comment_after_a_timed_flow_stays_on_its_line() {
    let out = stable("enum S { a, b\n flow a -> b after hours(1) // an unpaid booking expires\n}");
    assert!(
        out.contains("flow a -> b after hours(1) // an unpaid booking expires"),
        "{out}"
    );
}

#[test]
fn a_transition_keeps_since() {
    let src = "service S at \"/s\" {\n post \"/x\" go(id: uuid) -> D {\n  fact d: D = id\n  transition  d.state:a  ->  b   since   d.stamps.entered // when\n }\n}";
    let out = stable(src);
    assert!(
        out.contains("transition d.state: a -> b since d.stamps.entered // when"),
        "{out}"
    );
}

#[test]
fn after_and_since_stay_names() {
    let src =
        "model M { after: int, since: datetime }\nenum E { after, since\n flow after -> since\n}";
    let out = stable(src);
    assert!(out.contains("flow after -> since"), "{out}");
}

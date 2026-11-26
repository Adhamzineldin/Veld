//! Formatter behaviour for `emits` and `expect emitted` (`spec/events.md` sections 11 and 15).

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
fn a_flow_prints_emits_after_its_edge() {
    let out = stable(
        "enum S {\n  a, b, c\n  flow a ->   b    emits   Paid\n  flow b -> c\n  final c\n}\n",
    );
    assert_eq!(
        out,
        "enum S {\n  a,\n  b,\n  c,\n\n  flow a -> b emits Paid\n  flow b -> c\n  final c\n}\n"
    );
}

#[test]
fn comments_stay_with_a_flow_that_emits() {
    let src = "enum S {\n  a,\n  b,\n\n  // the receipt\n  flow a -> b emits Paid // sent\n}\n";
    assert_eq!(stable(src), src);
}

#[test]
fn expect_emitted_lines_print_after_the_call() {
    let src = "example \"pays\" {\n  given booking = { status: .pending }\n  pay(id: x)\n    returns { status: .paid }\n    -> ok\n  expect emitted   BookingPaid\n  expect emitted Receipt\n}\n";
    let out = stable(src);
    assert_eq!(
        out,
        "example \"pays\" {\n  given booking = { status: .pending }\n  pay(id: x)\n    returns { status: .paid }\n    -> ok\n  expect emitted BookingPaid\n  expect emitted Receipt\n}\n"
    );
}

#[test]
fn expect_not_emitted_and_its_comment_survive() {
    let src = "example \"x\" {\n  pay() -> InvalidTransition\n  // nothing is sent\n  expect not emitted\n}\n";
    assert_eq!(stable(src), src);
}

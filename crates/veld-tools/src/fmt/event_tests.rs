//! Formatter behaviour for events, channels, `publishes`, `subscribes` and event examples
//! (`spec/events.md` section 15).

use super::format;
use crate::testutil::ir;

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

/// The IR of `src` and of its formatted text are the same, apart from where things are written.
fn assert_same_ir(src: &str) {
    let strip = |v: &mut serde_json::Value| strip_origins(v);
    let mut a = serde_json::to_value(ir(src)).unwrap();
    let mut b = serde_json::to_value(ir(&stable(src))).unwrap();
    strip(&mut a);
    strip(&mut b);
    assert_eq!(a, b, "the formatted text means something else:\n{src}");
}

fn strip_origins(v: &mut serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::Object(m) => {
            m.remove("origin");
            m.values_mut().for_each(strip_origins);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_origins),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

const BOOKINGS: &str =
    include_str!("../../../../tests/conformance/ok_events_bookings/input/app.veld");

#[test]
fn the_bookings_contract_is_a_fixed_point_with_the_same_ir() {
    let out = stable(BOOKINGS);
    assert!(
        out.contains("service Notifications {\n  subscribes BookingEvents as \"notifications\" {\n    on BookingPaid      -> sendReceipt\n    on BookingCancelled -> sendApology\n  }\n}\n"),
        "{out}"
    );
    assert!(
        out.contains("  publishes BookingEvents, Audit\n  get \"/ping\" ping() -> 204\n"),
        "{out}"
    );
    assert_same_ir(BOOKINGS);
}

#[test]
fn an_event_prints_like_a_model() {
    let out = stable(
        "event   Paid {\n a:int\n rule positive: a>0\n  message \"m\"\n b?:string=null\n}\nchannel C { carries Paid }\nservice S { publishes C }",
    );
    assert!(
        out.starts_with(
            "event Paid {\n  a: int\n  b?: string = null\n\n  rule positive: a > 0\n    message \"m\"\n}\n"
        ),
        "{out}"
    );
    assert_eq!(
        stable("event E {}\nchannel C { carries E }"),
        "event E {}\n\nchannel C {\n  carries E\n}\n"
    );
}

#[test]
fn a_channel_prints_carries_then_key_one_per_line() {
    let src = "event A { id: uuid }\nevent B { id: uuid }\n/// The stream.\n@topic( \"a.b\" )\nchannel   C {\n key id\n carries A,B\n}\n";
    let out = stable(src);
    assert_eq!(
        out,
        "event A {\n  id: uuid\n}\n\nevent B {\n  id: uuid\n}\n\n/// The stream.\n@topic(\"a.b\")\nchannel C {\n  carries A, B\n  key id\n}\n"
    );
    assert_same_ir(&format!("{src}service S {{ publishes C }}"));
}

#[test]
fn a_service_without_at_prints_without_it() {
    let src = "event A { id: uuid }\nchannel C { carries A }\nservice   S{\npublishes   C\n}\n";
    let out = stable(src);
    assert!(out.ends_with("service S {\n  publishes C\n}\n"), "{out}");
    assert_same_ir(src);
    assert_eq!(stable("service S {}"), "service S {}\n");
}

#[test]
fn publishes_come_first_and_subscribes_last_with_aligned_arrows() {
    let src = "model Pong { x: int }\nevent A { id: uuid }\nevent LongName { id: uuid }\nchannel C { carries A, LongName }\nchannel D { carries A }\nservice S at \"/s\" {\n subscribes D as \"g2\" { on A -> d }\n get \"/\" ping() -> Pong\n publishes D\n subscribes C as \"g\" {\n /// Handles A.\n on A->a\n on LongName   ->   b\n }\n publishes C\n}\n";
    let out = stable(src);
    let want = "service S at \"/s\" {\n  publishes D\n  publishes C\n  get \"/\" ping() -> Pong\n\n  subscribes D as \"g2\" {\n    on A -> d\n  }\n\n  subscribes C as \"g\" {\n    /// Handles A.\n    on A        -> a\n    on LongName -> b\n  }\n}\n";
    assert!(out.ends_with(want), "{out}");
    assert_same_ir(src);
}

#[test]
fn comments_in_a_service_with_events_survive() {
    let src = "event A { id: uuid }\nchannel C { carries A }\nservice S {\n  // who we tell\n  publishes C // trailing\n\n  // who tells us\n  subscribes C as \"g\" { // header\n    // first\n    on A -> a // done\n    // last\n  }\n}\n";
    let out = stable(src);
    for c in [
        "// who we tell",
        "// trailing",
        "// who tells us",
        "// header",
        "// first",
        "// done",
        "// last",
    ] {
        assert!(out.contains(c), "lost `{c}`:\n{out}");
    }
    assert_same_ir(src);
}

#[test]
fn an_event_example_prints_the_given_then_one_expect_per_line() {
    let src = "event A { n: int }
event B { n: int }
channel C { carries A, B }
service S { subscribes C as \"g\" { on A -> h
 on B -> k }
 publishes C }
example   \"x\" {
 given event A{n:1} on C
 expect S.k   not   called
 expect   S.h called
}
example \"y\" { given event A { n: 0 }
 expect rejected }
";
    let out = stable(src);
    assert!(
        out.contains("example \"x\" {\n  given event A { n: 1 } on C\n  expect S.k not called\n  expect S.h called\n}\n"),
        "{out}"
    );
    assert!(
        out.contains("example \"y\" {\n  given event A { n: 0 }\n  expect rejected\n}\n"),
        "{out}"
    );
    assert_same_ir(src);
}

//! Formatter behaviour for the channel annotations of phase E2 and `called once`
//! (`spec/events.md` section 15).

use super::format;
use crate::testutil::ir;

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

fn assert_same_ir(src: &str) {
    let mut a = serde_json::to_value(ir(src)).unwrap();
    let mut b = serde_json::to_value(ir(&stable(src))).unwrap();
    strip_origins(&mut a);
    strip_origins(&mut b);
    assert_eq!(a, b, "the formatted text means something else:\n{src}");
}

const TAIL: &str = "service P { publishes C }\nservice S { subscribes C as \"g\" { on E -> h } }\n";

fn with_channel(anns: &str) -> String {
    format!("event E {{ id: uuid }}\n{anns}\nchannel C {{ carries E }}\n{TAIL}")
}

#[test]
fn the_annotations_print_before_channel_one_per_line_in_the_documented_order() {
    let out = stable(&with_channel(
        "@ordered @deadLetter @retry(max:3,backoff:fixed) @topic(\"a.b\")",
    ));
    assert!(
        out.contains(
            "@topic(\"a.b\")\n@retry(max: 3, backoff: fixed)\n@deadLetter\n@ordered\nchannel C {\n"
        ),
        "{out}"
    );
    assert_same_ir(&with_channel(
        "@ordered @deadLetter @retry(max:3,backoff:fixed) @topic(\"a.b\")",
    ));
}

#[test]
fn a_channel_that_is_already_in_order_does_not_change() {
    let out = stable(&with_channel(
        "@topic(\"a.b\")\n@retry(max: 0)\n@deadLetter",
    ));
    assert!(
        out.contains("@topic(\"a.b\")\n@retry(max: 0)\n@deadLetter\nchannel C {\n"),
        "{out}"
    );
}

#[test]
fn other_annotations_keep_their_place_between_the_four() {
    let out = stable(&with_channel(
        "@ordered\n@deprecated(\"old\")\n@retry(max: 2)\n@topic(\"t\")",
    ));
    // The four take the slots the four had: slots 0, 2 and 3; `@deprecated` stays second.
    assert!(
        out.contains("@topic(\"t\")\n@deprecated(\"old\")\n@retry(max: 2)\n@ordered\nchannel C"),
        "{out}"
    );
}

#[test]
fn a_channel_with_only_a_topic_or_a_deprecation_prints_as_it_did() {
    let out = stable(&with_channel("@deprecated(\"x\")\n@topic(\"t\")"));
    assert!(
        out.contains("@deprecated(\"x\")\n@topic(\"t\")\nchannel C"),
        "{out}"
    );
}

#[test]
fn the_arguments_of_retry_are_normalised() {
    let out = stable(&with_channel(
        "@retry( backoff :exponential ,max:  7 )\n@deadLetter",
    ));
    assert!(
        out.contains("@retry(backoff: exponential, max: 7)\n"),
        "{out}"
    );
}

#[test]
fn called_once_is_printed_after_called() {
    let src = format!(
        "{}example \"x\" {{ given event E {{ }}\n expect S.h  called  once\n expect S.k not called }}",
        with_channel("")
    );
    let out = stable(&src);
    assert!(out.contains("  expect S.h called once\n"), "{out}");
    assert!(out.contains("  expect S.k not called\n"), "{out}");
}

#[test]
fn called_once_keeps_its_meaning() {
    let src = "event E { id: uuid }\nchannel C { carries E }\nservice P { publishes C }\nservice S { subscribes C as \"g\" { on E -> h } }\nexample \"x\" { given event E { }\n expect S.h called once }";
    assert_same_ir(src);
}

#[test]
fn a_contract_with_the_annotations_is_a_fixed_point_with_the_same_ir() {
    let src = "event E { id: uuid }\n@topic(\"t\") @retry(max: 5) @deadLetter @ordered\nchannel C {\n carries E\n key id\n}\nservice P { publishes C }\nservice S { subscribes C as \"g\" { on E -> h } }\nexample \"x\" { given event E { }\n expect S.h called once }";
    assert_same_ir(src);
}

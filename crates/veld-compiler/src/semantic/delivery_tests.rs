//! `@retry`, `@deadLetter`, `@ordered`, `called once` and the feature `events_delivery`
//! (`spec/events.md` sections 10.2, 10.8 and 10.9).

use crate::testing::{codes, one};
use veld_ir::{Backoff, EventExpect, Feature, Retry};

/// A contract whose channel carries the annotations `anns`, published and subscribed so that no
/// graph warning is reported.
fn contract(anns: &str) -> String {
    format!(
        r#"
event Paid {{ id: uuid }}

{anns}
channel Payments {{
  carries Paid
  key id
}}

service Shop {{ publishes Payments }}

service Receipts {{
  subscribes Payments as "receipts" {{
    on Paid -> send
  }}
}}
"#
    )
}

fn ir_of(anns: &str) -> veld_ir::Ir {
    let out = one(&contract(anns));
    match out.ir {
        Some(ir) => ir,
        None => panic!("compile failed:\n{}", out.render()),
    }
}

#[test]
fn the_three_annotations_reach_the_ir() {
    let ir = ir_of("@retry(max: 5, backoff: fixed)\n@deadLetter\n@ordered");
    let c = &ir.channels[0];
    assert_eq!(
        c.retry,
        Some(Retry {
            max: 5,
            backoff: Backoff::Fixed
        })
    );
    assert!(c.dead_letter && c.ordered);
}

#[test]
fn backoff_defaults_to_exponential_and_zero_retries_are_allowed() {
    let ir = ir_of("@retry(max: 0)");
    assert_eq!(
        ir.channels[0].retry,
        Some(Retry {
            max: 0,
            backoff: Backoff::Exponential
        })
    );
    let ir = ir_of("@retry(backoff: fixed, max: 100)");
    assert_eq!(
        ir.channels[0].retry,
        Some(Retry {
            max: 100,
            backoff: Backoff::Fixed
        })
    );
}

#[test]
fn a_channel_without_them_has_none_and_keeps_its_json() {
    let ir = ir_of("");
    let c = &ir.channels[0];
    assert!(c.retry.is_none() && !c.dead_letter && !c.ordered);
    let v = serde_json::to_value(c).unwrap();
    for gone in ["retry", "dead_letter", "ordered"] {
        assert!(v.get(gone).is_none(), "{gone}");
    }
}

#[test]
fn the_annotations_may_stand_in_any_order_with_topic() {
    let ir = ir_of("@ordered\n@topic(\"shop.pay\")\n@deadLetter\n@retry(max: 2)");
    let c = &ir.channels[0];
    assert_eq!(c.topic, "shop.pay");
    assert!(c.ordered && c.dead_letter && c.retry.is_some());
}

#[test]
fn a_wrong_place_a_repeat_and_bad_arguments_are_e0602() {
    let bad = [
        "@retry(max: 3)\n@retry(max: 4)\n@deadLetter",
        "@deadLetter\n@deadLetter",
        "@ordered\n@ordered",
        "@retry\n@deadLetter",
        "@retry()\n@deadLetter",
        "@retry(backoff: fixed)\n@deadLetter",
        "@retry(max: 101)\n@deadLetter",
        "@retry(max: -1)\n@deadLetter",
        "@retry(max: 2.5)\n@deadLetter",
        "@retry(max: \"3\")\n@deadLetter",
        "@retry(3)\n@deadLetter",
        "@retry(max: 3, backoff: linear)\n@deadLetter",
        "@retry(max: 3, backoff: \"fixed\")\n@deadLetter",
        "@retry(max: 3, max: 4)\n@deadLetter",
        "@retry(max: 3, backoff: fixed, backoff: fixed)\n@deadLetter",
        "@retry(max: 3, delay: 5)\n@deadLetter",
        "@deadLetter(true)",
        "@deadLetter(topic: \"x\")",
        "@ordered(1)",
        "@ordered(key)",
    ];
    for anns in bad {
        let found = codes(&contract(anns));
        assert!(found.contains(&"E0602"), "{anns}: {found:?}");
        assert!(found.iter().all(|c| *c == "E0602"), "{anns}: {found:?}");
    }
}

#[test]
fn the_annotations_are_refused_outside_a_channel() {
    let on = [
        "model M { a: int }",
        "enum E { a, b }",
        "event P { id: uuid }",
        "service S at \"/s\" { get \"/a\" a() -> 204 }",
    ];
    for target in on {
        for ann in ["@retry(max: 1)", "@deadLetter", "@ordered"] {
            let src = format!("{ann}\n{target}");
            assert_eq!(codes(&src), ["E0602"], "{src}");
        }
    }
    assert_eq!(
        codes("event P { id: uuid @ordered }"),
        ["E0602"],
        "on a field"
    );
}

#[test]
fn retry_without_a_dead_letter_is_w1003_at_the_annotation() {
    let src = contract("@retry(max: 3)");
    assert_eq!(codes(&src), ["W1003"]);
    let out = one(&src);
    assert!(out.ir.is_some(), "a warning does not stop the compilation");
    let line = src[..src.find("@retry").unwrap_or(0)].lines().count() + 1;
    let at = format!("app/app.veld:{line}:1");
    assert!(out.render().contains(&at), "{at}\n{}", out.render());
}

#[test]
fn no_w1003_with_a_dead_letter_or_without_retry() {
    assert!(codes(&contract("@retry(max: 3)\n@deadLetter")).is_empty());
    assert!(codes(&contract("@deadLetter")).is_empty());
    assert!(codes(&contract("@ordered")).is_empty());
    assert!(
        codes(&contract(
            "@retry(max: 0, backoff: fixed)\n@deadLetter\n@ordered"
        ))
        .is_empty()
    );
}

#[test]
fn w1003_is_reported_once_per_channel() {
    let src = "event P { id: uuid }\n@retry(max: 1)\nchannel A { carries P }\n@retry(max: 2)\nchannel B { carries P }\nservice S { publishes A, B }\nservice T { subscribes A as \"a\" { on P -> x }\n subscribes B as \"b\" { on P -> y } }";
    assert_eq!(codes(src), ["W1003", "W1003"]);
}

#[test]
fn the_dead_letter_topic_counts_as_a_topic() {
    // `<topic>.dlq` can never equal the topic of another channel, because a topic must not end
    // in `.dlq` (E1003); a second channel with the same topic is E1004 whatever it has.
    let src = "event P { id: uuid }\n@topic(\"t\")\n@deadLetter\nchannel A { carries P }\n@topic(\"t\")\n@deadLetter\nchannel B { carries P }\nservice S { publishes A, B }\nservice T { subscribes A as \"a\" { on P -> x }\n subscribes B as \"b\" { on P -> y } }";
    assert_eq!(codes(src), ["E1004"]);
    let own = "event P { id: uuid }\n@topic(\"t.dlq\")\n@deadLetter\nchannel A { carries P }";
    assert_eq!(codes(own), ["E1003"]);
}

#[test]
fn a_service_that_uses_such_a_channel_uses_events_delivery() {
    let ir = ir_of("@ordered");
    let by = |name: &str| ir.services.iter().find(|s| s.name == name).map(|s| s.id);
    for who in ["Shop", "Receipts"] {
        let used = ir.features_used(&[by(who).unwrap()]);
        assert!(used.contains(&Feature::Events), "{who}");
        assert!(used.contains(&Feature::EventsDelivery), "{who}: {used:?}");
        assert!(ir.first_delivery_origin(by(who).unwrap()).is_some());
    }
    let plain = ir_of("");
    for s in &plain.services {
        assert!(
            !plain
                .features_used(&[s.id])
                .contains(&Feature::EventsDelivery)
        );
    }
}

#[test]
fn only_the_services_of_the_annotated_channel_use_the_feature() {
    let src = "event P { id: uuid }\n@deadLetter\nchannel A { carries P }\nchannel B { carries P }\nservice Pa { publishes A }\nservice Pb { publishes B }\nservice Sa { subscribes A as \"g\" { on P -> x } }\nservice Sb { subscribes B as \"g\" { on P -> y } }";
    let ir = crate::testing::ir(src);
    let used = |n: &str| {
        let id = ir.services.iter().find(|s| s.name == n).unwrap().id;
        ir.features_used(&[id]).contains(&Feature::EventsDelivery)
    };
    assert!(used("Pa") && used("Sa"));
    assert!(!used("Pb") && !used("Sb"));
}

#[test]
fn the_origin_of_the_gate_is_the_channel() {
    let ir = ir_of("@ordered");
    let shop = ir.services.iter().find(|s| s.name == "Shop").unwrap().id;
    let o = ir.first_delivery_origin(shop).unwrap();
    assert_eq!(o, &ir.channels[0].origin);
}

// ---- called once ---------------------------------------------------------------------------

fn example(expects: &str) -> String {
    format!(
        "{}\nexample \"x\" {{\n  given event Paid {{ id: \"0b0e45b8-3d2d-4c43-8a7a-0d4a1b3b9f11\" }}\n{expects}\n}}",
        contract("")
    )
}

#[test]
fn called_once_reaches_the_ir_and_plain_called_does_not_say_once() {
    let ir = crate::testing::ir(&example("  expect Receipts.send called once"));
    let Some(EventExpect::Calls(calls)) = &ir.examples[0].expect else {
        panic!("calls expected")
    };
    assert!(calls[0].called && calls[0].once);
    let v = serde_json::to_value(&ir.examples[0]).unwrap();
    assert_eq!(v["expect"][0]["once"], serde_json::json!(true));

    let ir = crate::testing::ir(&example("  expect Receipts.send called"));
    let Some(EventExpect::Calls(calls)) = &ir.examples[0].expect else {
        panic!("calls expected")
    };
    assert!(calls[0].called && !calls[0].once);
    let v = serde_json::to_value(&ir.examples[0]).unwrap();
    assert!(v["expect"][0].get("once").is_none());
}

#[test]
fn not_called_once_and_rejected_with_once_are_e1010() {
    assert_eq!(
        codes(&example("  expect Receipts.send not called once")),
        ["E1010"]
    );
    assert_eq!(codes(&example("  expect rejected once")), ["E1010"]);
    // `once` with a `called` line of another handler does not rescue `rejected`.
    assert_eq!(
        codes(&example(
            "  expect rejected\n  expect Receipts.send called once"
        )),
        ["E1010"]
    );
}

#[test]
fn once_is_an_ordinary_name_everywhere_else() {
    let src = "model Holder { once: int }\nenum E { once, other }\nconst once: int = 1\nservice S at \"/o\" {\n  get \"/a\" once(@query once: int) -> Holder\n}\nexample \"an endpoint called once\" { once(once: 1) -> ok }";
    let out = one(src);
    assert!(
        out.diagnostics
            .iter()
            .all(|d| d.code != "E0001" && d.code != "E0003"),
        "{}",
        out.render()
    );
}

#[test]
fn a_service_called_once_works_in_an_event_example() {
    let src = "event Paid { id: uuid }\nchannel C { carries Paid }\nservice P { publishes C }\nservice once { subscribes C as \"g\" { on Paid -> once } }\nexample \"x\" { given event Paid { }\n expect once.once called once }";
    let out = one(src);
    assert!(out.ir.is_some(), "{}", out.render());
    let Some(EventExpect::Calls(calls)) = &out.ir.unwrap().examples[0].expect else {
        panic!("calls expected")
    };
    assert!(calls[0].once);
}

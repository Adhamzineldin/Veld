//! Parser tests for events (`spec/events.md` section 2).

use super::{Parsed, parse};
use crate::diagnostics::FileId;
use crate::syntax::ast::{DeclKind, ExampleBody, ExpectKind};

fn p(src: &str) -> Parsed {
    parse(src, FileId(0))
}

fn clean(src: &str) -> Parsed {
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    r
}

#[test]
fn an_event_has_fields_and_rules_like_a_model() {
    let r = clean("event Paid {\n id: uuid\n amount: decimal\n rule positive: amount > 0\n}");
    let DeclKind::Event(e) = &r.file.decls[0].kind else {
        panic!("not an event")
    };
    assert_eq!(e.name.name, "Paid");
    assert_eq!(e.fields.len(), 2);
    assert_eq!(e.rules.len(), 1);
}

#[test]
fn a_channel_has_carries_and_key_lines() {
    let r = clean("@topic(\"a.b\")\nchannel C {\n carries A, B\n carries D\n key id\n}");
    let d = &r.file.decls[0];
    assert_eq!(d.annotations[0].name.name, "topic");
    let DeclKind::Channel(c) = &d.kind else {
        panic!("not a channel")
    };
    assert_eq!(c.carries.len(), 2);
    assert_eq!(c.carries[0].events.len(), 2);
    assert_eq!(c.keys[0].field.name, "id");
}

#[test]
fn a_service_may_leave_out_at_and_has_publishes_and_subscribes() {
    let r = clean(
        "service S {\n publishes A, B\n subscribes A as \"g\" {\n  /// docs\n  on X -> h1\n  on Y -> h2\n }\n}",
    );
    let DeclKind::Service(s) = &r.file.decls[0].kind else {
        panic!("not a service")
    };
    assert!(s.prefix.is_none());
    assert_eq!(s.publishes[0].channels.len(), 2);
    let sub = &s.subscribes[0];
    assert_eq!(sub.group.value, "g");
    assert_eq!(sub.handlers.len(), 2);
    assert_eq!(sub.handlers[0].docs.len(), 1);
    assert_eq!(sub.handlers[1].handler.name, "h2");
}

#[test]
fn endpoints_and_event_lines_mix_in_a_service() {
    let r = clean(
        "service S at \"/s\" {\n get \"/a\" a() -> 204\n publishes A\n subscribes B as \"g\" { on X -> h }\n get \"/b\" b()\n}",
    );
    let DeclKind::Service(s) = &r.file.decls[0].kind else {
        panic!("not a service")
    };
    assert_eq!(s.endpoints.len(), 2);
    assert_eq!(s.publishes.len(), 1);
    assert_eq!(s.subscribes.len(), 1);
}

#[test]
fn an_event_example_has_a_record_a_channel_and_expectations() {
    let r = clean(
        "example \"x\" {\n given event Paid { amount: 1.00 } on BookingEvents\n expect N.send called\n expect N.other not called\n}\nexample \"y\" { given event Paid { }\n expect rejected }",
    );
    let ExampleBody::Event(e) = &r.file.examples[0].body else {
        panic!("not an event example")
    };
    assert!(e.channel.is_some());
    assert_eq!(e.expects.len(), 2);
    assert!(matches!(
        &e.expects[1].kind,
        ExpectKind::Called { not: true, .. }
    ));
    let ExampleBody::Event(y) = &r.file.examples[1].body else {
        panic!("not an event example")
    };
    assert!(y.channel.is_none());
    assert!(matches!(y.expects[0].kind, ExpectKind::Rejected));
}

#[test]
fn the_new_words_stay_ordinary_names() {
    let src = "model event { channel: string, carries: int, key: int, on: int }\nmodel channel { event: event }\nenum E { publishes, subscribes, called, rejected }\nservice key at \"/k\" {\n get \"/a\" publishes(on: int) -> event\n get \"/b\" subscribes()\n}\nexample \"given an event fact\" { given event = absent\n  publishes(on: 1) -> ok }";
    let r = clean(src);
    assert_eq!(r.file.decls.len(), 4);
    assert!(matches!(r.file.examples[0].body, ExampleBody::Call(_)));
}

#[test]
fn a_status_only_response_may_be_followed_by_an_event_line() {
    let r = clean(
        "service S at \"/s\" {\n get \"/p\" p() -> 204\n publishes C\n subscribes D as \"g\" { on X -> h }\n}",
    );
    let DeclKind::Service(s) = &r.file.decls[0].kind else {
        panic!("not a service")
    };
    assert_eq!(s.endpoints.len(), 1);
    assert_eq!(s.publishes.len(), 1);
    assert_eq!(s.subscribes.len(), 1);
}

#[test]
fn an_event_example_needs_an_expect_line() {
    let r = p("example \"x\" { given event Paid { } }");
    assert_eq!(
        r.diagnostics.iter().filter(|d| d.code == "E0003").count(),
        1
    );
}

#[test]
fn a_broken_subscription_reports_and_the_next_item_still_parses() {
    let r = p("service S {\n subscribes A { on X -> h }\n publishes B\n}\nmodel After { a: int }");
    assert!(r.diagnostics.iter().any(|d| d.code == "E0003"));
    assert_eq!(r.file.decls.len(), 2);
}

#[test]
fn a_service_with_neither_at_nor_a_brace_is_still_an_error() {
    let r = p("service S nope");
    assert!(r.diagnostics.iter().any(|d| d.code == "E0003"));
}

#[test]
fn flow_emits_stays_rejected() {
    let r = p("enum S { a, b\n flow a -> b emits Paid\n}");
    assert!(r.diagnostics.iter().any(|d| d.code == "E0003"));
}

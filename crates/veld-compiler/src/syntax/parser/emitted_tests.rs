//! Parser tests for `emits` and `expect emitted` (`spec/events.md` section 11).

use super::{Parsed, parse};
use crate::diagnostics::FileId;
use crate::syntax::ast::{DeclKind, ExampleBody};

fn p(src: &str) -> Parsed {
    parse(src, FileId(0))
}

fn clean(src: &str) -> Parsed {
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    r
}

fn call(r: &Parsed) -> &crate::syntax::ast::ExampleCall {
    match &r.file.examples[0].body {
        ExampleBody::Call(c) => c,
        ExampleBody::Event(_) => panic!("an event example"),
    }
}

#[test]
fn a_flow_may_end_in_emits_and_an_event_name() {
    let r = clean("enum S {\n a, b, c\n flow a -> b emits Paid\n flow b -> c\n}");
    let DeclKind::Enum(e) = &r.file.decls[0].kind else {
        panic!("not an enum")
    };
    assert_eq!(
        e.flows[0].emits.as_ref().map(|i| i.name.as_str()),
        Some("Paid")
    );
    assert!(e.flows[1].emits.is_none());
}

#[test]
fn emits_without_a_name_is_a_syntax_error() {
    let r = p("enum S {\n a, b\n flow a -> b emits\n}");
    assert!(r.diagnostics.iter().any(|d| d.code == "E0003"));
}

#[test]
fn emits_stays_an_ordinary_name_off_the_line_of_a_flow() {
    let r = clean("enum S { emits, a\n flow emits -> a\n final a }\nmodel emits { emits: int }");
    let DeclKind::Enum(e) = &r.file.decls[0].kind else {
        panic!("not an enum")
    };
    assert!(e.flows[0].emits.is_none());
    assert_eq!(e.variants[0].name.name, "emits");
}

#[test]
fn a_call_example_takes_expect_emitted_lines_after_its_outcome() {
    let r = clean(
        "example \"x\" {\n pay(id: a)\n  returns { a: 1 }\n  -> ok\n expect emitted BookingPaid\n expect emitted Other\n}",
    );
    let c = call(&r);
    assert!(c.outcome.error.is_none() && c.outcome.value.is_none());
    assert_eq!(c.emitted.len(), 2);
    assert!(c.emitted.iter().all(|x| x.event.is_some()));
}

#[test]
fn expect_not_emitted_has_no_event() {
    let r = clean("example \"x\" {\n pay(id: a) -> InvalidTransition\n expect not emitted\n}");
    let c = call(&r);
    assert_eq!(c.emitted.len(), 1);
    assert!(c.emitted[0].event.is_none());
}

#[test]
fn an_outcome_value_that_is_a_name_is_still_a_value() {
    let r = clean("example \"x\" {\n pay(id: a) -> ok expect\n}");
    assert!(call(&r).outcome.value.is_some());
    assert!(call(&r).emitted.is_empty());
    let r = clean("example \"x\" {\n pay(id: a) -> ok expect.emitted\n}");
    assert!(call(&r).outcome.value.is_some());
    assert!(call(&r).emitted.is_empty());
}

#[test]
fn an_example_without_expect_lines_has_none() {
    let r = clean("example \"x\" {\n pay(id: a) -> ok\n}");
    assert!(call(&r).emitted.is_empty());
}

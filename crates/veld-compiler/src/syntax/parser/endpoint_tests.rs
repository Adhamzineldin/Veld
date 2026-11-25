//! Parser tests for endpoint blocks and examples (`spec/rules.md` sections 9 and 12).

use super::*;
use crate::syntax::ast::{DeclKind, Stmt, ValueKind};

fn p(src: &str) -> Parsed {
    parse(src, FileId(0))
}

fn codes(src: &str) -> Vec<&'static str> {
    p(src).diagnostics.iter().map(|d| d.code).collect()
}

fn endpoint(src: &str) -> Endpoint {
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{src}: {:?}", r.diagnostics);
    let DeclKind::Service(s) = &r.file.decls[0].kind else {
        panic!("not a service")
    };
    s.endpoints[0].clone()
}

const FULL: &str = r#"
service Bookings at "/b" {
  post "/" createBooking(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {
    fact room: Room = body.roomId
    fact note?: Note
    /// Docs belong to the statement.
    require not room.closed else RoomClosed
    require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
    require notInPast: body.start >= now
      message "A booking cannot start in the past"
    ensure result.status == .pending
    ensure named: old(room.capacity) >= 0 message "kept"
  }
}
"#;

#[test]
fn parses_every_statement_form() {
    let e = endpoint(FULL);
    let block = e.block.expect("a block");
    let kinds: Vec<&str> = block
        .stmts
        .iter()
        .map(|s| match s {
            Stmt::Fact(_) => "fact",
            Stmt::Require(_) => "require",
            Stmt::Ensure(_) => "ensure",
            Stmt::Transition(_) => "transition",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "fact", "fact", "require", "require", "require", "ensure", "ensure"
        ]
    );
    let Stmt::Fact(f) = &block.stmts[0] else {
        panic!()
    };
    assert!(!f.optional && f.key.is_some());
    let Stmt::Fact(f) = &block.stmts[1] else {
        panic!()
    };
    assert!(f.optional && f.key.is_none());
    let Stmt::Require(r) = &block.stmts[2] else {
        panic!()
    };
    assert!(r.name.is_none() && r.docs.len() == 1);
    assert_eq!(
        r.otherwise.as_ref().map(|o| o.payload.is_none()),
        Some(true)
    );
    let Stmt::Require(r) = &block.stmts[3] else {
        panic!()
    };
    let payload = r.otherwise.as_ref().and_then(|o| o.payload.as_ref());
    assert_eq!(payload.map(Vec::len), Some(1));
    let Stmt::Require(r) = &block.stmts[4] else {
        panic!()
    };
    assert_eq!(r.name.as_ref().map(|n| n.name.as_str()), Some("notInPast"));
    assert!(r.message.is_some());
}

#[test]
fn the_endpoint_span_covers_the_block() {
    let e = endpoint(FULL);
    let block = e.block.expect("a block");
    assert_eq!(e.span.end, block.span.end);
}

#[test]
fn a_block_may_follow_the_parameters_without_responses() {
    let e = endpoint("service S at \"/\" { get \"/\" f() { ensure true } }");
    assert!(e.responses.is_none() && e.block.is_some());
}

#[test]
fn an_endpoint_without_a_block_has_none() {
    let e = endpoint("service S at \"/\" { get \"/\" f() -> int }");
    assert!(e.block.is_none());
}

#[test]
fn allow_names_its_phase() {
    for (word, phase) in [("allow", "phase 4")] {
        let src = format!("service S at \"/\" {{ get \"/\" f() {{ {word} x(y)\n ensure true }} }}");
        let r = p(&src);
        assert_eq!(codes(&src), ["E0003"], "{word}");
        assert!(r.diagnostics[0].message.contains(phase), "{word}");
        let DeclKind::Service(s) = &r.file.decls[0].kind else {
            panic!()
        };
        let block = s.endpoints[0].block.as_ref().expect("block");
        assert_eq!(block.stmts.len(), 1, "the next statement is still parsed");
    }
}

#[test]
fn anything_else_in_a_block_is_reported_once() {
    assert_eq!(
        codes("service S at \"/\" { get \"/\" f() { nonsense here\n ensure true } }"),
        ["E0003"]
    );
}

#[test]
fn unclosed_block_terminates() {
    assert!(!codes("service S at \"/\" { get \"/\" f() { ensure true").is_empty());
}

#[test]
fn statement_names_need_the_colon() {
    // `ensure x` is an expression, `ensure x: e` a named statement.
    let e = endpoint("service S at \"/\" { get \"/\" f() { ensure x: true\n ensure y } }");
    let block = e.block.expect("block");
    let named: Vec<bool> = block
        .stmts
        .iter()
        .map(|s| matches!(s, Stmt::Ensure(e) if e.name.is_some()))
        .collect();
    assert_eq!(named, [true, false]);
}

#[test]
fn doc_on_a_fact_is_rejected() {
    assert_eq!(
        codes("service S at \"/\" { get \"/\" f() {\n /// no\n fact a: int } }"),
        ["E0003"]
    );
}

#[test]
fn contextual_words_stay_names() {
    // Endpoints and parameters called like the new words, with and without a block.
    let src = r#"
service S at "/" {
  get "/a" fact(@query require: int) -> int
  get "/b" ensure(@query old: int) -> int { require old > 0 }
  get "/c" example(@query given: int) -> int
  get "/d" absent(@query returns: int) -> int
  get "/e" ok(@query ok: int) -> int
}
"#;
    assert!(p(src).diagnostics.is_empty(), "{:?}", p(src).diagnostics);
}

const EXAMPLE: &str = r#"
/// Docs.
example "a full room is rejected" {
  given room = { capacity: 2, closed: false }
  given gone = absent
  as user
  at "2026-03-01T09:00:00Z"
  Bookings.createBooking(body: { roomId: room.id, guests: 3, tags: ["a"], n: -1 })
    returns { status: .paid }
    -> RoomFull { capacity: 2 }
}

example "ok with nothing" {
  list()
    -> ok
}
"#;

#[test]
fn parses_examples() {
    let r = p(EXAMPLE);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    assert_eq!(r.file.examples.len(), 2);
    let e = &r.file.examples[0];
    assert_eq!(e.docs.len(), 1);
    assert_eq!(e.name.value, "a full room is rejected");
    assert_eq!(e.givens.len(), 2);
    assert!(e.givens[0].value.is_some() && e.givens[1].value.is_none());
    assert!(e.principal.is_some() && e.at.is_some());
    let call = &e.call;
    assert_eq!(
        call.service.as_ref().map(|s| s.name.as_str()),
        Some("Bookings")
    );
    assert_eq!(call.endpoint.name, "createBooking");
    assert!(call.returns.is_some());
    assert!(call.outcome.error.is_some() && call.outcome.value.is_some());
    let ValueKind::Object(fields) = &call.args[0].value.kind else {
        panic!()
    };
    assert!(matches!(fields[0].value.kind, ValueKind::Path(ref p) if p.len() == 2));
    assert!(matches!(fields[3].value.kind, ValueKind::Int(-1)));
    let second = &r.file.examples[1];
    assert!(second.call.outcome.error.is_none() && second.call.outcome.value.is_none());
}

#[test]
fn a_negative_literal_after_a_word_is_a_literal() {
    let r = p("example \"x\" { f(a: 1) returns -5 -> ok -2.5 }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let c = &r.file.examples[0].call;
    assert!(matches!(
        c.returns.as_ref().map(|v| &v.kind),
        Some(ValueKind::Int(-5))
    ));
    assert!(matches!(
        c.outcome.value.as_ref().map(|v| &v.kind),
        Some(ValueKind::Float { .. })
    ));
}

#[test]
fn an_endpoint_may_be_called_given_and_a_given_may_be_named_given() {
    let r = p("example \"x\" { given given = absent\n given(a: 1) -> ok }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let e = &r.file.examples[0];
    assert_eq!(e.givens[0].name.name, "given");
    assert_eq!(e.call.endpoint.name, "given");
}

#[test]
fn example_is_an_ordinary_name_without_a_string() {
    let r = p("model example { value: int }\nexample \"x\" { f() -> ok }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    assert_eq!(r.file.decls.len(), 1);
    assert_eq!(r.file.examples.len(), 1);
}

#[test]
fn a_broken_example_is_skipped_and_the_next_declaration_still_parses() {
    let r = p("example \"x\" { f( -> ok }\nmodel M { a: int }");
    assert!(!r.diagnostics.is_empty());
    assert_eq!(r.file.decls.len(), 1);
}

#[test]
fn null_is_not_a_value() {
    assert_eq!(codes("example \"x\" { f(a: null) -> ok }"), ["E0003"]);
}

#[test]
fn value_nesting_is_bounded() {
    let deep = format!("{}1{}", "[".repeat(40), "]".repeat(40));
    let src = format!("example \"x\" {{ f(a: {deep}) -> ok }}");
    assert!(codes(&src).contains(&"E0005"));
    let fine = format!("{}1{}", "[".repeat(30), "]".repeat(30));
    let src = format!("example \"x\" {{ f(a: {fine}) -> ok }}");
    assert!(codes(&src).is_empty());
}

#[test]
fn use_after_an_example_is_still_an_error() {
    let src = "example \"x\" { f() -> ok }\nuse a.{B}";
    assert!(codes(src).contains(&"E0003"));
}

fn block_of(src: &str) -> Vec<Stmt> {
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{src}: {:?}", r.diagnostics);
    let DeclKind::Service(s) = &r.file.decls[0].kind else {
        panic!("not a service")
    };
    s.endpoints[0].block.clone().expect("a block").stmts
}

#[test]
fn a_transition_has_a_path_sources_and_a_target() {
    let stmts = block_of(
        "service S at \"/\" { post \"/\" f() { fact b: B = id\n transition b.status: pending, paid -> cancelled\n require true } }",
    );
    let Stmt::Transition(t) = &stmts[1] else {
        panic!("not a transition");
    };
    let path: Vec<&str> = t.path.iter().map(|i| i.name.as_str()).collect();
    let from: Vec<&str> = t.from.iter().map(|i| i.name.as_str()).collect();
    assert_eq!((path, from), (vec!["b", "status"], vec!["pending", "paid"]));
    assert_eq!(t.to.name, "cancelled");
    assert_eq!(stmts.len(), 3, "the statement after it is still parsed");
}

#[test]
fn a_nested_path_in_a_transition_parses() {
    let stmts = block_of("service S at \"/\" { post \"/\" f() { transition a.b.c: x -> y } }");
    let Stmt::Transition(t) = &stmts[0] else {
        panic!("not a transition");
    };
    assert_eq!(t.path.len(), 3);
}

#[test]
fn a_broken_transition_is_one_diagnostic_and_the_block_goes_on() {
    for src in [
        "service S at \"/\" { post \"/\" f() { transition b.s pending -> paid\n ensure true } }",
        "service S at \"/\" { post \"/\" f() { transition b.s: pending paid\n ensure true } }",
        "service S at \"/\" { post \"/\" f() { transition b.s: -> paid\n ensure true } }",
    ] {
        assert_eq!(codes(src), ["E0003"], "{src}");
    }
}

fn enum_of(src: &str) -> crate::syntax::ast::EnumDecl {
    let r = p(src);
    assert!(r.diagnostics.is_empty(), "{src}: {:?}", r.diagnostics);
    let DeclKind::Enum(e) = &r.file.decls[0].kind else {
        panic!("not an enum")
    };
    e.clone()
}

fn names(ids: &[crate::syntax::ast::Ident]) -> Vec<&str> {
    ids.iter().map(|i| i.name.as_str()).collect()
}

#[test]
fn flows_and_finals_are_statements_of_an_enum_body() {
    let e =
        enum_of("enum S {\n a, b, c\n flow a -> b -> c\n flow a -> c\n final c\n final a2, b2\n}");
    assert_eq!(
        names(
            &e.variants
                .iter()
                .map(|v| v.name.clone())
                .collect::<Vec<_>>()
        ),
        ["a", "b", "c"]
    );
    assert_eq!(e.flows.len(), 2);
    assert_eq!(names(&e.flows[0].states), ["a", "b", "c"]);
    assert_eq!(names(&e.finals[1].states), ["a2", "b2"]);
}

#[test]
fn flow_and_final_stay_variants_without_the_lookahead() {
    // `flow` not followed by `IDENT ->`, and `final` not followed by an identifier on its line.
    let e = enum_of("enum E { flow, final, other }");
    assert_eq!(e.variants.len(), 3);
    assert!(e.flows.is_empty() && e.finals.is_empty());
    let e = enum_of("enum E {\n flow\n final\n other\n}");
    assert_eq!(e.variants.len(), 3);
    let e = enum_of("enum E {\n final = \"F\"\n flow = \"L\"\n}");
    assert_eq!(e.variants.len(), 2);
}

#[test]
fn a_machine_may_still_have_variants_named_flow_and_final() {
    let e = enum_of("enum E {\n flow, final, x\n flow flow -> final\n final final\n}");
    assert_eq!(e.variants.len(), 3);
    assert_eq!(names(&e.flows[0].states), ["flow", "final"]);
    assert_eq!(names(&e.finals[0].states), ["final"]);
}

#[test]
fn emits_is_rejected_and_names_events() {
    let src = "enum E {\n a, b\n flow a -> b emits Booked\n final b\n}";
    let r = p(src);
    assert_eq!(codes(src), ["E0003"]);
    assert!(r.diagnostics[0].message.contains("event"));
    let DeclKind::Enum(e) = &r.file.decls[0].kind else {
        panic!("not an enum")
    };
    assert_eq!(e.finals.len(), 1, "the next statement is still parsed");
}

#[test]
fn a_flow_needs_a_target() {
    assert_eq!(codes("enum E { a, b\n flow a -> \n}"), ["E0003"]);
}

#[test]
fn docs_and_annotations_do_not_belong_to_a_flow() {
    assert_eq!(
        codes("enum E {\n a, b\n /// doc\n flow a -> b\n}"),
        ["E0003"]
    );
}

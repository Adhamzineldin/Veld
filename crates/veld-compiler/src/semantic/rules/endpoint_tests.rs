//! Endpoint blocks: scope, typing, guards and lowering (`spec/rules.md` section 9).

use crate::testing::{codes, ir, one};
use veld_ir::Ir;
use veld_ir::rules::{ExprKind, Root};

const HEAD: &str = r#"
model Room { id: uuid, capacity: int, closed: bool, owner?: string }
model NewBooking { roomId: uuid, guests: int, start: datetime, note?: string }
model Booking { id: uuid, status: Status }
enum Status { pending, paid }
error RoomClosed 409 { }
error RoomFull 409 { capacity: int }
"#;

fn svc(body: &str) -> String {
    format!("{HEAD}\nservice Bookings at \"/b\" {{\n{body}\n}}\n")
}

fn create(block: &str) -> String {
    svc(&format!(
        "  post \"/\" create(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {{\n{block}\n  }}"
    ))
}

fn compile(block: &str) -> Ir {
    ir(&create(block))
}

fn codes_of(block: &str) -> Vec<&'static str> {
    codes(&create(block))
}

const FULL: &str = r#"
    fact room: Room = body.roomId
    require not room.closed else RoomClosed
    require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
    require notInPast: body.start >= now message "past"
    ensure result.status == .pending
    ensure old(room.capacity) >= 0
"#;

#[test]
fn a_full_block_lowers_in_source_order() {
    let ir = compile(FULL);
    let ep = &ir.services[0].endpoints[0];
    let r = &ep.rules;
    assert_eq!(r.facts.len(), 1);
    assert_eq!(r.requires.len(), 3);
    assert_eq!(r.ensures.len(), 2);
    assert_eq!(r.requires[2].name.as_deref(), Some("notInPast"));
    assert_eq!(r.requires[2].message.as_deref(), Some("past"));
    assert_eq!(r.requires[2].reads, ["body"]);
    assert!(r.requires[0].reads.is_empty(), "a fact is not a parameter");
    let els = r.requires[1].else_error.as_ref().expect("else");
    assert_eq!(els.status, 409);
    assert_eq!(els.fields.len(), 1);
    assert_eq!(els.fields[0].wire, "capacity");
    assert!(matches!(
        &r.ensures[1].expr.e,
        ExprKind::Bin(b) if matches!(b.l.e, ExprKind::Old(_))
    ));
    assert!(matches!(
        &r.facts[0].key.as_ref().map(|k| &k.e),
        Some(ExprKind::Path(p)) if p.root == Root::Param("body".into())
    ));
}

#[test]
fn a_required_fact_adds_not_found_like_the_implicit_validation_error() {
    let ir = compile(FULL);
    let names: Vec<&str> = ir.services[0].endpoints[0]
        .errors
        .iter()
        .map(|e| ir.decls[e.decl.index()].name.as_str())
        .collect();
    assert!(names.contains(&"NotFound") && names.contains(&"ValidationError"));
    let ir = compile("    fact room?: Room = body.roomId\n    require body.guests > 0");
    let names: Vec<&str> = ir.services[0].endpoints[0]
        .errors
        .iter()
        .map(|e| ir.decls[e.decl.index()].name.as_str())
        .collect();
    assert!(!names.contains(&"NotFound"), "{names:?}");
}

#[test]
fn an_endpoint_without_a_block_lowers_as_before() {
    let ir = ir(&svc("  get \"/x\" f() -> 200 Booking"));
    let ep = &ir.services[0].endpoints[0];
    assert!(ep.rules.is_empty());
    assert!(ir.services[0].facts.is_empty());
    let names: Vec<&str> = ep
        .errors
        .iter()
        .map(|e| ir.decls[e.decl.index()].name.as_str())
        .collect();
    assert_eq!(names, ["Internal"]);
}

#[test]
fn a_bare_require_adds_validation_error_even_without_parameters() {
    let ir = ir(&svc("  get \"/x\" f() -> 200 Booking { require now > now }"));
    let names: Vec<&str> = ir.services[0].endpoints[0]
        .errors
        .iter()
        .map(|e| ir.decls[e.decl.index()].name.as_str())
        .collect();
    assert!(names.contains(&"ValidationError"), "{names:?}");
}

#[test]
fn service_facts_are_one_per_name_sorted() {
    let src = svc(r#"
  post "/a" a(body: NewBooking) -> 201 Booking { fact room: Room = body.roomId
    fact booking?: Booking }
  post "/b" b(body: NewBooking) -> 201 Booking { fact room: Room = body.roomId }
"#);
    let ir = ir(&src);
    let facts = &ir.services[0].facts;
    let names: Vec<&str> = facts.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["booking", "room"]);
    assert!(facts[0].optional && facts[0].key.is_none());
    assert!(!facts[1].optional && facts[1].key.is_some());
}

#[test]
fn reads_are_parameters_in_order_of_first_use() {
    let src = svc(r#"
  put "/{id}" f(id: int, body: NewBooking, @query limit: int) -> 200 Booking {
    require limit > id && body.guests > id
  }"#);
    let ir = ir(&src);
    assert_eq!(
        ir.services[0].endpoints[0].rules.requires[0].reads,
        ["limit", "id", "body"]
    );
}

// ---- scope -----------------------------------------------------------------------

#[test]
fn unknown_names_are_e0901_with_the_names_in_scope() {
    let out = one(&create("    require zzz.guests > 0"));
    assert_eq!(
        out.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["E0901"]
    );
    let text = out.render();
    assert!(text.contains("names in scope: body, now"), "{text}");
    let text = one(&create("    require bodyy.guests > 0")).render();
    assert!(text.contains("did you mean `body`"), "{text}");
}

#[test]
fn facts_see_the_facts_before_them_only() {
    let ok = "    fact a: Room = body.roomId\n    fact b: Booking = a.id";
    assert!(codes_of(ok).is_empty(), "{:?}", codes_of(ok));
    let bad = "    fact b: Booking = a.id\n    fact a: Room = body.roomId";
    assert_eq!(codes_of(bad), ["E0901"]);
}

#[test]
fn require_sees_every_fact_wherever_it_is_declared() {
    let src = "    require room.capacity > 0\n    fact room: Room = body.roomId";
    assert!(codes_of(src).is_empty(), "{:?}", codes_of(src));
}

#[test]
fn a_fact_named_like_another_name_is_e0920() {
    for dup in [
        "fact body: Room",
        "fact a: Room\n    fact a: Room",
        "fact now: Room",
        "fact result: Room",
    ] {
        assert_eq!(codes_of(&format!("    {dup}")), ["E0920"], "{dup}");
    }
}

#[test]
fn a_stream_body_is_not_in_scope() {
    let src = svc(r#"
  ws "/live" live(body: stream Booking) -> stream Booking {
    require body.id == body.id
  }"#);
    assert!(codes(&src).contains(&"E0901"), "{:?}", codes(&src));
}

#[test]
fn a_parameter_named_now_keeps_its_name() {
    let src = svc(r#"
  get "/x" f(@query now: int) -> 200 Booking { require now > 0 }"#);
    let out = one(&src);
    assert!(out.ir.is_some(), "{}", out.render());
    let ir = out.ir.expect("ir");
    let e = &ir.services[0].endpoints[0].rules.requires[0].expr;
    let ExprKind::Bin(b) = &e.e else { panic!() };
    assert!(matches!(&b.l.e, ExprKind::Path(p) if p.root == Root::Param("now".into())));
}

// ---- now, result, old ------------------------------------------------------------

#[test]
fn now_is_available_in_every_statement() {
    let src = "    fact room: Room = body.roomId\n    require body.start >= now\n    ensure now >= body.start";
    assert!(codes_of(src).is_empty(), "{:?}", codes_of(src));
}

#[test]
fn now_is_a_datetime_without_fields() {
    assert_eq!(codes_of("    require now.x > now"), ["E0902"]);
    assert_eq!(codes_of("    require has(now)"), ["E0906"]);
}

#[test]
fn result_and_old_only_exist_in_ensure() {
    assert_eq!(codes_of("    require result.id == body.roomId"), ["E0904"]);
    assert_eq!(codes_of("    require old(body.guests) > 0"), ["E0904"]);
    assert!(codes_of("    ensure result.id == result.id").is_empty());
    assert!(codes_of("    ensure old(body.guests) > 0").is_empty());
}

#[test]
fn old_cannot_nest_and_takes_one_argument() {
    assert_eq!(codes_of("    ensure old(old(body.guests)) > 0"), ["E0904"]);
    assert_eq!(codes_of("    ensure old(body.guests, 1) > 0"), ["E0906"]);
}

#[test]
fn result_needs_a_success_body() {
    let src = svc("  delete \"/x\" f(@query q: uuid) -> 204 { ensure result.id == q }");
    assert_eq!(codes(&src), ["E0904"]);
    let src = svc("  delete \"/x\" f() { ensure true }");
    assert!(codes(&src).is_empty());
}

#[test]
fn result_is_not_in_a_fact_key_or_an_else_payload() {
    assert_eq!(codes_of("    fact a: Room = result.id"), ["E0904"]);
    let src = "    require body.guests > 0 else RoomFull { capacity: result.id }";
    assert_eq!(codes_of(src), ["E0904"]);
}

#[test]
fn a_fact_read_in_ensure_must_be_inside_old() {
    let bare = "    fact room: Room = body.roomId\n    ensure room.capacity >= 0";
    assert_eq!(codes_of(bare), ["E0923"]);
    let wrapped = "    fact room: Room = body.roomId\n    ensure old(room.capacity) >= 0";
    assert!(codes_of(wrapped).is_empty());
}

#[test]
fn ensure_on_a_stream_is_e0924() {
    let src = svc("  get \"/s\" s() -> stream Booking { ensure true }");
    assert_eq!(codes(&src), ["E0924"]);
}

// ---- guards ----------------------------------------------------------------------

#[test]
fn optional_parameters_and_fields_need_guards() {
    let src = svc(r#"
  get "/x" f(@query limit?: int) -> 200 Booking { require limit > 0 }"#);
    assert_eq!(codes(&src), ["E0903"]);
    let src = svc(r#"
  get "/x" f(@query limit?: int) -> 200 Booking { require has(limit) implies limit > 0 }"#);
    assert!(codes(&src).is_empty(), "{:?}", codes(&src));
    assert_eq!(codes_of("    require body.note == \"x\""), ["E0903"]);
    assert!(codes_of("    require has(body.note) && body.note == \"x\"").is_empty());
}

#[test]
fn has_on_a_required_parameter_is_e0906() {
    assert_eq!(codes_of("    require has(body)"), ["E0906"]);
}

#[test]
fn an_optional_fact_needs_a_guard() {
    let unguarded = "    fact room?: Room = body.roomId\n    require room.capacity > 0";
    assert_eq!(codes_of(unguarded), ["E0903"]);
    let guarded = "    fact room?: Room = body.roomId\n    require has(room) implies room.capacity > 0";
    assert!(codes_of(guarded).is_empty(), "{:?}", codes_of(guarded));
}

#[test]
fn the_key_of_a_required_fact_must_be_known_present_but_not_an_optional_fact() {
    let src = |opt: &str| {
        svc(&format!(
            "  get \"/x\" f(@query id?: uuid) -> 200 Booking {{ fact room{opt}: Room = id }}"
        ))
    };
    assert_eq!(codes(&src("")), ["E0903"]);
    assert!(codes(&src("?")).is_empty(), "{:?}", codes(&src("?")));
}

#[test]
fn a_guard_across_old_is_written_inside_it() {
    let src = "    fact room?: Room = body.roomId\n    ensure old(has(room) implies room.capacity >= 0)";
    assert!(codes_of(src).is_empty(), "{:?}", codes_of(src));
    let bad = "    fact room?: Room = body.roomId\n    ensure old(room.capacity) >= 0";
    assert_eq!(codes_of(bad), ["E0903"]);
}

#[test]
fn the_payload_does_not_inherit_the_guard_of_its_require() {
    let src = svc(r#"
  get "/x" f(@query n?: int) -> 200 Booking | RoomFull {
    require has(n) && n > 0 else RoomFull { capacity: n }
  }"#);
    assert_eq!(codes(&src), ["E0903"]);
}

// ---- statements ------------------------------------------------------------------

#[test]
fn a_require_must_be_a_bool() {
    assert_eq!(codes_of("    require body.guests"), ["E0902"]);
    assert_eq!(codes_of("    ensure body.guests"), ["E0902"]);
}

#[test]
fn statement_names_are_unique_across_require_and_ensure() {
    assert_eq!(
        codes_of("    require r: true\n    require r: true"),
        ["E0907"]
    );
    assert_eq!(codes_of("    require r: true\n    ensure r: true"), ["E0907"]);
    assert!(codes_of("    require true\n    require true").is_empty());
}

#[test]
fn else_must_name_a_declared_error() {
    let src = svc(r#"
  get "/x" f() -> 200 Booking { require false else RoomFull { capacity: 1 } }"#);
    assert_eq!(codes(&src), ["E0921"]);
    assert_eq!(codes_of("    require false else Status"), ["E0921"]);
    assert_eq!(codes_of("    require false else Booking"), ["E0921"]);
}

#[test]
fn else_may_name_an_implicit_error_of_the_endpoint() {
    let src = "    require body.guests > 0 else ValidationError { issues: [] }";
    assert!(codes_of(src).is_empty(), "{:?}", codes_of(src));
}

#[test]
fn the_payload_is_checked_against_the_error() {
    assert_eq!(codes_of("    require true else RoomFull"), ["E0922"]);
    assert_eq!(
        codes_of("    require true else RoomFull { capacity: 1, extra: 2 }"),
        ["E0922"]
    );
    assert_eq!(
        codes_of("    require true else RoomFull { capacity: 1, capacity: 2 }"),
        ["E0922"]
    );
    assert_eq!(
        codes_of("    require true else RoomFull { capacity: \"x\" }"),
        ["E0902"]
    );
    assert!(codes_of("    require true else RoomFull { capacity: 1 }").is_empty());
    assert!(codes_of("    require true else RoomClosed").is_empty());
    assert!(codes_of("    require true else RoomClosed {}").is_empty());
}

#[test]
fn a_payload_value_widens_exactly() {
    let src = svc(r#"
  error Big 409 { n: long }
  get "/x" f() -> 200 Booking | Big { require false else Big { n: 1 } }"#);
    // an `error` inside a service is a syntax error; declare it at the top level instead
    assert!(!codes(&src).is_empty());
    let src = format!(
        "{HEAD}\nerror Big 409 {{ n: long }}\nservice S at \"/\" {{\n  get \"/x\" f(@query a: int) -> 200 Booking | Big {{ require false else Big {{ n: a }} }}\n}}\n"
    );
    let ir = ir(&src);
    let f = &ir.services[0].endpoints[0].rules.requires[0]
        .else_error
        .as_ref()
        .expect("else")
        .fields[0];
    assert!(matches!(f.value.e, ExprKind::Widen(_)));
}

#[test]
fn a_fact_must_agree_across_the_endpoints_of_a_service() {
    let two = |a: &str, b: &str| {
        svc(&format!(
            "  post \"/a\" a(body: NewBooking) -> 201 Booking {{ {a} }}\n  post \"/b\" b(body: NewBooking) -> 201 Booking {{ {b} }}"
        ))
    };
    let ok = two("fact room: Room = body.roomId", "fact room: Room = body.roomId");
    assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
    for (a, b) in [
        ("fact room: Room = body.roomId", "fact room: Booking = body.roomId"),
        ("fact room: Room = body.roomId", "fact room: Room = body.guests"),
        ("fact room: Room = body.roomId", "fact room: Room"),
        ("fact room: Room = body.roomId", "fact room?: Room = body.roomId"),
    ] {
        assert_eq!(codes(&two(a, b)), ["E0922"], "{a} / {b}");
    }
}

#[test]
fn a_duration_cannot_key_a_fact() {
    let src = "    fact a: Room = days(1)";
    assert_eq!(codes_of(src), ["E0902"]);
}

#[test]
fn an_expression_at_the_depth_limit_is_e0005() {
    let deep = vec!["body.guests > 0"; 40].join(" && ");
    assert_eq!(codes_of(&format!("    require {deep}")), ["E0005"]);
}

#[test]
fn a_broken_statement_does_not_hide_the_next() {
    let out = one(&create("    require nope > 0\n    require alsoNope > 0"));
    assert_eq!(
        out.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["E0901", "E0901"]
    );
}

#[test]
fn invariants_still_report_now_result_and_old() {
    let src = "model M { a: int\n rule r: a > now\n rule s: old(a) > 0 }";
    assert_eq!(codes(src), ["E0904", "E0904"]);
}

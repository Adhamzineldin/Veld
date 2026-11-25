//! Formatter behaviour for endpoint blocks and examples (`spec/rules.md` sections 9 and 12).

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

const MESSY: &str = r#"
service Bookings at "/b" {
  get "/ping" ping() -> 204
  post "/" createBooking(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {
    ensure   result.status==.pending
    require notInPast:   body.start>=now
         message "A booking cannot start in the past"
    require body.guests<=room.capacity else RoomFull{capacity:room.capacity,}
    fact room:Room=body.roomId
    require not room.closed else RoomClosed {}
    fact note ?: Note
    ensure named: old(room.capacity) >= 0 message "kept"
  }
  get "/other" other() -> 204
}
"#;

#[test]
fn statements_are_grouped_facts_requires_ensures_each_in_source_order() {
    let out = stable(MESSY);
    assert_eq!(
        out,
        r#"service Bookings at "/b" {
  get "/ping" ping() -> 204

  post "/" createBooking(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {
    fact room: Room = body.roomId
    fact note?: Note
    require notInPast: body.start >= now
      message "A booking cannot start in the past"
    require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
    require not room.closed else RoomClosed {}
    ensure result.status == .pending
    ensure named: old(room.capacity) >= 0
      message "kept"
  }

  get "/other" other() -> 204
}
"#
    );
}

#[test]
fn an_empty_block_stays_empty_and_a_block_without_responses_works() {
    assert_eq!(
        stable("service S at \"/\" { get \"/\" f() {} }"),
        "service S at \"/\" {\n  get \"/\" f() {}\n}\n"
    );
    assert_eq!(
        stable("service S at \"/\" { get \"/\" f() { ensure true } }"),
        "service S at \"/\" {\n  get \"/\" f() {\n    ensure true\n  }\n}\n"
    );
}

#[test]
fn a_wrapped_head_keeps_its_block() {
    let src = "service S at \"/\" { post \"/\" createOrder(@header(\"X-Key\") key?: string, body: CreateOrder) -> 201 Order | OutOfStock | Conflict | BadRequest | Forbidden | Unauthorized { require true } }";
    let out = stable(src);
    assert!(
        out.contains("| Unauthorized {\n    require true\n  }"),
        "{out}"
    );
}

#[test]
fn comments_in_blocks_survive_and_stay_with_their_statement() {
    let src = r#"service S at "/" {
  post "/" f(body: B) -> 201 B { // header note
    // about the fact
    fact room: Room = body.id // trailing fact
    /// Documented.
    require a: body.n > 0 // trailing require
    /* before ensure */
    ensure body.n > 1
    // tail of the block
  } // after the endpoint
}
"#;
    assert_eq!(stable(src), src);
}

#[test]
fn comments_move_with_a_regrouped_statement() {
    let out = stable(
        "service S at \"/\" {\n  get \"/\" f() {\n    ensure true // e\n    // about a\n    fact a: A\n  }\n}\n",
    );
    assert_eq!(
        out,
        "service S at \"/\" {\n  get \"/\" f() {\n    // about a\n    fact a: A\n    ensure true // e\n  }\n}\n"
    );
}

#[test]
fn a_comment_before_the_brace_of_a_wrapped_head_is_stable() {
    let src = "service S at \"/\" {\n  post \"/\" createOrder(body: CreateOrder) -> 201 Order | OutOfStock | Conflict | BadRequest | Forbidden | Unauthorized /* c */ {\n    require true\n  }\n}\n";
    stable(src);
}

#[test]
fn contextual_words_stay_names_through_the_formatter() {
    let src = "service S at \"/\" {\n  get \"/\" fact(@query require: int) -> 200 int {\n    require require > 0\n  }\n}\n\nmodel example {\n  value: int\n}\n";
    assert_eq!(stable(src), src);
}

const EXAMPLES: &str = r#"
/// What the call does.
example "a full room is rejected" {
  given room = {capacity:2,closed:false}
    given   gone = absent
  at "2026-03-01T09:00:00Z"
  createBooking(body:{roomId:room.id,guests:3,tags:["a",.b,-1,2.50]}) -> RoomFull {capacity:2}
}
example "with returns" {
  Bookings.createBooking(body: {guests: 1})
    returns { status: .paid }
    -> Internal
}
example "plain" { ping() -> ok }
"#;

#[test]
fn examples_print_in_the_canonical_form() {
    let out = stable(EXAMPLES);
    assert_eq!(
        out,
        r#"/// What the call does.
example "a full room is rejected" {
  given room = { capacity: 2, closed: false }
  given gone = absent
  at "2026-03-01T09:00:00Z"
  createBooking(body: { roomId: room.id, guests: 3, tags: ["a", .b, -1, 2.50] }) -> RoomFull { capacity: 2 }
}

example "with returns" {
  Bookings.createBooking(body: { guests: 1 })
    returns { status: .paid }
    -> Internal
}

example "plain" {
  ping() -> ok
}
"#
    );
}

#[test]
fn examples_and_declarations_keep_their_source_order() {
    let src = "model A { a: int }\nexample \"x\" { f() -> ok }\nmodel B { b: int }\n";
    assert_eq!(
        stable(src),
        "model A {\n  a: int\n}\n\nexample \"x\" {\n  f() -> ok\n}\n\nmodel B {\n  b: int\n}\n"
    );
}

#[test]
fn comments_in_examples_survive() {
    let src = r#"// file header

// about the example
example "x" { // header
  // about the given
  given room = absent // trailing given
  as user // principal
  at "2026-03-01T09:00:00Z"
  // about the call
  f() -> ok // trailing call
  // tail
} // after
"#;
    let out = stable(src);
    for c in [
        "// file header",
        "// about the example",
        "// header",
        "// about the given",
        "// trailing given",
        "// principal",
        "// about the call",
        "// trailing call",
        "// tail",
        "// after",
    ] {
        assert!(out.contains(c), "lost {c}:\n{out}");
    }
}

#[test]
fn negative_values_after_words_are_stable() {
    let out = stable("example \"x\" { f(a: -1) returns -5 -> ok -2.5 }");
    assert!(
        out.contains("returns -5") && out.contains("-> ok -2.5"),
        "{out}"
    );
}

#[test]
fn an_endpoint_called_given_and_a_given_called_given_are_stable() {
    let out = stable("example \"x\" { given given = absent\n given() -> ok }");
    assert!(
        out.contains("given given = absent") && out.contains("given() -> ok"),
        "{out}"
    );
}

// ---- state machines (`spec/rules.md` section 10) ---------------------------------------------------

#[test]
fn an_enum_prints_variants_then_flows_then_finals() {
    let out = stable(
        "enum S {\n  flow a   ->  b->c\n  a, b, c\n  final   c\n  flow a -> c\n  final b,  a\n}\n",
    );
    assert_eq!(
        out,
        "enum S {\n  a,\n  b,\n  c,\n\n  flow a -> b -> c\n  flow a -> c\n  final c\n  final b, a\n}\n"
    );
}

#[test]
fn a_plain_enum_keeps_its_one_line_form() {
    assert_eq!(stable("enum S { a,b ,c }"), "enum S { a, b, c }\n");
    assert_eq!(
        stable("enum S { flow, final, x }"),
        "enum S { flow, final, x }\n"
    );
}

#[test]
fn variants_named_flow_and_final_survive_in_a_machine() {
    let src = "enum S {\n  start,\n  flow,\n  final,\n\n  flow start -> flow -> final\n  final final\n}\n";
    assert_eq!(stable(src), src);
}

#[test]
fn comments_stay_with_their_flow_and_final_statements() {
    let src = "enum S { // header\n  a,\n  b, // about b\n\n  // the only way\n  flow a -> b // trailing flow\n  // b is the end\n  final b // trailing final\n  // tail\n}\n";
    let out = stable(src);
    for c in [
        "// header",
        "// about b",
        "// the only way",
        "// trailing flow",
        "// b is the end",
        "// trailing final",
        "// tail",
    ] {
        assert!(out.contains(c), "lost {c}:\n{out}");
    }
}

#[test]
fn a_transition_sits_between_facts_and_requires() {
    let out = stable(
        "service S at \"/\" {\n  post \"/\" f() -> 204 {\n    ensure true\n    require true\n    transition  b.status :pending,paid->cancelled\n    fact b: B = id\n  }\n}\n",
    );
    assert_eq!(
        out,
        "service S at \"/\" {\n  post \"/\" f() -> 204 {\n    fact b: B = id\n    transition b.status: pending, paid -> cancelled\n    require true\n    ensure true\n  }\n}\n"
    );
}

#[test]
fn comments_move_with_a_transition() {
    let out = stable(
        "service S at \"/\" {\n  post \"/\" f() {\n    require true\n    // the move\n    transition b.s: a -> b // trailing\n    fact b: B\n  }\n}\n",
    );
    assert!(
        out.contains("    // the move\n    transition b.s: a -> b // trailing\n    require true"),
        "{out}"
    );
}

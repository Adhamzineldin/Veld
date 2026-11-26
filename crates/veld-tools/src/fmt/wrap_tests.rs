//! Line wrapping at 100 columns (`wrap.rs`): each construct, what stays as it was, and that the
//! result is a fixed point that still parses.

use super::format;

fn f(src: &str) -> String {
    let once = format(src)
        .unwrap_or_else(|d| format!("ERR {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>()));
    assert_eq!(
        format(&once).ok().as_deref(),
        Some(once.as_str()),
        "not idempotent:\n{once}"
    );
    once
}

const HEAD: &str = "model Req { roomId: uuid, note?: string, guests: int }\nmodel Hold { id: uuid }\nerror RoomFull 409 { capacity: int }\n";

#[test]
fn a_short_rule_stays_on_one_line() {
    let out = f("model M {\n  a: int\n  b: int\n\n  rule r: a > 1 && b > 1\n}");
    assert!(out.contains("  rule r: a > 1 && b > 1\n"), "{out}");
}

#[test]
fn a_long_rule_breaks_before_the_operator_of_the_lowest_level() {
    let src = "model M {\n  alpha: int\n  rule long: alpha > 1000000 && alpha > 2000000 && alpha > 3000000 && alpha > 4000000 && alpha < 9999999\n    message \"m\"\n}";
    let out = f(src);
    let want = "  rule long: alpha > 1000000\n      && alpha > 2000000\n      && alpha > 3000000\n      && alpha > 4000000\n      && alpha < 9999999\n    message \"m\"\n";
    assert!(out.contains(want), "{out}");
}

#[test]
fn a_long_implies_chain_breaks_before_each_implies() {
    let src = "model M {\n  alpha: int\n  rule r: alpha > 1000000 implies alpha > 2000000 implies alpha > 3000000 implies alpha > 4000000 implies alpha > 5\n}";
    let out = f(src);
    assert!(out.contains("  rule r: alpha > 1000000\n      implies alpha > 2000000\n      implies alpha > 3000000\n"), "{out}");
}

#[test]
fn a_comparison_at_the_top_breaks_before_the_comparison() {
    let src = "model M {\n  alpha: int\n  rule r: alpha + alpha + alpha + alpha + alpha + alpha + alpha + alpha + alpha + alpha + alpha + alpha + 1 > 5\n}";
    let out = f(src);
    assert!(out.ends_with("+ 1\n      > 5\n}\n"), "{out}");
}

#[test]
fn a_long_policy_clause_and_a_long_require_are_broken() {
    let src = format!(
        "{HEAD}policy P(user: Req) {{\n  permit when user.guests > 1000000 || user.guests < 2000000 || user.guests == 3000000 || user.guests == 4000000\n}}\nservice S at \"/s\" {{\n  post \"/x\" x(body: Req) -> Hold {{\n    require body.guests > 1000000 && body.guests < 2000000 && body.guests != 3000000 && body.guests != 4000000 else RoomFull {{ capacity: 1 }}\n  }}\n}}\n"
    );
    let out = f(&src);
    assert!(
        out.contains("  permit when user.guests > 1000000\n      || user.guests < 2000000\n"),
        "{out}"
    );
    assert!(
        out.contains("    require body.guests > 1000000\n        && body.guests < 2000000\n"),
        "{out}"
    );
    assert!(
        out.contains("        && body.guests != 4000000 else RoomFull { capacity: 1 }\n"),
        "{out}"
    );
}

fn wide_example(call: &str) -> String {
    format!(
        "{HEAD}service S at \"/s\" {{\n  post \"/x\" hold(body: Req) -> Hold | RoomFull\n}}\nexample \"e\" {{\n  {call}\n}}\n"
    )
}

#[test]
fn an_example_call_that_fits_is_one_line() {
    let out = f(&wide_example(
        "hold(body: { roomId: \"r\", guests: 4 }) -> ok",
    ));
    assert!(
        out.contains("  hold(body: { roomId: \"r\", guests: 4 }) -> ok\n"),
        "{out}"
    );
}

#[test]
fn a_wide_example_call_has_one_argument_per_line_and_a_nested_record_breaks() {
    let call = "hold(body: { roomId: \"00000000-0000-4000-8000-000000000001\", note: \"a long note that makes the line wide\", guests: 4 }) -> ok";
    let out = f(&wide_example(call));
    let want = "  hold(\n    body: {\n      roomId: \"00000000-0000-4000-8000-000000000001\",\n      note: \"a long note that makes the line wide\",\n      guests: 4\n    }\n  ) -> ok\n";
    assert!(out.contains(want), "{out}");
}

#[test]
fn a_wide_outcome_record_hangs_on_the_call() {
    let x = "x".repeat(70);
    let call = format!("hold(body: {{ roomId: \"r\", guests: 4 }}) -> Hold {{ id: \"{x}\" }}");
    let out = f(&wide_example(&call));
    let want = format!(
        "  hold(body: {{ roomId: \"r\", guests: 4 }}) -> Hold {{\n    id: \"{x}\"\n  }}\n}}\n"
    );
    assert!(out.contains(&want), "{out}");
}

#[test]
fn a_saga_step_with_a_wide_call_breaks_its_arguments() {
    let src = "model Req { roomId: uuid, note?: string, guests: int }\nmodel Hold { id: uuid }\nerror RoomFull 409 { capacity: int }\nservice Rooms at \"/rooms\" {\n  @idempotent\n  post \"/hold\" hold(body: Req) -> Hold | RoomFull\n  @idempotent\n  post \"/release/{id}\" release(id: uuid) -> 204\n}\nservice Checkout at \"/checkout\" {\n  saga post \"/\" checkout(body: Req) -> Hold | RoomFull {\n    step hold = Rooms.hold(body: { roomId: body.roomId, note: \"a long note that makes the line wide\", guests: 4 }) compensate Rooms.release(id: hold.id)\n  }\n}\n";
    let out = f(src);
    let want = "    step hold = Rooms.hold(\n      body: { roomId: body.roomId, note: \"a long note that makes the line wide\", guests: 4 }\n    )\n      compensate Rooms.release(id: hold.id)\n";
    assert!(out.contains(want), "{out}");
}

#[test]
fn comments_around_a_wrapped_line_are_kept() {
    let src = "model M {\n  alpha: int\n  // before\n  rule long: alpha > 1000000 && alpha > 2000000 && alpha > 3000000 && alpha > 4000000 && alpha < 9999999 // after\n}";
    let out = f(src);
    assert!(out.contains("// before"), "{out}");
    assert!(out.contains("// after"), "{out}");
}

#[test]
fn a_wide_given_record_of_a_saga_example_breaks_one_member_per_line() {
    let src = "model Req { roomId: uuid }\nmodel Hold { id: uuid }\nerror RoomFull 409 { capacity: int }\nservice Rooms at \"/rooms\" {\n  @idempotent\n  post \"/hold\" hold(body: Req) -> Hold | RoomFull\n}\nservice Checkout at \"/checkout\" {\n  saga post \"/\" checkout(body: Req) -> Hold | RoomFull {\n    step hold = Rooms.hold(body: { roomId: body.roomId })\n  }\n}\nexample \"e\" {\n  given step hold = RoomFull { capacity: 2222222222222222, capacity: 2222222222222222, capacity: 2222222222222222 }\n  key \"k\"\n  checkout(body: { roomId: \"r\" }) -> RoomFull { capacity: 1 }\n}\n";
    let out = f(src);
    let want = "  given step hold = RoomFull {\n    capacity: 2222222222222222,\n    capacity: 2222222222222222,\n    capacity: 2222222222222222\n  }\n  key \"k\"\n";
    assert!(out.contains(want), "{out}");
}

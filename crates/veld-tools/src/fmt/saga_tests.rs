//! Formatter behaviour for sagas (`spec/platform.md` sections 17 and 18): the block of a `saga
//! post` endpoint and the saga lines of an example.

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
service Checkout at "/checkout" {
  /// Books a room.
  @idempotent(header:"X-Key")
  saga   post "/" checkout(body: Req)->Booking|RoomFull{
    step hold=Rooms.hold(body:{roomId:body.roomId})   compensate   Rooms.release(id:hold.id)
    step charge = Payments.charge(body: body)
    step confirm = Bookings.confirm( id : hold.id )
    timeout   minutes( 5 )
  }
}
"#;

#[test]
fn a_saga_is_printed_in_the_canonical_style() {
    let out = stable(MESSY);
    let want = r#"service Checkout at "/checkout" {
  /// Books a room.
  @idempotent(header: "X-Key")
  saga post "/" checkout(body: Req) -> Booking | RoomFull {
    step hold = Rooms.hold(body: { roomId: body.roomId }) compensate Rooms.release(id: hold.id)
    step charge = Payments.charge(body: body)
    step confirm = Bookings.confirm(id: hold.id)
    timeout minutes(5)
  }
}
"#;
    assert_eq!(out, want);
}

#[test]
fn a_long_step_puts_its_compensation_on_the_next_line() {
    let src = "service S at \"/s\" {\n saga post \"/\" go(body: Req) -> Booking {\n  step reserveTheWholeEvening = Reservations.reserveTheWholeEvening(body: { roomId: body.roomId, guests: body.guests }) compensate Reservations.cancelTheWholeEvening(id: reserveTheWholeEvening.id)\n  timeout seconds(5)\n }\n}\n";
    let out = stable(src);
    assert!(
        out.contains(
            "    step reserveTheWholeEvening = Reservations.reserveTheWholeEvening(body: { roomId: body.roomId, guests: body.guests })\n      compensate Reservations.cancelTheWholeEvening(id: reserveTheWholeEvening.id)\n"
        ),
        "{out}"
    );
}

#[test]
fn objects_nest_and_an_empty_one_is_two_braces() {
    let src = "service S at \"/s\" {\n saga post \"/\" go(body: Req) -> Booking {\n  step a = T.f(x: {}, y: { p: { q: 1 } })\n  timeout seconds(5)\n }\n}\n";
    let out = stable(src);
    assert!(
        out.contains("    step a = T.f(x: {}, y: { p: { q: 1 } })\n"),
        "{out}"
    );
}

#[test]
fn comments_stay_with_their_lines() {
    let src = r#"service S at "/s" {
  saga post "/" go(body: Req) -> Booking { // the header
    // the first step
    step a = T.f(x: body) // trailing a
    // the timeout
    timeout seconds(5)
    // the tail
  }
}
"#;
    let out = stable(src);
    assert_eq!(out, src);
}

#[test]
fn an_empty_saga_is_not_printed_as_one_line() {
    // It does not parse into a valid saga (E1203 comes from the compiler, not the parser), but the
    // parser accepts the block and the formatter keeps it.
    let src = "service S at \"/s\" {\n  saga post \"/\" go(body: Req) -> Booking {}\n}\n";
    let out = stable(src);
    assert_eq!(out, src);
}

#[test]
fn a_statement_of_an_endpoint_block_in_a_saga_is_refused() {
    let src = "service S at \"/s\" {\n saga post \"/\" go(body: Req) -> Booking {\n  require true\n  timeout seconds(5)\n }\n}\n";
    assert!(f(src).starts_with("ERR"), "{}", f(src));
}

const EXAMPLE: &str = r#"
example "a declined payment releases the room"{
   given   step hold = { id : "h-1" }
  given step charge=PaymentDeclined{reason:"funds"}
  given undo hold = unavailable
  given undo charge = ok
  key "k-1"
  checkout(body:{roomId:"r"}) -> PaymentDeclined
  expect   compensated hold,  charge
}
example "stuck" {
  given step a = unavailable
  go() -> SagaFailed { step: "a", compensated: false }
  expect stuck
}
"#;

#[test]
fn the_saga_lines_of_an_example_are_printed_in_order() {
    let out = stable(EXAMPLE);
    let want = r#"example "a declined payment releases the room" {
  given step hold = { id: "h-1" }
  given step charge = PaymentDeclined { reason: "funds" }
  given undo hold = unavailable
  given undo charge = ok
  key "k-1"
  checkout(body: { roomId: "r" }) -> PaymentDeclined
  expect compensated hold, charge
}

example "stuck" {
  given step a = unavailable
  go() -> SagaFailed { step: "a", compensated: false }
  expect stuck
}
"#;
    assert_eq!(out, want);
}

#[test]
fn fact_givens_come_before_saga_givens() {
    let out = stable(
        "example \"e\" {\n  given step a = unavailable\n  given room = absent\n  go() -> ok\n}\n",
    );
    assert_eq!(
        out,
        "example \"e\" {\n  given room = absent\n  given step a = unavailable\n  go() -> ok\n}\n"
    );
}

#[test]
fn comments_around_saga_lines_stay_with_them() {
    let src = r#"example "c" {
  // the step
  given step a = unavailable // no answer
  // the undo
  given undo a = ok
  go() -> ok
  // the check
  expect stuck
}
"#;
    assert_eq!(stable(src), src);
}

#[test]
fn the_words_stay_ordinary_names_when_printed() {
    let src = "model saga { step: int, timeout: int }\nservice step at \"/s\" {\n  get \"/a\" saga(@query timeout: int) -> saga\n}\nexample \"e\" {\n  given step = absent\n  saga(timeout: 1) -> ok\n}\n";
    let out = stable(src);
    assert!(
        out.contains("get \"/a\" saga(@query timeout: int) -> saga"),
        "{out}"
    );
    assert!(out.contains("given step = absent"), "{out}");
}

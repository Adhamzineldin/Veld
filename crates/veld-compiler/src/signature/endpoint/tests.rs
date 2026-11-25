//! Ids of endpoint rules (`spec/rules.md` section 9.6).

use super::*;
use crate::testing::ir;

const HEAD: &str = r#"
model Room { id: uuid, capacity: int, closed: bool }
model NewBooking { roomId: uuid, guests: int }
model Booking { id: uuid, n: int }
error RoomFull 409 { capacity: int }
error RoomClosed 409 { }
"#;

fn compile(body: &str) -> veld_ir::Ir {
    ir(&format!(
        "{HEAD}\nservice Bookings at \"/b\" {{\n  post \"/\" create(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {{\n{body}\n  }}\n}}\n"
    ))
}

fn requires(body: &str) -> Vec<String> {
    compile(body).services[0].endpoints[0]
        .rules
        .requires
        .iter()
        .map(|r| r.id.clone())
        .collect()
}

fn ensures(body: &str) -> Vec<String> {
    compile(body).services[0].endpoints[0]
        .rules
        .ensures
        .iter()
        .map(|r| r.id.clone())
        .collect()
}

const FACT: &str = "    fact room: Room = body.roomId\n";

#[test]
fn the_id_has_the_documented_form() {
    let id = &requires("    require guestsPositive: body.guests > 0")[0];
    assert!(
        id.starts_with("veld:app/Bookings.create.guestsPositive@"),
        "{id}"
    );
    assert_eq!(id.len(), "veld:app/Bookings.create.guestsPositive@".len() + 16);
}

#[test]
fn an_unnamed_statement_uses_its_kind_as_the_name() {
    assert!(requires("    require body.guests > 0")[0].starts_with("veld:app/Bookings.create.require@"));
    assert!(ensures("    ensure result.n > 0")[0].starts_with("veld:app/Bookings.create.ensure@"));
}

#[test]
fn the_hashed_bytes_are_the_canonical_shape() {
    let ir = compile("    require g: body.guests > 0");
    let r = &ir.services[0].endpoints[0].rules.requires[0];
    let at = EndpointRef {
        package: "app",
        module: "app",
        service: "Bookings",
        endpoint: "create",
    };
    assert_eq!(
        endpoint_rule_bytes(&ir.decls, &at, StatementKind::Require, Some("g"), &r.expr),
        r#"{"d":"app:app.Bookings.create","e":{"e":{"bin":{"l":{"e":{"path":{"root":{"param":"body"},"steps":[{"w":"guests"}]}},"t":"int"},"op":"gt","r":{"e":{"lit":{"int":"0"}},"t":"int"}}},"t":"bool"},"k":"require","n":"g"}"#
    );
}

#[test]
fn a_fact_appears_by_name_in_the_shape() {
    let ir = compile(&format!("{FACT}    require r: room.capacity > 0"));
    let r = &ir.services[0].endpoints[0].rules.requires[0];
    let at = EndpointRef {
        package: "app",
        module: "app",
        service: "Bookings",
        endpoint: "create",
    };
    let bytes = endpoint_rule_bytes(&ir.decls, &at, StatementKind::Require, Some("r"), &r.expr);
    assert!(bytes.contains(r#""root":{"fact":"room"}"#), "{bytes}");
}

#[test]
fn the_message_and_the_else_part_do_not_change_the_id() {
    let plain = requires(&format!("{FACT}    require r: body.guests <= room.capacity"));
    let described = requires(&format!(
        "{FACT}    require r: body.guests <= room.capacity message \"too many\""
    ));
    let with_else = requires(&format!(
        "{FACT}    require r: body.guests <= room.capacity else RoomFull {{ capacity: room.capacity }}"
    ));
    let other_else = requires(&format!(
        "{FACT}    require r: body.guests <= room.capacity else RoomClosed"
    ));
    assert_eq!(plain, described);
    assert_eq!(plain, with_else);
    assert_eq!(plain, other_else);
}

#[test]
fn the_expression_changes_the_id() {
    let a = requires("    require r: body.guests > 0");
    let b = requires("    require r: body.guests > 1");
    assert_ne!(a, b);
}

#[test]
fn the_name_and_the_kind_change_the_id() {
    let a = requires("    require r: body.guests > 0");
    let b = requires("    require s: body.guests > 0");
    assert_ne!(a, b);
    let unnamed = requires("    require body.guests > 0");
    assert_ne!(a, unnamed);
    let e = ensures("    ensure r: body.guests > 0");
    assert_ne!(a[0].split('@').nth(1), e[0].split('@').nth(1));
}

#[test]
fn a_parameter_is_part_of_the_rule_by_its_name() {
    let a = ir(
        "model N { v: int }\nservice S at \"/\" {\n  get \"/\" f(@query v: int) -> 200 N { require r: v > 0 }\n}\n",
    );
    let b = ir(
        "model N { v: int }\nservice S at \"/\" {\n  get \"/\" f(@query v: int, @query other: int) -> 200 N { require r: v > 0 }\n}\n",
    );
    // The same statement on an endpoint with another parameter list keeps its id: the shape
    // names only what the expression reads.
    let id = |ir: &veld_ir::Ir| ir.services[0].endpoints[0].rules.requires[0].id.clone();
    assert_eq!(id(&a), id(&b));
    let c = ir(
        "model N { v: int }\nservice S at \"/\" {\n  get \"/\" f(@query w: int) -> 200 N { require r: w > 0 }\n}\n",
    );
    assert_ne!(id(&a), id(&c));
}

#[test]
fn another_endpoint_or_service_has_another_id() {
    let one = ir(
        "model N { v: int }\nservice S at \"/\" {\n  post \"/\" f(body: N) -> 201 N { require r: body.v > 0 }\n}\n",
    );
    let two = ir(
        "model N { v: int }\nservice S at \"/\" {\n  post \"/\" g(body: N) -> 201 N { require r: body.v > 0 }\n}\n",
    );
    let id = |ir: &veld_ir::Ir| ir.services[0].endpoints[0].rules.requires[0].id.clone();
    assert_ne!(id(&one), id(&two));
}

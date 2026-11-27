//! Examples: resolution, givens and values (`spec/rules.md` section 12).

use crate::testing::{codes, ir, multi, one};
use serde_json::json;
use veld_ir::Ir;
use veld_ir::endpoint_rules::{ExampleOutcome, ExampleValue};

const HEAD: &str = r#"
enum Status { pending, paid = "PAID_V" }
model Room { id: uuid, capacity: int, closed: bool, owner?: string, price: decimal, nights: long, ratio: float, tags: string[], meta: json, status: Status, extras: Map<string, int> }
model NewBooking { roomId: uuid, guests: int, start: datetime, day: date }
model Booking { id: uuid, status: Status }
error RoomFull 409 { capacity: int }

service Bookings at "/b" {
  post "/" create(body: NewBooking) -> 201 Booking | RoomFull {
    fact room: Room = body.roomId
    fact other?: Room
    require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
  }
  get "/{id}" find(id: uuid) -> 200 Booking
  delete "/{id}" remove(id: uuid) -> 204
}
"#;

fn with(examples: &str) -> String {
    format!("{HEAD}\n{examples}")
}

fn codes_of(examples: &str) -> Vec<&'static str> {
    codes(&with(examples))
}

fn compile(examples: &str) -> Ir {
    ir(&with(examples))
}

#[test]
fn every_value_form_lowers_to_its_wire_form() {
    let ir = compile(
        r#"
example "all the forms" {
  given room = {
    id: "00000000-0000-4000-8000-0000000000AA", capacity: 2, closed: false, price: 12.50,
    nights: 9007199254740993, ratio: 1, tags: ["a", "b"], meta: { k: [1, "x", true, 2.5] },
    status: .paid, extras: { one: 1 }
  }
  at "2026-03-01T09:00:00Z"
  create(body: { roomId: room.id, guests: 3, start: "2026-03-02T00:00:00+02:00", day: "2026-03-02" })
    returns { id: room.id, status: .pending }
    -> RoomFull { capacity: 2 }
}
"#,
    );
    let ex = &ir.examples[0];
    assert_eq!(ex.name, "all the forms");
    assert_eq!(ex.endpoint, "create");
    assert_eq!(ex.now.as_deref(), Some("2026-03-01T09:00:00Z"));
    let given = serde_json::to_value(ex.givens[0].value.as_ref().expect("value")).expect("json");
    assert_eq!(
        given,
        json!({"object": [
            {"name": "id", "wire": "id", "value": {"json": "00000000-0000-4000-8000-0000000000aa"}},
            {"name": "capacity", "wire": "capacity", "value": {"json": 2}},
            {"name": "closed", "wire": "closed", "value": {"json": false}},
            {"name": "price", "wire": "price", "value": {"json": "12.50"}},
            {"name": "nights", "wire": "nights", "value": {"json": "9007199254740993"}},
            {"name": "ratio", "wire": "ratio", "value": {"json": 1.0}},
            {"name": "tags", "wire": "tags", "value": {"list": [{"json": "a"}, {"json": "b"}]}},
            {"name": "meta", "wire": "meta", "value": {"json": {"k": [1, "x", true, 2.5]}}},
            {"name": "status", "wire": "status", "value": {"json": "PAID_V"}},
            {"name": "extras", "wire": "extras", "value": {"object": [
                {"name": "one", "wire": "one", "value": {"json": 1}}]}}
        ]})
    );
    let arg = serde_json::to_value(&ex.args[0].value).expect("json");
    assert_eq!(
        arg["object"][0],
        json!({"name": "roomId", "wire": "roomId", "value": {"ref": {"given": "room", "path": ["id"]}}})
    );
    assert!(matches!(ex.outcome, Some(ExampleOutcome::Error(_))));
    assert!(ex.returns.is_some());
}

#[test]
fn an_ok_outcome_and_an_error_outcome_with_and_without_a_pattern() {
    let ir = compile(
        r#"
example "ok with a pattern" { find(id: "00000000-0000-4000-8000-000000000001") -> ok { status: .pending } }
example "ok" { remove(id: "00000000-0000-4000-8000-000000000001") -> ok }
example "implicit" { given room = absent
  create(body: { guests: 1 }) -> NotFound }
"#,
    );
    let by = |n: &str| ir.examples.iter().find(|e| e.name == n).expect("example");
    assert!(matches!(
        &by("ok with a pattern").outcome,
        Some(ExampleOutcome::Ok(Some(_)))
    ));
    assert!(matches!(&by("ok").outcome, Some(ExampleOutcome::Ok(None))));
    let implicit = by("implicit");
    assert_eq!(implicit.givens[0].value, None);
    let Some(ExampleOutcome::Error(e)) = &implicit.outcome else {
        panic!()
    };
    assert_eq!(ir.decls[e.decl.index()].name, "NotFound");
}

#[test]
fn left_out_values_are_not_an_error() {
    let src = r#"
example "nothing given" { given room = {}
  create() -> ok }
"#;
    assert!(codes_of(src).is_empty(), "{:?}", codes_of(src));
}

#[test]
fn examples_are_sorted_by_package_and_name() {
    let ir = compile("example \"b\" { remove() -> ok }\nexample \"a\" { remove() -> ok }");
    let names: Vec<&str> = ir.examples.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    assert_eq!(ir.examples[0].service.expect("a call").index(), 0);
}

#[test]
fn a_contract_without_examples_has_no_examples_member() {
    let ir = ir(HEAD);
    assert!(ir.examples.is_empty());
    assert!(
        !serde_json::to_string(&ir)
            .expect("json")
            .contains("examples")
    );
}

// ---- E0950 -------------------------------------------------------------------------

#[test]
fn a_duplicate_name_is_e0950_also_across_files() {
    assert_eq!(
        codes_of("example \"x\" { remove() -> ok }\nexample \"x\" { remove() -> ok }"),
        ["E0950"]
    );
    let out = multi(&[
        (
            "app",
            "a.veld",
            "service S at \"/\" { get \"/\" f() }\nexample \"x\" { f() -> ok }",
        ),
        ("app", "b.veld", "example \"x\" { f() -> ok }"),
    ]);
    assert_eq!(
        out.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["E0950"]
    );
}

#[test]
fn the_same_name_in_another_package_is_fine() {
    let out = multi(&[
        (
            "a",
            "a.veld",
            "service S at \"/\" { get \"/\" f() }\nexample \"x\" { f() -> ok }",
        ),
        (
            "b",
            "b.veld",
            "service S at \"/\" { get \"/\" f() }\nexample \"x\" { f() -> ok }",
        ),
    ]);
    assert!(out.ir.is_some(), "{}", out.render());
}

#[test]
fn an_unknown_endpoint_is_e0950_with_a_suggestion() {
    let out = one(&with("example \"x\" { creat() -> ok }"));
    assert_eq!(
        out.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["E0950"]
    );
    assert!(
        out.render().contains("did you mean `create`"),
        "{}",
        out.render()
    );
}

#[test]
fn a_name_two_services_share_is_ambiguous_until_qualified() {
    let two = |call: &str| {
        format!(
            "service A at \"/a\" {{ get \"/\" f() }}\nservice B at \"/b\" {{ get \"/\" f() }}\nexample \"x\" {{ {call} -> ok }}"
        )
    };
    assert_eq!(codes(&two("f()")), ["E0950"]);
    assert!(codes(&two("A.f()")).is_empty());
    assert_eq!(codes(&two("C.f()")), ["E0950"]);
    let ir = ir(&two("B.f()"));
    assert_eq!(ir.examples[0].service.expect("a call").index(), 1);
}

// ---- E0951 -------------------------------------------------------------------------

#[test]
fn a_given_must_be_a_fact_of_the_endpoint() {
    assert_eq!(
        codes_of("example \"x\" { given rooom = absent\n create() -> ok }"),
        ["E0951"]
    );
    assert_eq!(
        codes_of("example \"x\" { given room = absent\n find() -> ok }"),
        ["E0951"]
    );
    assert_eq!(
        codes_of("example \"x\" { given room = absent\n given room = absent\n create() -> ok }"),
        ["E0951"]
    );
}

// ---- E0952 -------------------------------------------------------------------------

#[test]
fn a_value_must_fit_its_type() {
    for bad in [
        "create(body: { guests: \"three\" })",
        "create(body: { guests: 3000000000 })",
        "create(body: { guests: 1.5 })",
        "create(body: { roomId: \"not-a-uuid\" })",
        "create(body: { day: \"2026-13-01\" })",
        "create(body: { start: \"yesterday\" })",
        "create(body: { unknown: 1 })",
        "create(body: { guests: 1, guests: 2 })",
        "create(body: [1])",
        "create(body: 7)",
        "create(body: { guests: [1] })",
        "create(nope: 1)",
        "create(body: {}, body: {})",
        "find(id: .pending)",
        "create(body: { start: .pending })",
        "create(body: { roomId: other.id })",
    ] {
        assert_eq!(
            codes_of(&format!("example \"x\" {{ {bad} -> ok }}")),
            ["E0952"],
            "{bad}"
        );
    }
}

#[test]
fn enum_values_must_name_a_variant() {
    let src = "example \"x\" { given room = { status: .paidd }\n create() -> ok }";
    assert_eq!(codes_of(src), ["E0952"]);
}

#[test]
fn a_read_must_name_an_earlier_given_with_a_value_and_a_field() {
    let given = "given room = absent\n";
    assert_eq!(
        codes_of(&format!(
            "example \"x\" {{ {given} create(body: {{ roomId: room.id }}) -> ok }}"
        )),
        ["E0952"],
        "an absent given has no fields"
    );
    let with_room = "given room = { id: \"00000000-0000-4000-8000-000000000001\" }\n";
    assert_eq!(
        codes_of(&format!(
            "example \"x\" {{ {with_room} create(body: {{ roomId: room.idd }}) -> ok }}"
        )),
        ["E0952"]
    );
    assert_eq!(
        codes_of(&format!(
            "example \"x\" {{ {with_room} create(body: {{ guests: room.id }}) -> ok }}"
        )),
        ["E0952"],
        "uuid is not an int"
    );
    assert!(
        codes_of(&format!(
            "example \"x\" {{ {with_room} create(body: {{ guests: room.capacity }}) -> ok }}"
        ))
        .is_empty()
    );
}

#[test]
fn returns_needs_a_success_body() {
    assert_eq!(
        codes_of(
            "example \"x\" { remove() returns { id: \"00000000-0000-4000-8000-000000000001\" } -> ok }"
        ),
        ["E0952"]
    );
    assert!(codes_of("example \"x\" { find() returns { status: .paid } -> ok }").is_empty());
}

#[test]
fn an_outcome_must_be_an_error_the_endpoint_can_answer_with() {
    assert_eq!(codes_of("example \"x\" { find() -> RoomFull }"), ["E0952"]);
    assert_eq!(codes_of("example \"x\" { find() -> Booking }"), ["E0952"]);
    assert_eq!(codes_of("example \"x\" { find() -> Nope }"), ["E0205"]);
    assert!(codes_of("example \"x\" { find() -> Forbidden }").is_empty());
    assert!(
        codes_of("example \"x\" { find() -> InvalidTransition { field: \"status\" } }").is_empty()
    );
    assert_eq!(
        codes_of("example \"x\" { remove() -> ok { id: \"x\" } }"),
        ["E0952"],
        "no success body to match"
    );
}

#[test]
fn a_principal_needs_a_declared_principal() {
    assert_eq!(
        codes_of("example \"x\" { as 1\n remove() -> ok }"),
        ["E0952"]
    );
}

#[test]
fn the_clock_is_a_date_and_time() {
    assert!(
        codes_of("example \"x\" { at \"2026-03-01T09:00:00.5+02:00\"\n remove() -> ok }")
            .is_empty()
    );
    assert_eq!(
        codes_of("example \"x\" { at \"tomorrow\"\n remove() -> ok }"),
        ["E0952"]
    );
}

#[test]
fn every_error_of_one_example_is_reported() {
    let out = one(&with(
        "example \"x\" { given zzz = absent\n create(nope: 1) -> Nope }",
    ));
    let mut found: Vec<&str> = out.diagnostics.iter().map(|d| d.code).collect();
    found.sort_unstable();
    assert_eq!(found, ["E0205", "E0951", "E0952"]);
}

#[test]
fn union_and_page_values_follow_the_wire_form() {
    let src = r#"
model Card { number: string }
model Bank { iban: string }
union Payment { card: Card, bank: Bank }
model Pay { method: Payment, history: Page<Card> }
service P at "/p" { post "/" pay(body: Pay) -> 201 Pay }
example "union" { pay(body: { method: { kind: "card", number: "4" }, history: { items: [{ number: "1" }], total: 1 } }) -> ok }
"#;
    let ir = ir(src);
    let arg = serde_json::to_value(&ir.examples[0].args[0].value).expect("json");
    assert_eq!(
        arg["object"][0]["value"]["object"][0]["value"],
        json!({"json": "card"})
    );
    assert_eq!(
        codes(&src.replace("kind: \"card\", ", "")),
        ["E0952"],
        "a union value needs its discriminator"
    );
    assert_eq!(
        codes(&src.replace("kind: \"card\"", "kind: \"cash\"")),
        ["E0952"]
    );
}

#[test]
fn a_json_value_holds_literals_only() {
    let src = "example \"x\" { given room = { meta: { a: .pending } }\n create() -> ok }";
    assert_eq!(codes_of(src), ["E0952"]);
}

#[test]
fn the_ir_form_round_trips() {
    let ir = compile(
        "example \"x\" { given room = { capacity: 1 }\n create(body: { guests: room.capacity }) -> ok }",
    );
    let json = serde_json::to_string(&ir).expect("json");
    let back: Ir = serde_json::from_str(&json).expect("reads back");
    assert_eq!(ir, back);
    let value: &ExampleValue = &ir.examples[0].args[0].value;
    assert!(matches!(value, ExampleValue::Object(_)));
}

//! `@version` and `@idempotent` (`spec/rules.md` sections 11.7 to 11.9): where they may stand,
//! what they lower to, and what a contract without them keeps.

use crate::testing::{codes, ir as build};
use veld_ir::{DeclKind, Feature, Ir};

const ROOM: &str = "model Room {\n  id: uuid\n  name: string\n  @version\n  revision: long\n}\n\
model Rename { name: string }\n\
error RoomFull 409 { capacity: int }\n";

fn rooms(endpoints: &str) -> String {
    format!("{ROOM}service Rooms at \"/rooms\" {{\n{endpoints}\n}}\n")
}

const GUARDED: &str = "  put \"/{id}\" rename(id: uuid, body: Rename) -> Room | NotFound {\n    fact room: Room = id\n  }";

fn find<'a>(ir: &'a Ir, name: &str) -> &'a veld_ir::Endpoint {
    match ir.services[0].endpoints.iter().find(|e| e.name == name) {
        Some(e) => e,
        None => panic!("no endpoint {name}"),
    }
}

fn error_names(ir: &Ir, ep: &veld_ir::Endpoint) -> Vec<String> {
    ep.errors
        .iter()
        .map(|e| format!("{} {}", e.status, ir.decls[e.decl.index()].name))
        .collect()
}

// ---- @version ---------------------------------------------------------------------------------

#[test]
fn a_version_field_is_marked_in_the_ir_and_only_that_one() {
    let ir = build(&rooms(GUARDED));
    let room = ir
        .decls
        .iter()
        .find(|d| d.name == "Room")
        .map(|d| d.fields());
    let flags: Vec<(&str, bool)> = room
        .unwrap_or_default()
        .iter()
        .map(|f| (f.name.as_str(), f.version))
        .collect();
    assert_eq!(flags, [("id", false), ("name", false), ("revision", true)]);
    let json = serde_json::to_value(&ir).unwrap_or_default();
    let fields = &json["decls"][1]["fields"];
    let versions = fields
        .as_array()
        .map(|a| a.iter().filter(|f| f.get("version").is_some()).count());
    assert_eq!(versions, Some(1), "{json}");
}

#[test]
fn the_first_fact_of_a_model_with_a_version_is_the_guard() {
    let ir = build(&rooms(GUARDED));
    let v = &find(&ir, "rename").rules.version;
    let v = v.as_ref();
    assert_eq!(v.map(|v| v.fact.as_str()), Some("room"));
    assert_eq!(v.map(|v| v.field.as_str()), Some("revision"));
    assert_eq!(v.map(|v| v.wire.as_str()), Some("revision"));
    let id = v.map(|v| v.id.as_str()).unwrap_or_default();
    assert!(id.starts_with("veld:app/Rooms.rename.version@"), "{id}");
    assert_eq!(id.len(), "veld:app/Rooms.rename.version@".len() + 16);
}

#[test]
fn a_fact_before_the_versioned_one_that_has_no_version_is_skipped() {
    let src = rooms(
        "  put \"/{id}\" rename(id: uuid, @query other: uuid) -> Room {\n    fact first: Rename = other\n    fact room: Room = id\n    fact second: Room = id\n  }",
    );
    let ir = build(&src);
    let v = find(&ir, "rename").rules.version.as_ref();
    assert_eq!(v.map(|v| v.fact.as_str()), Some("room"));
}

#[test]
fn a_wire_name_is_used_for_the_path_of_the_guard() {
    let src = "model Room {\n  id: uuid\n  @wire(\"rev\")\n  @version\n  revision: int\n}\n\
        service Rooms at \"/rooms\" {\n  get \"/{id}\" one(id: uuid) -> Room {\n    fact room: Room = id\n  }\n}\n";
    let ir = build(src);
    let v = find(&ir, "one").rules.version.as_ref();
    assert_eq!(v.map(|v| v.field.as_str()), Some("revision"));
    assert_eq!(v.map(|v| v.wire.as_str()), Some("rev"));
}

#[test]
fn an_inherited_version_field_is_the_version_of_the_child() {
    let src = "model Base {\n  @version\n  revision: long\n}\n\
        model Room extends Base { id: uuid }\n\
        service Rooms at \"/rooms\" {\n  get \"/{id}\" one(id: uuid) -> Room {\n    fact room: Room = id\n  }\n}\n";
    let ir = build(src);
    assert_eq!(
        find(&ir, "one")
            .rules
            .version
            .as_ref()
            .map(|v| v.field.as_str()),
        Some("revision")
    );
}

#[test]
fn the_guard_declares_precondition_failed_and_validation_error() {
    let ir = build(&rooms(GUARDED));
    let names = error_names(&ir, find(&ir, "rename"));
    assert!(
        names.contains(&"412 PreconditionFailed".to_string()),
        "{names:?}"
    );
    assert!(
        names.contains(&"422 ValidationError".to_string()),
        "{names:?}"
    );
    let decl = ir.decls.iter().find(|d| d.name == "PreconditionFailed");
    let Some(DeclKind::Error(e)) = decl.map(|d| &d.kind) else {
        panic!("PreconditionFailed is not in the IR");
    };
    assert_eq!(e.default_status, Some(412));
    let fields: Vec<&str> = e.fields.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(fields, ["expected", "actual"]);
}

#[test]
fn a_guard_uses_rules_version_and_a_plain_etag_does_too() {
    let guarded = build(&rooms(GUARDED));
    let used = guarded.features_used(&[veld_ir::ServiceId(0)]);
    assert!(used.contains(&Feature::RulesVersion), "{used:?}");
    assert!(used.contains(&Feature::RulesEndpoint), "{used:?}");
    assert!(!used.contains(&Feature::Idempotency), "{used:?}");
    // An endpoint with no block that answers with the model owes the ETag, and nothing else.
    let plain = build(&rooms("  get \"/{id}\" one(id: uuid) -> Room"));
    let used = plain.features_used(&[veld_ir::ServiceId(0)]);
    assert!(used.contains(&Feature::RulesVersion), "{used:?}");
    assert!(!used.contains(&Feature::RulesEndpoint), "{used:?}");
    let ep = find(&plain, "one");
    assert!(ep.rules.version.is_none());
    assert_eq!(
        plain.etag_field(ep).map(|f| f.name.as_str()),
        Some("revision")
    );
    assert!(
        !error_names(&plain, ep)
            .iter()
            .any(|e| e.contains("Precondition"))
    );
}

#[test]
fn a_model_in_a_list_or_a_body_owes_no_etag() {
    let src = rooms(
        "  get \"/\" all() -> Room[]\n  post \"/\" make(body: Room) -> 201 RoomFull | Rename",
    );
    let src = src.replace("-> 201 RoomFull | Rename", "-> 204");
    let ir = build(&src);
    let used = ir.features_used(&[veld_ir::ServiceId(0)]);
    assert!(!used.contains(&Feature::RulesVersion), "{used:?}");
}

#[test]
fn a_contract_without_the_annotations_keeps_its_ir() {
    let src = "model Room { id: uuid\n revision: long }\n\
        service Rooms at \"/rooms\" {\n  get \"/{id}\" one(id: uuid) -> Room {\n    fact room: Room = id\n  }\n}\n";
    let text = serde_json::to_string(&build(src)).unwrap_or_default();
    for key in [
        "\"version\":true",
        "idempotency",
        "PreconditionFailed",
        "if_match",
        "\"store\"",
    ] {
        assert!(!text.contains(key), "{key} in {text}");
    }
}

#[test]
fn the_annotations_change_no_signature() {
    let with = build(&format!(
        "{}service Rooms at \"/rooms\" {{\n  @idempotent\n  put \"/{{id}}\" rename(id: uuid, body: Rename) -> Room | NotFound {{\n    fact room: Room = id\n  }}\n}}\n",
        ROOM
    ));
    let without = build(&format!(
        "{}service Rooms at \"/rooms\" {{\n  put \"/{{id}}\" rename(id: uuid, body: Rename) -> Room | NotFound {{\n    fact room: Room = id\n  }}\n}}\n",
        ROOM.replace("  @version\n", "")
    ));
    let sigs = |ir: &Ir| -> Vec<String> {
        ir.decls
            .iter()
            .filter(|d| ["Room", "Rename", "RoomFull"].contains(&d.name.as_str()))
            .map(|d| format!("{}={}", d.name, d.sig))
            .chain(ir.services[0].endpoints.iter().map(|e| e.sig.clone()))
            .collect()
    };
    assert_eq!(sigs(&with), sigs(&without));
}

// ---- @version: errors ---------------------------------------------------------------------------

#[test]
fn version_on_the_wrong_field_is_e0602() {
    for (field, why) in [
        ("@version\n  name: string", "a string"),
        ("@version\n  n: float", "a float"),
        ("@version\n  n?: int", "an optional field"),
        ("@version(1)\n  n: int", "an argument"),
        ("@version\n  @version\n  n: int", "twice"),
    ] {
        let src =
            format!("model M {{\n  {field}\n}}\nservice S at \"/s\" {{ get \"/\" f() -> M }}\n");
        assert_eq!(codes(&src), ["E0602"], "{why}");
    }
}

#[test]
fn version_on_an_error_or_an_event_field_is_e0602() {
    let on_error = "error E 409 {\n  @version\n  n: int\n}\nmodel M { a: int }\nservice S at \"/s\" { get \"/\" f() -> M | E }\n";
    assert_eq!(codes(on_error), ["E0602"]);
    let on_event = "event Ev {\n  @version\n  n: int\n}\n";
    assert_eq!(codes(on_event), ["E0602"]);
}

#[test]
fn a_second_version_field_is_e0602_inherited_or_not() {
    let own = "model M {\n  @version\n  a: int\n  @version\n  b: long\n}\nservice S at \"/s\" { get \"/\" f() -> M }\n";
    assert_eq!(codes(own), ["E0602"]);
    let inherited = "model P {\n  @version\n  a: int\n}\nmodel C extends P {\n  @version\n  b: long\n}\nservice S at \"/s\" { get \"/\" f() -> C }\n";
    assert_eq!(codes(inherited), ["E0602"]);
    // A parent with two is reported once, not again for each child.
    let parent = "model P {\n  @version\n  a: int\n  @version\n  b: int\n}\nmodel C extends P { c: int }\nservice S at \"/s\" { get \"/\" f() -> C }\n";
    assert_eq!(codes(parent), ["E0602"]);
}

#[test]
fn version_applies_to_a_field_only() {
    let on_model = "@version\nmodel M { a: int }\nservice S at \"/s\" { get \"/\" f() -> M }\n";
    assert_eq!(codes(on_model), ["E0602"]);
    let on_endpoint =
        "model M { a: int }\nservice S at \"/s\" {\n  @version\n  get \"/\" f() -> M\n}\n";
    assert_eq!(codes(on_endpoint), ["E0602"]);
}

// ---- @idempotent ----------------------------------------------------------------------------------

fn idem(annotation: &str) -> String {
    format!(
        "model M {{ a: int }}\nservice S at \"/s\" {{\n  {annotation}\n  post \"/\" make(body: M, @header(\"X-Tenant\") tenant: string) -> 201 M\n}}\n"
    )
}

#[test]
fn idempotent_defaults_and_arguments_are_lowered() {
    let ir = build(&idem("@idempotent"));
    let i = find(&ir, "make").idempotency.as_ref();
    assert_eq!(i.map(|i| i.header.as_str()), Some("Idempotency-Key"));
    assert_eq!(i.map(|i| (i.required, i.caller)), Some((false, false)));
    for (text, header, required) in [
        ("@idempotent(required: true)", "Idempotency-Key", true),
        (
            "@idempotent(header: \"X-Request-Id\")",
            "X-Request-Id",
            false,
        ),
        (
            "@idempotent(required: true, header: \"X-Request-Id\")",
            "X-Request-Id",
            true,
        ),
        ("@idempotent(required: false)", "Idempotency-Key", false),
    ] {
        let ir = build(&idem(text));
        let i = find(&ir, "make").idempotency.as_ref();
        assert_eq!(i.map(|i| i.header.as_str()), Some(header), "{text}");
        assert_eq!(i.map(|i| i.required), Some(required), "{text}");
    }
}

#[test]
fn the_caller_is_part_of_the_scope_when_the_endpoint_has_auth() {
    let src = "model M { a: int }\nservice S at \"/s\" {\n  @auth(bearer)\n  @idempotent\n  post \"/\" make(body: M) -> 201 M\n}\n";
    let ir = build(src);
    assert_eq!(
        find(&ir, "make").idempotency.as_ref().map(|i| i.caller),
        Some(true)
    );
    let from_service = "model M { a: int }\n@auth(bearer)\nservice S at \"/s\" {\n  @idempotent\n  post \"/\" make(body: M) -> 201 M\n  @auth(none)\n  @idempotent\n  post \"/open\" open(body: M) -> 201 M\n}\n";
    let ir = ir_of(from_service);
    assert_eq!(
        find(&ir, "make").idempotency.as_ref().map(|i| i.caller),
        Some(true)
    );
    assert_eq!(
        find(&ir, "open").idempotency.as_ref().map(|i| i.caller),
        Some(false)
    );
}

fn ir_of(src: &str) -> Ir {
    build(src)
}

#[test]
fn idempotent_declares_conflict_and_validation_error() {
    let ir = build(&idem("@idempotent"));
    let names = error_names(&ir, find(&ir, "make"));
    assert!(names.contains(&"409 Conflict".to_string()), "{names:?}");
    assert!(
        names.contains(&"422 ValidationError".to_string()),
        "{names:?}"
    );
    // The prelude already has `Conflict 409 { message: string }`: nothing new is needed.
    let conflict = ir.decls.iter().find(|d| d.name == "Conflict");
    assert!(conflict.is_some());
    assert!(!ir.decls.iter().any(|d| d.name == "PreconditionFailed"));
    let used = ir.features_used(&[veld_ir::ServiceId(0)]);
    assert!(used.contains(&Feature::Idempotency), "{used:?}");
    assert!(!used.contains(&Feature::RulesVersion), "{used:?}");
    assert!(!used.contains(&Feature::RulesEndpoint), "{used:?}");
}

#[test]
fn an_endpoint_that_lists_conflict_itself_is_not_given_a_second_one() {
    let src = "model M { a: int }\nservice S at \"/s\" {\n  @idempotent\n  post \"/\" make(body: M) -> 201 M | Conflict\n}\n";
    let ir = build(src);
    let n = find(&ir, "make")
        .errors
        .iter()
        .filter(|e| e.status == 409)
        .count();
    assert_eq!(n, 1);
}

#[test]
fn idempotent_with_wrong_arguments_is_e0602() {
    for text in [
        "@idempotent(\"k\")",
        "@idempotent(header: 1)",
        "@idempotent(header: \"\")",
        "@idempotent(header: \"Bad Name\")",
        "@idempotent(required: \"yes\")",
        "@idempotent(other: true)",
        "@idempotent(required: true, required: false)",
        "@idempotent(header: \"A\", header: \"B\")",
        "@idempotent(header: \"Authorization\")",
        "@idempotent(header: \"if-match\")",
        "@idempotent(header: \"Veld-Test-Facts\")",
        "@idempotent(header: \"x-tenant\")",
        "@idempotent @idempotent",
    ] {
        assert_eq!(codes(&idem(text)), ["E0602"], "{text}");
    }
}

#[test]
fn idempotent_is_for_endpoints_that_have_one_answer() {
    let stream = "model M { a: int }\nservice S at \"/s\" {\n  @idempotent\n  get \"/\" feed() -> stream M\n}\n";
    assert_eq!(codes(stream), ["E0602"]);
    let ws = "model M { a: int }\nservice S at \"/s\" {\n  @idempotent\n  ws \"/\" chat(body: stream M) -> stream M\n}\n";
    assert_eq!(codes(ws), ["E0602"]);
    let on_field =
        "model M {\n  @idempotent\n  a: int\n}\nservice S at \"/s\" { get \"/\" f() -> M }\n";
    assert_eq!(codes(on_field), ["E0602"]);
    let on_service =
        "model M { a: int }\n@idempotent\nservice S at \"/s\" { get \"/\" f() -> M }\n";
    assert_eq!(codes(on_service), ["E0602"]);
    let key_header = "model M { a: int }\nservice S at \"/s\" {\n  @auth(apiKey(header: \"X-Key\"))\n  @idempotent(header: \"x-key\")\n  post \"/\" make(body: M) -> 201 M\n}\n";
    assert_eq!(codes(key_header), ["E0602"]);
}

// ---- examples -------------------------------------------------------------------------------------

fn examples(example: &str) -> String {
    format!(
        "{ROOM}service Rooms at \"/rooms\" {{\n  @idempotent(required: true)\n  put \"/{{id}}\" rename(id: uuid, body: Rename) -> Room | RoomFull | NotFound {{\n    fact room: Room = id\n  }}\n  get \"/{{id}}\" one(id: uuid) -> Room\n}}\n\n{example}\n"
    )
}

#[test]
fn ifmatch_and_key_lines_are_lowered() {
    let src = examples(
        "example \"stale\" {\n  ifMatch 3\n  key \"k-1\"\n  rename(id: \"00000000-0000-4000-8000-000000000001\", body: { name: \"x\" }) -> PreconditionFailed { expected: 3 }\n}\n\
         example \"any\" {\n  ifMatch any\n  key \"k-2\" stored RoomFull { capacity: 2 }\n  rename(body: { name: \"x\" }) -> RoomFull\n}\n\
         example \"other\" {\n  key \"k-3\" other\n  rename(body: { name: \"x\" }) -> Conflict\n}\n\
         example \"running\" {\n  key \"k-4\" running\n  rename(body: { name: \"x\" }) -> Conflict\n}\n\
         example \"ok\" {\n  key \"k-5\" stored ok { name: \"x\" }\n  rename(body: { name: \"x\" }) -> ok\n}\n\
         example \"none\" {\n  rename(body: { name: \"x\" }) -> ValidationError\n}\n",
    );
    let ir = build(&src);
    let by = |n: &str| match ir.examples.iter().find(|e| e.name == n) {
        Some(e) => e,
        None => panic!("no example {n}"),
    };
    let json = |n: &str| serde_json::to_value(by(n)).unwrap_or_default();
    assert_eq!(
        json("stale")["if_match"],
        serde_json::json!({"version": "3"})
    );
    assert_eq!(json("stale")["key"], serde_json::json!({"value": "k-1"}));
    assert_eq!(json("any")["if_match"], "any");
    assert_eq!(json("any")["key"]["value"], "k-2");
    assert!(json("any")["key"]["store"]["stored"]["error"].is_object());
    assert_eq!(json("other")["key"]["store"], "other");
    assert_eq!(json("running")["key"]["store"], "running");
    assert!(json("ok")["key"]["store"]["stored"].get("ok").is_some());
    assert!(json("none").get("if_match").is_none());
    assert!(json("none").get("key").is_none());
    // The declarations the lines reach are in the IR.
    assert!(ir.decls.iter().any(|d| d.name == "RoomFull"));
}

#[test]
fn an_ifmatch_without_a_guard_and_a_key_without_idempotent_are_e0952() {
    let src = examples(
        "example \"a\" {\n  ifMatch 1\n  one(id: \"00000000-0000-4000-8000-000000000001\") -> ok\n}\n",
    );
    assert_eq!(codes(&src), ["E0952"]);
    let src = examples(
        "example \"b\" {\n  key \"k\"\n  one(id: \"00000000-0000-4000-8000-000000000001\") -> ok\n}\n",
    );
    assert_eq!(codes(&src), ["E0952"]);
}

#[test]
fn a_key_a_request_cannot_carry_is_e0952() {
    for key in ["\"\"", "\"a b\"", "\"tab\\t\"", "\"é\""] {
        let src = examples(&format!(
            "example \"a\" {{\n  key {key}\n  rename(body: {{ name: \"x\" }}) -> ValidationError\n}}\n"
        ));
        assert_eq!(codes(&src), ["E0952"], "{key}");
    }
    let long = "k".repeat(256);
    let src = examples(&format!(
        "example \"a\" {{\n  key \"{long}\"\n  rename(body: {{ name: \"x\" }}) -> ValidationError\n}}\n"
    ));
    assert_eq!(codes(&src), ["E0952"]);
    let edge = "k".repeat(255);
    let src = examples(&format!(
        "example \"a\" {{\n  key \"{edge}\"\n  rename(body: {{ name: \"x\" }}) -> ValidationError\n}}\n"
    ));
    assert!(codes(&src).is_empty());
}

#[test]
fn a_stored_answer_must_be_one_the_endpoint_gives_and_the_store_holds() {
    let internal = examples(
        "example \"a\" {\n  key \"k\" stored Internal\n  rename(body: { name: \"x\" }) -> Internal\n}\n",
    );
    assert_eq!(codes(&internal), ["E0952"]);
    let unknown = examples(
        "example \"a\" {\n  key \"k\" stored TooManyRequests\n  rename(body: { name: \"x\" }) -> Conflict\n}\n",
    );
    assert_eq!(codes(&unknown), ["E0952"]);
    let wrong_field = examples(
        "example \"a\" {\n  key \"k\" stored RoomFull { nope: 1 }\n  rename(body: { name: \"x\" }) -> RoomFull\n}\n",
    );
    assert_eq!(codes(&wrong_field), ["E0952"]);
}

#[test]
fn ifmatch_wants_a_version_or_any() {
    let src =
        examples("example \"a\" {\n  ifMatch \"3\"\n  rename(body: { name: \"x\" }) -> ok\n}\n");
    assert!(!codes(&src).is_empty());
    let neg = examples("example \"a\" {\n  ifMatch -3\n  rename(body: { name: \"x\" }) -> ok\n}\n");
    assert!(!codes(&neg).is_empty());
}

#[test]
fn an_endpoint_that_the_example_cannot_answer_with_precondition_failed_is_e0952() {
    // `one` has no guard, so it never answers 412.
    let src = examples(
        "example \"a\" {\n  one(id: \"00000000-0000-4000-8000-000000000001\") -> PreconditionFailed\n}\n",
    );
    assert_eq!(codes(&src), ["E0952"]);
}

//! Formatter behaviour for `@version`, `@idempotent` and the `ifMatch` and `key` lines of an
//! example (`spec/rules.md` sections 11.7 to 11.9): they survive a round trip, formatting is
//! idempotent, and the words stay names where they are names.

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

#[test]
fn the_field_annotation_stays_on_its_field() {
    let out = stable(
        "model M {\n id: uuid\n revision:long   @version\n  @wire( \"rev\" )  other: int\n}",
    );
    assert!(out.contains("  revision: long @version\n"), "{out}");
    assert!(out.contains("other: int @wire(\"rev\")"), "{out}");
}

#[test]
fn an_idempotent_endpoint_keeps_its_arguments() {
    let src = "service S at \"/s\" {\n  @idempotent\n  post \"/a\" a(body: B) -> 201 B\n  @idempotent( required:true , header:\"X-Id\" )\n  post \"/b\" b(body: B) -> 201 B\n}";
    let out = stable(src);
    assert!(out.contains("  @idempotent\n  post \"/a\""), "{out}");
    assert!(
        out.contains("  @idempotent(required: true, header: \"X-Id\")\n  post \"/b\""),
        "{out}"
    );
}

const EXAMPLES: &str = r#"
example "all the lines" {
  given room = { revision: 4 }
  at "2026-03-01T09:00:00Z"
  ifMatch   3
  key "k-1"   stored   RoomFull { capacity: 2 }
  rename(id: room.id) -> RoomFull { capacity: 2 }
}
example "any" {
  ifMatch   any
  key "k-2" other
  rename() -> ok
}
example "running" {
  key  "k-3"  running
  rename() -> Conflict
}
example "a bare key" {
  key "k-4"
  rename() -> ok
}
example "stored ok" {
  key "k-5" stored ok { name: "x" }
  rename() -> ok { name: "x" }
}
example "stored without a value" {
  key "k-6" stored Full
  rename() -> Full
}
"#;

#[test]
fn the_lines_of_an_example_keep_their_order_and_their_words() {
    let out = stable(EXAMPLES);
    assert!(
        out.contains(
            "  at \"2026-03-01T09:00:00Z\"\n  ifMatch 3\n  key \"k-1\" stored RoomFull { capacity: 2 }\n  rename("
        ),
        "{out}"
    );
    for line in [
        "  ifMatch any\n  key \"k-2\" other\n",
        "  key \"k-3\" running\n",
        "  key \"k-4\"\n  rename()",
        "  key \"k-5\" stored ok { name: \"x\" }\n",
        "  key \"k-6\" stored Full\n  rename() -> Full",
    ] {
        assert!(out.contains(line), "{line:?} in\n{out}");
    }
}

#[test]
fn a_comment_before_a_line_stays_before_it() {
    let out = stable(
        "example \"c\" {\n  // the stale tag\n  ifMatch 3\n  // the key\n  key \"k\"\n  rename() -> ok\n}",
    );
    assert!(
        out.contains("  // the stale tag\n  ifMatch 3\n  // the key\n  key \"k\"\n"),
        "{out}"
    );
}

#[test]
fn the_words_are_still_names_after_formatting() {
    let src = "example \"names\" {\n  key \"k\"\n  stored()   -> ok\n}\nexample \"more\" {\n  key \"k\" other\n  other( a: 1 ) -> ok\n}\nexample \"ifMatch\" {\n  ifMatch( ) -> ok\n}";
    let out = stable(src);
    assert!(out.contains("  key \"k\"\n  stored() -> ok"), "{out}");
    assert!(
        out.contains("  key \"k\" other\n  other(a: 1) -> ok"),
        "{out}"
    );
    assert!(out.contains("  ifMatch() -> ok"), "{out}");
}

#[test]
fn a_service_with_a_guard_and_an_idempotent_endpoint_round_trips() {
    let src = r#"
model Room {
  id: uuid
  @version
  revision: long
}

service Rooms at "/rooms" {
  @idempotent(required: true)
  put "/{id}" save(id: uuid, body: Room) -> Room | NotFound {
    fact room: Room = id
  }
}
"#;
    let out = stable(src);
    assert!(out.contains("@version"), "{out}");
    assert!(out.contains("@idempotent(required: true)"), "{out}");
}

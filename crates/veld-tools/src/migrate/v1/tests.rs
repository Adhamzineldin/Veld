use super::ast::{Headers, Item, TypeRef, UnionAlt, V1Type, Value};
use super::parse;

fn file(src: &str) -> super::V1File {
    match parse(src) {
        Ok(p) => p.file,
        Err(e) => panic!("v1 parse failed: {e}"),
    }
}

#[test]
fn parses_imports_in_every_form() {
    let f = file(
        "import \"../models/a.veld\"\nimport @models/user\nimport models/x\nfrom @models import User, Role\nfrom /models import *\nimport @services/**\n",
    );
    let texts: Vec<(&str, bool)> = f.imports.iter().map(|i| (i.text.as_str(), i.glob)).collect();
    assert_eq!(
        texts,
        [
            ("models/a", false),
            ("models/user", false),
            ("models/x", false),
            ("models/User", false),
            ("models/*", true),
            ("services/*", true),
        ]
    );
}

#[test]
fn parses_models_with_all_field_forms() {
    let f = file(
        "model Child extends Parent {\n  description: \"A child\"\n  id: string @unique @index\n  n?: int @default(3) @deprecated \"old\"\n  tags: List<string>\n  m: Map<string, int>\n  xs: Item[]\n  s: string = \"x\" @example(\"y\")\n  who: Role @serverSet @relation(User)\n}",
    );
    let Item::Model(m) = &f.items[0] else { panic!("model expected") };
    assert_eq!(m.extends.as_deref(), Some("Parent"));
    assert_eq!(m.description.as_deref(), Some("A child"));
    assert!(m.fields[0].unique && m.fields[0].index);
    assert!(m.fields[1].optional);
    assert_eq!(m.fields[1].default, Some(Value::Num("3".into())));
    assert_eq!(m.fields[1].deprecated.as_deref(), Some("old"));
    assert_eq!(m.fields[2].ty, V1Type::List(Box::new(V1Type::Named("string".into()))));
    assert_eq!(m.fields[3].ty, V1Type::Map(Box::new(V1Type::Named("int".into()))));
    assert_eq!(m.fields[4].ty, V1Type::List(Box::new(V1Type::Named("Item".into()))));
    assert_eq!(m.fields[5].default, Some(Value::Str("x".into())));
    assert_eq!(m.fields[5].example, Some(Value::Str("y".into())));
    assert!(m.fields[6].server_set);
    assert_eq!(m.fields[6].relation.as_deref(), Some("User"));
}

#[test]
fn a_field_may_be_called_description() {
    let f = file("model A { description: string\n name: string }");
    let Item::Model(m) = &f.items[0] else { panic!("model expected") };
    assert!(m.description.is_none());
    assert_eq!(m.fields[0].name, "description");
}

#[test]
fn parses_enums_constants_and_prefix() {
    let f = file("prefix: /v1\nenum Role { admin user, guest }\nconstants Http { OK: int = 200\n NAME: string = \"x\" }");
    assert_eq!(f.prefix.as_deref(), Some("/v1"));
    let Item::Enum(e) = &f.items[0] else { panic!("enum expected") };
    assert_eq!(e.values, ["admin", "user", "guest"]);
    let Item::Constants(c) = &f.items[1] else { panic!("constants expected") };
    assert_eq!(c.items.len(), 2);
    assert_eq!(c.items[1].value, Value::Str("x".into()));
}

#[test]
fn parses_modules_and_actions() {
    let f = file(
        "module M {\n description: \"d\"\n prefix: /api/{x}\n action A {\n  method: POST\n  path: /:id/x\n  input: In\n  output: Out[]:201\n  query: Q\n  headers: { \"Idempotency-Key\": string, Auth?: string }\n  middleware: [G, H]\n  errors: [NotFound, Conflict:409]\n  @deprecated \"gone\"\n }\n action B { method: WS path: /ws stream: Tick emit: Cmd middleware: Guard }\n}",
    );
    let Item::Module(m) = &f.items[0] else { panic!("module expected") };
    assert_eq!(m.prefix.as_deref(), Some("/api/{x}"));
    let a = &m.actions[0];
    assert_eq!(a.method, "POST");
    assert_eq!(a.input, Some(TypeRef::Named("In".into())));
    let out = a.output.as_ref();
    assert_eq!(out.map(|o| (o.array, o.status)), Some((true, Some(201))));
    assert!(matches!(&a.headers, Some(Headers::Inline(h)) if h.len() == 2 && h[1].optional));
    assert_eq!(a.middleware, ["G", "H"]);
    assert_eq!(a.errors, [("NotFound".to_string(), None), ("Conflict".to_string(), Some(409))]);
    assert_eq!(a.deprecated.as_deref(), Some("gone"));
    let b = &m.actions[1];
    assert_eq!((b.stream.as_deref(), b.emit.as_deref()), (Some("Tick"), Some("Cmd")));
    assert_eq!(b.middleware, ["Guard"]);
}

#[test]
fn inline_types_are_parsed() {
    let f = file("module M { action A { method: GET path: / input: { a: int, b?: string } output: { c: float }[] query: { q: string } } }");
    let Item::Module(m) = &f.items[0] else { panic!("module expected") };
    let a = &m.actions[0];
    assert!(matches!(&a.input, Some(TypeRef::Inline(f)) if f.len() == 2));
    assert!(a.output.as_ref().is_some_and(|o| o.array && matches!(&o.ty, TypeRef::Inline(f) if f.len() == 1)));
    assert!(matches!(&a.query, Some(TypeRef::Inline(_))));
}

#[test]
fn field_unions_are_kept() {
    let f = file("model A { s: string | \"x\" | \"y\"\n t: string | int }");
    let Item::Model(m) = &f.items[0] else { panic!("model expected") };
    assert_eq!(
        m.fields[0].union,
        [UnionAlt::Type("string".into()), UnionAlt::Literal("x".into()), UnionAlt::Literal("y".into())]
    );
    assert_eq!(m.fields[1].union.len(), 2);
}

#[test]
fn comments_become_leading_and_trailing_text() {
    let f = file("// top\nmodel A {\n  // lead\n  a: int // trail\n  b: int\n}\n// end\n");
    let Item::Model(m) = &f.items[0] else { panic!("model expected") };
    assert_eq!(m.comments, [" top"]);
    assert_eq!(m.fields[0].comments, [" lead"]);
    assert_eq!(m.fields[0].trailing.as_deref(), Some(" trail"));
    assert_eq!(f.trailing, [" end"]);
}

#[test]
fn syntax_errors_name_the_line() {
    let e = parse("model A {\n a int\n}").err().unwrap_or_default();
    assert!(e.starts_with("line 2:"), "{e}");
    assert!(parse("module M { action A { method: GET } }").is_err());
    assert!(parse("wat").is_err());
}

#[test]
fn unknown_field_annotations_are_kept_for_the_warning() {
    let f = file("model A { a: int @shiny(1, \"x\") }");
    let Item::Model(m) = &f.items[0] else { panic!("model expected") };
    assert_eq!(m.fields[0].unknown, ["@shiny(1,\"x\")"]);
}

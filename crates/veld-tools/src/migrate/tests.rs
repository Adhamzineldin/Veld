use std::collections::BTreeMap;

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;

use super::{MigrateResult, migrate_v1};

fn run(files: &[(&str, &str)]) -> MigrateResult {
    let owned: Vec<(String, String)> = files.iter().map(|(p, t)| ((*p).to_string(), (*t).to_string())).collect();
    migrate_v1(&owned)
}

fn errors_of(r: &MigrateResult) -> Vec<String> {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: r.files.iter().map(|(p, t)| SourceFile { path: p.clone(), text: t.clone() }).collect(),
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    out.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("{} {}", d.code, d.message)).collect()
}

/// Migrates one file, checks that the output compiles, and returns its text.
fn one(src: &str) -> (String, Vec<String>) {
    let r = run(&[("a.veld", src)]);
    assert_eq!(errors_of(&r), Vec::<String>::new(), "output must compile:\n{}", r.files.first().map_or("", |f| f.1.as_str()));
    let text = r.files.first().map(|f| f.1.clone()).unwrap_or_default();
    (text, r.warnings)
}

fn has_warning(w: &[String], needle: &str) -> bool {
    w.iter().any(|x| x.contains(needle))
}

const SVC: &str = "module M { prefix: /m\n";

#[test]
fn descriptions_prefixes_paths_and_names() {
    let (t, w) = one("model A { description: \"An A\"\n id: string }\nmodule Things {\n description: \"Thing API\"\n prefix: /api/things\n action ListThings { description: \"List\" method: GET path: / output: A[] }\n action GetThing { method: GET path: /:id output: A }\n}");
    assert_eq!(
        t,
        "/// An A\nmodel A {\n  id: string\n}\n\n/// Thing API\nservice Things at \"/api/things\" {\n  /// List\n  get \"/\" listThings() -> A[]\n\n  get \"/{id}\" getThing(id: string) -> A\n}\n"
    );
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn input_query_headers_and_status() {
    let (t, _) = one(&format!(
        "model In {{ a: string }}\nmodel Q {{ q?: string }}\nmodel Out {{ a: string }}\n{SVC} action Create {{ method: POST path: / input: In output: Out:200 }}\n action Search {{ method: GET path: / query: Q output: Out[] }}\n action H {{ method: GET path: /h headers: {{ \"Idempotency-Key\": string, Trace?: string }} }}\n}}"
    ));
    assert!(t.contains("post \"/\" create(body: In) -> 200 Out"), "{t}");
    assert!(t.contains("get \"/\" search(query: Q) -> Out[]"), "{t}");
    assert!(t.contains("get \"/h\" h(@header(\"Idempotency-Key\") idempotencyKey: string, @header(\"Trace\") trace?: string)"), "{t}");
}

#[test]
fn named_header_models_are_expanded() {
    let (t, _) = one(&format!("model Hs {{ Authorization: string }}\n{SVC} action H {{ method: GET path: / headers: Hs }}\n}}"));
    assert!(t.contains("h(@header(\"Authorization\") authorization: string)"), "{t}");
}

#[test]
fn input_on_get_becomes_query_with_a_warning() {
    let (t, w) = one(&format!("model F {{ q?: string }}\n{SVC} action L {{ method: GET path: / input: F }}\n}}"));
    assert!(t.contains("l(query: F)"), "{t}");
    assert!(has_warning(&w, "became `query`"), "{w:?}");
}

#[test]
fn inline_types_become_generated_models() {
    let (t, _) = one(&format!("{SVC} action Make {{ method: POST path: / input: {{ a: int }} output: {{ b: string }}[] }}\n}}"));
    assert!(t.contains("post \"/\" make(body: MakeInput) -> MakeOutput[]"), "{t}");
    assert!(t.contains("model MakeInput {\n  a: int\n}"), "{t}");
    assert!(t.contains("model MakeOutput {\n  b: string\n}"), "{t}");
}

#[test]
fn auth_like_middleware_becomes_auth_with_a_warning_naming_the_decision() {
    let (t, w) = one(&format!("{SVC} action A {{ method: GET path: / middleware: [AuthGuard, RateLimit] }}\n action B {{ method: GET path: /b middleware: Audit }}\n}}"));
    assert!(t.contains("@auth(bearer)\n  @middleware(RateLimit)\n  get \"/\" a()"), "{t}");
    assert!(t.contains("@middleware(Audit)\n  get \"/b\" b()"), "{t}");
    assert!(has_warning(&w, "`AuthGuard` looks like authentication; mapped to @auth(bearer)"), "{w:?}");
}

#[test]
fn identical_guards_on_every_action_move_to_the_service() {
    let (t, _) = one(&format!("{SVC} action A {{ method: GET path: / middleware: [AuthGuard] }}\n action B {{ method: GET path: /b middleware: AuthGuard }}\n}}"));
    assert!(t.contains("@auth(bearer)\nservice M at \"/m\" {"), "{t}");
    assert!(!t.contains("  @auth"), "{t}");
}

#[test]
fn errors_map_to_prelude_or_generated_declarations() {
    let (t, w) = one(&format!("{SVC} action A {{ method: GET path: / errors: [NotFound, Teapot:418, Conflict:410] }}\n action B {{ method: GET path: /b errors: [Teapot] }}\n}}"));
    assert!(t.contains("get \"/\" a() -> NotFound | Teapot | 410 Conflict"), "{t}");
    assert!(t.contains("get \"/b\" b() -> Teapot"), "{t}");
    assert!(t.contains("error Teapot 418 {\n  message?: string\n}"), "{t}");
    assert!(has_warning(&w, "`Teapot` is not in the v2 prelude; declared `error Teapot` with status 418"), "{w:?}");
    assert_eq!(w.iter().filter(|x| x.contains("`Teapot` is not")).count(), 1, "one warning per error: {w:?}");
}

#[test]
fn error_names_that_clash_with_a_model_get_an_error_suffix() {
    let (t, _) = one(&format!("model Oops {{ a: string }}\n{SVC} action A {{ method: GET path: / errors: [Oops] }}\n}}"));
    assert!(t.contains("-> OopsError"), "{t}");
    assert!(t.contains("error OopsError 400"), "{t}");
}

#[test]
fn field_forms_and_annotations() {
    let (t, w) = one("enum Role { admin user }\nmodel A {\n id: string @unique @index\n n?: int\n tags: List<string>\n m: Map<string, int>\n xs: A[]\n r: Role @default(user)\n c: int = 3 @example(\"4\")\n old: string @deprecated \"use id\"\n s: string @serverSet\n}");
    for line in [
        "id: string @unique @index",
        "n?: int",
        "tags: string[]",
        "m: Map<string, int>",
        "xs: A[]",
        "r: Role = .user",
        "c: int = 3 @example(4)",
        "old: string @deprecated(\"use id\")",
        "s: string @readonly",
    ] {
        assert!(t.contains(line), "missing `{line}` in\n{t}");
    }
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn optional_fields_with_defaults_lose_the_question_mark() {
    let (t, w) = one("model A { n?: int @default(3) }");
    assert!(t.contains("n: int = 3"), "{t}");
    assert!(has_warning(&w, "optional field with a default"), "{w:?}");
}

#[test]
fn unmappable_annotations_leave_todo_comments_and_warnings() {
    let (t, w) = one("model U { id: string }\nmodel A {\n u: U @relation(U)\n x: int @shiny(1)\n}");
    assert!(t.contains("// TODO(veld migrate): @relation(U) on `u` has no v2 equivalent\n  u: U"), "{t}");
    assert!(t.contains("// TODO(veld migrate): annotation @shiny(1) on `x` was dropped"), "{t}");
    assert!(has_warning(&w, "@relation(U) has no v2 equivalent"));
    assert!(has_warning(&w, "unknown annotation @shiny(1)"));
}

#[test]
fn reserved_words_are_renamed_with_wire_names() {
    let (t, w) = one("enum E { error ok }\nmodel A { stream: string }");
    assert!(t.contains("enum E { error_ = \"error\", ok }"), "{t}");
    assert!(t.contains("stream_: string @wire(\"stream\")"), "{t}");
    assert!(has_warning(&w, "`stream` is a reserved word"));
}

#[test]
fn string_literal_unions_become_enums_and_type_unions_become_json() {
    let (t, w) = one("model A { status: string | \"draft\" | \"in-review\"\n v: string | int }");
    assert!(t.contains("status: AStatus"), "{t}");
    assert!(t.contains("enum AStatus { draft, in_review = \"in-review\" }"), "{t}");
    assert!(t.contains("// TODO(veld migrate): v was a union of types"), "{t}");
    assert!(t.contains("v: json"), "{t}");
    assert!(has_warning(&w, "became the generated enum `AStatus`"));
    assert!(has_warning(&w, "union of types has no v2 form"));
}

#[test]
fn constants_are_flattened_with_a_warning() {
    let (t, w) = one("constants Http {\n OK: int = 200\n NAME: string = \"x\"\n RATE: float = 2\n}");
    assert!(t.contains("const OK: int = 200"), "{t}");
    assert!(t.contains("const NAME: string = \"x\""), "{t}");
    assert!(t.contains("const RATE: float = 2"), "{t}");
    assert!(has_warning(&w, "constants group `Http` was flattened"));
}

#[test]
fn duplicate_constant_names_get_the_group_prefix() {
    let (t, _) = one("constants A { MAX: int = 1 }\nconstants B { MAX: int = 2 }");
    assert!(t.contains("const A_MAX: int = 1") && t.contains("const B_MAX: int = 2"), "{t}");
}

#[test]
fn websocket_actions_use_stream_bodies_and_responses() {
    let (t, _) = one(&format!("model Cmd {{ t: string }}\nmodel Tick {{ n: int }}\n{SVC} action Chat {{ method: WS path: /chat emit: Cmd stream: Tick }}\n}}"));
    assert!(t.contains("ws \"/chat\" chat(body: stream Cmd) -> stream Tick"), "{t}");
}

#[test]
fn action_deprecation_and_extends() {
    let (t, _) = one(&format!("model P {{ a: int }}\nmodel C extends P {{ b: int }}\n{SVC} action Old {{ method: GET path: / @deprecated \"use new\" }}\n}}"));
    assert!(t.contains("model C extends P {"), "{t}");
    assert!(t.contains("@deprecated(\"use new\")\n  get \"/\" old()"), "{t}");
}

#[test]
fn comments_survive_as_comments() {
    let (t, _) = one("// about A\nmodel A {\n  // the id\n  id: string // trailing\n  n: int\n}\n// the end");
    assert!(t.contains("// about A\nmodel A {"), "{t}");
    assert!(t.contains("  // the id\n  id: string // trailing"), "{t}");
    assert!(t.trim_end().ends_with("// the end"), "{t}");
}

#[test]
fn imports_are_rebuilt_from_the_names_a_file_uses() {
    let r = run(&[
        ("models/user.veld", "model User { id: string }\nenum Role { admin }"),
        ("models/todo.veld", "import @models/user\nmodel Todo { owner: User\n role: Role }"),
        ("modules/todos.veld", "import @models/todo\nmodule Todos { prefix: /t action G { method: GET path: / output: Todo } }"),
    ]);
    assert_eq!(errors_of(&r), Vec::<String>::new());
    let text = |p: &str| r.files.iter().find(|f| f.0 == p).map(|f| f.1.clone()).unwrap_or_default();
    assert!(text("models/todo.veld").starts_with("use models.user.{Role, User}\n"), "{}", text("models/todo.veld"));
    assert!(text("modules/todos.veld").starts_with("use models.todo.{Todo}\n"));
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn files_with_only_imports_are_omitted_with_a_warning() {
    let r = run(&[("app.veld", "import @modules/todos"), ("m.veld", "model A { a: int }")]);
    assert_eq!(r.files.len(), 1);
    assert!(has_warning(&r.warnings, "app.veld: only contains imports"));
}

#[test]
fn unresolvable_imports_warn() {
    let r = run(&[("m.veld", "import \"./nowhere.veld\"\nmodel A { a: int }")]);
    assert!(has_warning(&r.warnings, "import `nowhere` matches no file"));
}

#[test]
fn syntax_errors_keep_the_original_as_comments() {
    let r = run(&[("bad.veld", "model A {\n a int\n}")]);
    assert!(has_warning(&r.warnings, "bad.veld: not migrated, v1 syntax error: line 2"));
    let text = &r.files[0].1;
    assert!(text.starts_with("// TODO(veld migrate): this v1 file could not be parsed"), "{text}");
    assert!(text.contains("//  a int"), "{text}");
    assert_eq!(errors_of(&r), Vec::<String>::new());
}

#[test]
fn unknown_types_are_reported_by_the_compile_check() {
    let r = run(&[("a.veld", "model A { b: Missing }")]);
    assert!(r.warnings.iter().any(|w| w.contains("migrated output has an error E0205")), "{:?}", r.warnings);
}

#[test]
fn output_is_already_canonical() {
    let (t, _) = one(&format!("model A {{ a: int }}\n{SVC} action X {{ method: GET path: / output: A }}\n}}"));
    assert_eq!(crate::fmt::format(&t).ok().as_deref(), Some(t.as_str()));
}

#[test]
fn duplicate_names_across_files_warn() {
    let r = run(&[("a.veld", "model A { a: int }"), ("b.veld", "model A { b: int }")]);
    assert!(has_warning(&r.warnings, "`A` is declared in more than one file"));
}

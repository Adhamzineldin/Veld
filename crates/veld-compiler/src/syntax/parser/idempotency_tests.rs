//! Parser tests for `ifMatch` and `key` lines of an example (`spec/rules.md` section 11.9). The
//! words `ifMatch`, `any`, `stored`, `other`, `running` and `key` are contextual: they are words
//! only where the grammar expects them.

use super::{Parsed, parse};
use crate::diagnostics::FileId;
use crate::syntax::ast::{ExampleBody, ExampleCall, KeyStoreLine};

fn clean(src: &str) -> Parsed {
    let r = parse(src, FileId(0));
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    r
}

fn call(r: &Parsed) -> &ExampleCall {
    match r.file.examples.first().map(|e| &e.body) {
        Some(ExampleBody::Call(c)) => c,
        _ => panic!("no call example"),
    }
}

fn example(lines: &str) -> String {
    format!("example \"e\" {{\n{lines}\n}}\n")
}

#[test]
fn if_match_takes_a_version_or_any() {
    let r = clean(&example("  ifMatch 3\n  save() -> ok"));
    let c = call(&r);
    assert_eq!(c.if_match.as_ref().map(|i| i.version), Some(Some(3)));
    assert!(c.key.is_none());
    let r = clean(&example("  ifMatch any\n  save() -> ok"));
    assert_eq!(call(&r).if_match.as_ref().map(|i| i.version), Some(None));
}

#[test]
fn a_key_line_has_four_forms() {
    let forms = [
        ("key \"k\"", "new"),
        ("key \"k\" other", "other"),
        ("key \"k\" running", "running"),
        ("key \"k\" stored Full { capacity: 2 }", "stored"),
        ("key \"k\" stored ok", "stored"),
    ];
    for (line, kind) in forms {
        let r = clean(&example(&format!("  {line}\n  save() -> ok")));
        let k = call(&r).key.as_ref();
        let found = match k.map(|k| &k.store) {
            Some(KeyStoreLine::New) => "new",
            Some(KeyStoreLine::Other) => "other",
            Some(KeyStoreLine::Running) => "running",
            Some(KeyStoreLine::Stored(_)) => "stored",
            None => "none",
        };
        assert_eq!(found, kind, "{line}");
        assert_eq!(k.map(|k| k.key.value.as_str()), Some("k"));
    }
}

#[test]
fn both_lines_come_before_the_call_in_that_order() {
    let r = clean(&example(
        "  given room = absent\n  as { id: 1 }\n  at \"2026-01-01T00:00:00Z\"\n  ifMatch 7\n  key \"k\" other\n  save() -> ok",
    ));
    let c = call(&r);
    assert!(c.if_match.is_some() && c.key.is_some());
    assert_eq!(c.endpoint.name, "save");
}

#[test]
fn a_stored_value_must_start_on_the_line_of_its_outcome() {
    // Without the rule the call on the next line would read as the value of the outcome.
    let r = clean(&example("  key \"k\" stored Full\n  save() -> ok"));
    let c = call(&r);
    let Some(KeyStoreLine::Stored(o)) = c.key.as_ref().map(|k| &k.store) else {
        panic!("not stored");
    };
    assert!(o.value.is_none());
    assert_eq!(c.endpoint.name, "save");
    // A value that begins on the same line may go on to the next ones.
    let r = clean(&example(
        "  key \"k\" stored Full {\n    capacity: 2\n  }\n  save() -> ok",
    ));
    let Some(KeyStoreLine::Stored(o)) = call(&r).key.as_ref().map(|k| &k.store) else {
        panic!("not stored");
    };
    assert!(o.value.is_some());
}

#[test]
fn the_words_are_names_everywhere_else() {
    // Endpoints with the names of the words, called with and without a line before them.
    for word in ["ifMatch", "any", "stored", "other", "running", "key"] {
        let r = clean(&example(&format!("  {word}() -> ok")));
        assert_eq!(call(&r).endpoint.name, word);
        assert!(call(&r).if_match.is_none() && call(&r).key.is_none());
        let r = clean(&example(&format!("  key \"k\"\n  {word}() -> ok")));
        assert_eq!(call(&r).endpoint.name, word);
        assert!(call(&r).key.is_some());
        let r = clean(&example(&format!(
            "  key \"k\" other\n  {word}(a: 1) -> ok"
        )));
        assert_eq!(call(&r).endpoint.name, word);
        let r = clean(&example(&format!("  {word}.go() -> ok")));
        assert_eq!(
            call(&r).service.as_ref().map(|s| s.name.as_str()),
            Some(word)
        );
    }
    // Fields, parameters, variants and models of those names.
    clean(
        "model ifMatch { any: int\n stored: bool\n other?: string\n running?: string\n key: uuid }\n\
         enum E { stored, other, running, any, ifMatch, key }\n\
         service S at \"/s\" { get \"/\" f(key: string, any: int) -> ifMatch }\n",
    );
}

#[test]
fn a_malformed_if_match_is_an_error_with_a_hint() {
    for bad in ["ifMatch \"3\"", "ifMatch -3", "ifMatch maybe", "ifMatch"] {
        let r = parse(&example(&format!("  {bad}\n  save() -> ok")), FileId(0));
        assert!(!r.diagnostics.is_empty(), "{bad}");
    }
}

#[test]
fn a_key_needs_a_string() {
    let r = parse(&example("  key\n  save() -> ok"), FileId(0));
    // Without a string `key` is the call of an endpoint named `key`, which then misses its `(`.
    assert!(!r.diagnostics.is_empty());
}

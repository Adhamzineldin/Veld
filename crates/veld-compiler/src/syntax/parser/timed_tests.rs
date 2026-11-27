//! Parser tests for `flow ... after` and `transition ... since` (`spec/platform.md` section 10).
//! `after` and `since` are contextual: words only on the line of a `flow` or a `transition`.

use super::{Parsed, parse};
use crate::diagnostics::FileId;
use crate::syntax::ast::{DeclKind, ExprKind, Stmt};

fn clean(src: &str) -> Parsed {
    let r = parse(src, FileId(0));
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    r
}

#[test]
fn after_on_a_flow_keeps_its_expression() {
    let r = clean("enum S { a, b\n flow a -> b after hours(24)\n final b }");
    let DeclKind::Enum(e) = &r.file.decls[0].kind else {
        panic!("not an enum")
    };
    assert_eq!(e.flows.len(), 1);
    let after = e.flows[0].after.as_ref().expect("after");
    assert!(matches!(&after.kind, ExprKind::Call { name, .. } if name.name == "hours"));
    assert_eq!(e.finals.len(), 1);
}

#[test]
fn a_flow_without_after_has_none() {
    let r = clean("enum S { a, b\n flow a -> b\n}");
    let DeclKind::Enum(e) = &r.file.decls[0].kind else {
        panic!("not an enum")
    };
    assert!(e.flows[0].after.is_none());
}

#[test]
fn since_on_a_transition_keeps_its_path() {
    let src = "service S at \"/s\" {\n post \"/x\" go(id: uuid) -> int {\n  fact d: Doc = id\n  \
               transition d.state: a -> b since d.stamps.entered\n }\n}";
    let r = clean(src);
    let DeclKind::Service(s) = &r.file.decls[0].kind else {
        panic!("not a service")
    };
    let block = s.endpoints[0].block.as_ref().expect("block");
    let Some(Stmt::Transition(t)) = block
        .stmts
        .iter()
        .find(|s| matches!(s, Stmt::Transition(_)))
    else {
        panic!("no transition")
    };
    let names: Vec<&str> = t
        .since
        .as_ref()
        .expect("since")
        .iter()
        .map(|i| i.name.as_str())
        .collect();
    assert_eq!(names, ["d", "stamps", "entered"]);
}

#[test]
fn after_and_since_stay_ordinary_names() {
    let src = "model after { since: datetime, after: int }\n\
               enum E { after, since, a\n flow a -> after\n flow after -> since\n}\n\
               service S at \"/s\" {\n get \"/a\" since(@query after: int) -> after\n \
               get \"/b\" after() -> int { fact since: after = 1\n transition since.after: a -> b }\n}";
    let r = parse(src, FileId(0));
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn a_since_on_the_next_line_is_not_a_clause() {
    let src = "service S at \"/s\" {\n post \"/x\" go(id: uuid) -> int {\n  fact d: Doc = id\n  \
               transition d.state: a -> b\n  fact since: Doc = id\n }\n}";
    let r = parse(src, FileId(0));
    // `since` starts a name on the next line; it must not be taken as a `since` clause.
    assert!(
        !r.diagnostics.iter().any(|d| d.message.contains("since")),
        "{:?}",
        r.diagnostics
    );
}

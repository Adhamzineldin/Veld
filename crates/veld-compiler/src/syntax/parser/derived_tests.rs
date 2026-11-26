//! What follows `=` in a field (`spec/rules.md` section 17.2): one literal is a default, any
//! other expression makes the field derived. There is no keyword.

use super::*;
use crate::syntax::ast::{DeclKind, ExprKind, LitKind};

fn field(src: &str) -> crate::syntax::ast::Field {
    let r = parse(&format!("model M {{ {src} }}"), FileId(0));
    assert!(r.diagnostics.is_empty(), "{src}: {:?}", r.diagnostics);
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    m.fields[0].clone()
}

fn is_default(src: &str) -> bool {
    let f = field(src);
    assert!(f.default.is_some() ^ f.derived.is_some(), "{src}");
    f.default.is_some()
}

#[test]
fn a_literal_stays_a_default() {
    for src in [
        "a: int = 3",
        "a: int = -1",
        "a: float = -0.5",
        "a: string = \"x\"",
        "a: bool = true",
        "a: bool = false",
        "a?: string = null",
        "a: S = .open",
        "a: int[] = []",
    ] {
        assert!(is_default(src), "{src}");
    }
}

#[test]
fn any_other_expression_makes_the_field_derived() {
    for src in [
        "a: int = b",
        "a: int = -b",
        "a: int = b + 1",
        "a: int = 1 + 1",
        "a: bool = !b",
        "a: bool = not b",
        "a: int = (1)",
        "a: int = len(b)",
        "a: int = b.c",
        "a: bool = .open == s",
        "a: int[] = [1, 2]",
        "a: bool = 1 < 2",
        "a: bool = true && b",
        "a: bool = 1 in [1, 2]",
        "a: int = sum(xs, x => x.n)",
    ] {
        assert!(!is_default(src), "{src}");
    }
}

#[test]
fn the_derived_expression_is_the_whole_expression() {
    let f = field("a: int = b + 2 * c");
    let Some(e) = f.derived else {
        panic!("not derived")
    };
    assert!(matches!(e.kind, ExprKind::Binary { .. }));
    assert!(f.default.is_none());
}

#[test]
fn annotations_after_a_derived_field_still_belong_to_the_field() {
    let f = field("a: int = b + 1 @deprecated(\"x\")");
    assert!(f.derived.is_some());
    assert_eq!(f.annotations.len(), 1);
}

#[test]
fn the_next_member_is_not_swallowed() {
    let r = parse(
        "model M {\n  a: int = 1\n  b: int = a + 1\n  c: int = b\n  d: int\n  rule r: d > 0\n}",
        FileId(0),
    );
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    let kinds: Vec<(bool, bool)> = m
        .fields
        .iter()
        .map(|f| (f.default.is_some(), f.derived.is_some()))
        .collect();
    assert_eq!(
        kinds,
        [(true, false), (false, true), (false, true), (false, false)]
    );
    assert_eq!(m.rules.len(), 1);
}

#[test]
fn a_bad_default_is_still_reported_where_it_was() {
    let lits = field("a: int = 7").default;
    assert!(matches!(lits.map(|l| l.kind), Some(LitKind::Int(7))));
    let r = parse("model M { a: int = }", FileId(0));
    assert_eq!(
        r.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["E0003"]
    );
}

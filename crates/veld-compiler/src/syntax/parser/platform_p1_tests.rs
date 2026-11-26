//! The annotations of platform phase P1 as the parser reads them (`spec/platform.md` sections 2
//! to 5). `@visible` takes an expression after `when:`; every other argument stays an ordinary
//! argument.

use super::*;
use crate::syntax::ast::{DeclKind, ExprKind};

fn field(src: &str) -> crate::syntax::ast::Field {
    let r = parse(&format!("model M {{ {src} }}"), FileId(0));
    assert!(r.diagnostics.is_empty(), "{src}: {:?}", r.diagnostics);
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    m.fields[0].clone()
}

#[test]
fn visible_keeps_its_expression_and_has_no_arguments() {
    let f = field("a?: string @visible(when: user.id == owner || \"x\" in user.roles)");
    let a = &f.annotations[0];
    assert_eq!(a.name.name, "visible");
    assert!(a.args.is_none());
    let when = a.when.as_ref().unwrap();
    assert!(matches!(when.kind, ExprKind::Binary { .. }), "{when:?}");
}

#[test]
fn the_expression_may_hold_parentheses_calls_and_lambdas() {
    let f = field("a?: string @visible(when: (len(user.roles) > 0) && !has(user.name))");
    assert!(f.annotations[0].when.is_some());
}

#[test]
fn visible_without_when_is_an_ordinary_argument_list() {
    let f = field("a?: string @visible(true)");
    let a = &f.annotations[0];
    assert!(a.when.is_none() && a.args.as_ref().is_some_and(|v| v.len() == 1));
}

#[test]
fn other_annotations_never_take_an_expression() {
    let f = field("a?: string @deprecated(when: x)");
    let a = &f.annotations[0];
    assert!(a.when.is_none() && a.args.is_some());
}

#[test]
fn a_missing_closing_parenthesis_is_a_syntax_error() {
    let r = parse("model M { a?: string @visible(when: a == 1 }", FileId(0));
    assert!(!r.diagnostics.is_empty());
}

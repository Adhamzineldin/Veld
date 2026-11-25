//! The roots of an endpoint block: `param`, `fact`, `now`, `result` and `old(...)`.

use super::*;
use crate::tree::Tree;
use veld_codegen::print_expr;
use veld_ir::{BinExpr, Expr, ExprKind, Scalar, Step};

const SOURCE: &str = r#"model Line {
  qty: int
  stamp: datetime
  nick?: string
  inner?: Inner
}

model Inner {
  n: int
}

service S at "/s" {
  post "/" go(body: Line) -> 204
}
"#;

fn sc(s: Scalar) -> ExprType {
    ExprType::Type(Type::Scalar(s))
}

fn ex(t: ExprType, e: ExprKind) -> Expr {
    Expr { t, e }
}

fn step(name: &str, optional: bool) -> Step {
    Step {
        name: name.into(),
        wire: name.into(),
        optional,
    }
}

fn path(root: Root, steps: Vec<Step>, t: ExprType) -> Expr {
    ex(t, ExprKind::Path(PathRef { root, steps }))
}

fn var(code: &str, name: &str, ty: Type, optional: bool) -> ScopeVar {
    ScopeVar {
        name: name.into(),
        code: code.into(),
        ty,
        optional,
    }
}

/// What the printer gives for `expr` in the scope of an endpoint that takes a `body: Line`, an
/// optional `note: string`, has the facts `line: Line` and `maybe?: Line`, and returns a `Line`.
fn print(expr: &Expr) -> (String, Vec<String>, Vec<(String, ExprType)>) {
    let ir = veld_emit_conformance::compile("rule_expr", SOURCE);
    let services = ir.services.iter().map(|s| s.id).collect();
    let tree = Tree::new(&ir, "g", "g.runtime", services);
    let line = ir.decls.iter().find(|d| d.name == "Line").unwrap();
    let ty = Type::Decl(line.id);
    let scope = EndpointScope {
        params: vec![
            var("request.body()", "body", ty.clone(), false),
            var("request.note()", "note", Type::Scalar(Scalar::String), true),
            var("request.id()", "id", Type::Scalar(Scalar::Uuid), false),
        ],
        facts: vec![
            var("fact_line", "line", ty.clone(), false),
            var("fact_maybe", "maybe", ty.clone(), true),
        ],
        principal: Some(var("principal", "user", ty.clone(), false)),
        result: Some(ty),
    };
    let mut fc = FileCtx::new(&tree, "g.app", "Handler");
    let mut printer = JavaExpr::for_endpoint(&mut fc, scope);
    let code = print_expr(&mut printer, expr).code;
    (
        code,
        printer.unsupported().to_vec(),
        printer.olds().to_vec(),
    )
}

fn code(expr: &Expr) -> String {
    let (code, unsupported, _) = print(expr);
    assert!(unsupported.is_empty(), "{unsupported:?}");
    code
}

fn bin(op: Op, l: Expr, r: Expr, t: ExprType) -> Expr {
    ex(t, ExprKind::Bin(Box::new(BinExpr { op, l, r })))
}

fn param(name: &str, steps: Vec<Step>, t: ExprType) -> Expr {
    path(Root::Param(name.into()), steps, t)
}

fn fact(name: &str, steps: Vec<Step>, t: ExprType) -> Expr {
    path(Root::Fact(name.into()), steps, t)
}

fn now() -> Expr {
    ex(sc(Scalar::Datetime), ExprKind::Now(veld_ir::Always))
}

#[test]
fn a_parameter_is_read_from_the_request_record() {
    let qty = param("body", vec![step("qty", false)], sc(Scalar::Int));
    assert_eq!(code(&qty), "request.body().qty()");
    let id = param("id", vec![], sc(Scalar::Uuid));
    assert_eq!(code(&id), "request.id()");
    let note = param("note", vec![], sc(Scalar::String));
    assert_eq!(code(&note), "request.note()");
}

#[test]
fn a_datetime_is_converted_where_it_is_read_and_now_is_a_local() {
    let start = param("body", vec![step("stamp", false)], sc(Scalar::Datetime));
    let past = bin(Op::Ge, start, now(), sc(Scalar::Bool));
    assert_eq!(
        code(&past),
        "Rules.instantMs(request.body().stamp()) >= now"
    );
}

#[test]
fn a_fact_is_a_local_named_after_it() {
    let qty = fact("line", vec![step("qty", false)], sc(Scalar::Int));
    assert_eq!(code(&qty), "fact_line.qty()");
    let nick = fact("line", vec![step("nick", true)], sc(Scalar::String));
    assert_eq!(code(&nick), "fact_line.nick()");
}

#[test]
fn has_checks_an_optional_root_and_the_optional_steps_below_it() {
    let present =
        |root: Root, steps: Vec<Step>| ex(sc(Scalar::Bool), ExprKind::Has(PathRef { root, steps }));
    assert_eq!(
        code(&present(Root::Fact("maybe".into()), vec![])),
        "fact_maybe != null"
    );
    assert_eq!(
        code(&present(Root::Param("note".into()), vec![])),
        "request.note() != null"
    );
    assert_eq!(
        code(&present(
            Root::Fact("line".into()),
            vec![step("nick", true)]
        )),
        "fact_line.nick() != null"
    );
    assert_eq!(
        code(&present(Root::Fact("line".into()), vec![])),
        "true",
        "a required fact is always there"
    );
}

#[test]
fn the_principal_is_a_local_of_the_generated_type_of_its_model() {
    let qty = path(Root::Principal, vec![step("qty", false)], sc(Scalar::Int));
    assert_eq!(code(&qty), "principal.qty()");
    let stamp = path(
        Root::Principal,
        vec![step("stamp", false)],
        sc(Scalar::Datetime),
    );
    assert_eq!(code(&stamp), "Rules.instantMs(principal.stamp())");
    let nick = path(
        Root::Principal,
        vec![step("nick", true)],
        sc(Scalar::String),
    );
    assert_eq!(code(&nick), "principal.nick()");
    let has = ex(
        sc(Scalar::Bool),
        ExprKind::Has(PathRef {
            root: Root::Principal,
            steps: vec![step("nick", true)],
        }),
    );
    assert_eq!(code(&has), "principal.nick() != null");
}

#[test]
fn the_principal_without_a_scope_is_refused() {
    let ir = veld_emit_conformance::compile("rule_expr", SOURCE);
    let tree = Tree::new(&ir, "g", "g.runtime", vec![]);
    let mut fc = FileCtx::new(&tree, "g.app", "Handler");
    let mut printer = JavaExpr::for_endpoint(&mut fc, EndpointScope::default());
    let qty = path(Root::Principal, vec![step("qty", false)], sc(Scalar::Int));
    print_expr(&mut printer, &qty);
    assert_eq!(
        printer.unsupported().len(),
        1,
        "{:?}",
        printer.unsupported()
    );
}

#[test]
fn result_is_the_value_the_handler_returned() {
    let qty = path(Root::Result, vec![step("qty", false)], sc(Scalar::Int));
    assert_eq!(code(&qty), "result.qty()");
    let stamp = path(
        Root::Result,
        vec![step("stamp", false)],
        sc(Scalar::Datetime),
    );
    assert_eq!(code(&stamp), "Rules.instantMs(result.stamp())");
}

#[test]
fn old_reads_a_saved_slot_and_the_operand_is_kept_once() {
    let qty = || fact("line", vec![step("qty", false)], sc(Scalar::Int));
    let old = |e: Expr| ex(sc(Scalar::Int), ExprKind::Old(Box::new(e)));
    let same = bin(
        Op::Eq,
        path(Root::Result, vec![step("qty", false)], sc(Scalar::Int)),
        old(qty()),
        sc(Scalar::Bool),
    );
    let (printed, unsupported, olds) = print(&same);
    assert!(unsupported.is_empty(), "{unsupported:?}");
    assert_eq!(printed, "result.qty() == (int) o0.get()");
    assert_eq!(olds.len(), 1);
    assert_eq!(olds[0].0, "fact_line.qty()");
    // The same operand twice is one slot; another operand is the next one.
    let twice = bin(Op::Add, old(qty()), old(qty()), sc(Scalar::Int));
    let (printed, _, olds) = print(&twice);
    assert_eq!(printed, "Rules.addInt((int) o0.get(), (int) o0.get())");
    assert_eq!(olds.len(), 1);
    let other = bin(
        Op::Add,
        old(qty()),
        old(param("body", vec![step("qty", false)], sc(Scalar::Int))),
        sc(Scalar::Int),
    );
    let (printed, _, olds) = print(&other);
    assert_eq!(printed, "Rules.addInt((int) o0.get(), (int) o1.get())");
    assert_eq!(olds.len(), 2);
}

#[test]
fn a_plain_path_is_given_to_a_loader_as_the_developer_sees_it() {
    let ir = veld_emit_conformance::compile("rule_expr", SOURCE);
    let services = ir.services.iter().map(|s| s.id).collect();
    let tree = Tree::new(&ir, "g", "g.runtime", services);
    let mut fc = FileCtx::new(&tree, "g.app", "Handler");
    let scope = EndpointScope {
        params: vec![var(
            "request.body()",
            "body",
            Type::Decl(ir.decls.iter().find(|d| d.name == "Line").unwrap().id),
            false,
        )],
        ..EndpointScope::default()
    };
    let mut printer = JavaExpr::for_endpoint(&mut fc, scope);
    let start = param("body", vec![step("stamp", false)], sc(Scalar::Datetime));
    assert_eq!(
        printer.app_value(&start).as_deref(),
        Some("request.body().stamp()"),
        "an Instant, not epoch milliseconds"
    );
    let later = bin(
        Op::Add,
        start,
        ex(ExprType::Duration, ExprKind::Lit(ExprLit::Int(1))),
        sc(Scalar::Datetime),
    );
    assert_eq!(
        printer.app_value(&later).as_deref(),
        Some("Instant.ofEpochMilli(Rules.addLong(Rules.instantMs(request.body().stamp()), 1))")
    );
}

#[test]
fn names_that_are_not_in_scope_are_refused() {
    // The principal is a root of the scope now; without one it is refused (see
    // `the_principal_without_a_scope_is_refused`).
    let unknown = fact("nothing", vec![], sc(Scalar::Int));
    assert_eq!(print(&unknown).1.len(), 1);
}

#[test]
fn a_fact_key_that_reads_an_optional_value_is_loaded_only_when_it_is_there() {
    let ir = veld_emit_conformance::compile("rule_expr", SOURCE);
    let services = ir.services.iter().map(|s| s.id).collect();
    let tree = Tree::new(&ir, "g", "g.runtime", services);
    let mut fc = FileCtx::new(&tree, "g.app", "Handler");
    let line = Type::Decl(ir.decls.iter().find(|d| d.name == "Line").unwrap().id);
    let scope = EndpointScope {
        params: vec![var("request.body()", "body", line, false)],
        ..EndpointScope::default()
    };
    let mut printer = JavaExpr::for_endpoint(&mut fc, scope);
    let key = param("body", vec![step("nick", true)], sc(Scalar::String));
    assert_eq!(
        printer.presence(&key).as_deref(),
        Some("request.body().nick() != null")
    );
    let sure = param("body", vec![step("qty", false)], sc(Scalar::Int));
    assert_eq!(printer.presence(&sure), None);
}

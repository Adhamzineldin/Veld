//! Exact output of the Rust printer for every kind of expression and operand type, built from
//! hand-made IR.

use veld_codegen::{ExprPrinter, print_expr};
use veld_ir::{
    BinExpr, Builtin, CallExpr, EnumLit, Expr, ExprKind, ExprLit, ExprType, Op, PathRef, QuantExpr,
    Quantifier, Root, Scalar, Step, Type,
};

use super::RustPrinter;
use crate::tree::Tree;
use crate::types::Ctx;

fn ty(s: Scalar) -> ExprType {
    ExprType::Type(Type::Scalar(s))
}

fn x(t: ExprType, e: ExprKind) -> Expr {
    Expr { t, e }
}

fn int(n: i64) -> Expr {
    x(ty(Scalar::Int), ExprKind::Lit(ExprLit::Int(n)))
}

fn step(name: &str, optional: bool) -> Step {
    Step {
        name: name.into(),
        wire: name.into(),
        optional,
    }
}

fn field(t: ExprType, name: &str, optional: bool) -> Expr {
    x(
        t,
        ExprKind::Path(PathRef {
            root: Root::SelfValue,
            steps: vec![step(name, optional)],
        }),
    )
}

fn bin(op: Op, t: ExprType, l: Expr, r: Expr) -> Expr {
    x(t, ExprKind::Bin(Box::new(BinExpr { op, l, r })))
}

fn show(e: &Expr) -> String {
    let ir = veld_emit_conformance::ir_with_rule();
    let tree = Tree::new(&ir, "crate", "src", vec![], true);
    let ctx = Ctx {
        tree: &tree,
        here: "crate::app::greeter",
    };
    let mut p = RustPrinter::new(&ctx);
    let out = print_expr(&mut p, e).code;
    assert!(p.unsupported.is_empty(), "{:?}", p.unsupported);
    out
}

fn boolean() -> ExprType {
    ty(Scalar::Bool)
}

#[test]
fn literals_take_the_type_of_their_node() {
    assert_eq!(show(&int(5)), "5_i32");
    assert_eq!(show(&int(-5)), "-5_i32");
    let long = x(ty(Scalar::Long), ExprKind::Lit(ExprLit::Int(7)));
    assert_eq!(show(&long), "7_i64");
    let float = x(ty(Scalar::Float), ExprKind::Lit(ExprLit::Float(0.5)));
    assert_eq!(show(&float), "0.5f64");
    let dec = x(
        ty(Scalar::Decimal),
        ExprKind::Lit(ExprLit::Decimal("1.50".into())),
    );
    assert_eq!(show(&dec), "vr::dec(\"1.50\")?");
    let widened = x(ty(Scalar::Decimal), ExprKind::Lit(ExprLit::Int(3)));
    assert_eq!(show(&widened), "vr::dec(\"3\")?");
    let s = x(
        ty(Scalar::String),
        ExprKind::Lit(ExprLit::String("a\"b".into())),
    );
    assert_eq!(show(&s), "\"a\\\"b\"");
    assert_eq!(
        show(&x(boolean(), ExprKind::Lit(ExprLit::Bool(true)))),
        "true"
    );
}

#[test]
fn fields_are_read_by_their_type() {
    let cases = [
        (Scalar::Int, "self.n"),
        (Scalar::Bool, "self.n"),
        (Scalar::String, "self.n.as_str()"),
        (Scalar::Uuid, "self.n.as_str()"),
        (Scalar::Decimal, "vr::dec(&self.n)?"),
        (Scalar::Datetime, "vr::datetime(&self.n)?"),
        (Scalar::Date, "vr::date(&self.n)?"),
        (Scalar::Time, "vr::time(&self.n)?"),
        (Scalar::Bytes, "&self.n"),
    ];
    for (scalar, want) in cases {
        assert_eq!(show(&field(ty(scalar), "n", false)), want, "{scalar:?}");
    }
}

#[test]
fn an_optional_step_is_read_through_a_helper_that_cannot_panic() {
    assert_eq!(
        show(&field(ty(Scalar::Int), "age", true)),
        "(*vr::some(&self.age)?)"
    );
    assert_eq!(
        show(&field(ty(Scalar::String), "nick", true)),
        "(*vr::some(&self.nick)?).as_str()"
    );
    let nested = x(
        ty(Scalar::String),
        ExprKind::Path(PathRef {
            root: Root::SelfValue,
            steps: vec![step("address", true), step("zip", false)],
        }),
    );
    assert_eq!(show(&nested), "vr::some(&self.address)?.zip.as_str()");
}

#[test]
fn keyword_field_names_are_raw_identifiers() {
    assert_eq!(show(&field(ty(Scalar::Int), "type", false)), "self.r#type");
}

#[test]
fn has_checks_each_optional_step() {
    let has = |steps: Vec<Step>| {
        x(
            boolean(),
            ExprKind::Has(PathRef {
                root: Root::SelfValue,
                steps,
            }),
        )
    };
    assert_eq!(show(&has(vec![step("a", true)])), "self.a.is_some()");
    assert_eq!(
        show(&has(vec![step("a", true), step("b", true)])),
        "self.a.as_ref().is_some_and(|h0| h0.b.is_some())"
    );
    assert_eq!(
        show(&has(vec![step("a", true), step("b", false)])),
        "self.a.is_some()"
    );
}

#[test]
fn logic_is_lazy_and_parenthesised_by_precedence() {
    let a = field(boolean(), "a", false);
    let b = field(boolean(), "b", false);
    let c = field(boolean(), "c", false);
    let or = bin(Op::Or, boolean(), a.clone(), b.clone());
    assert_eq!(
        show(&bin(Op::And, boolean(), or, c.clone())),
        "(self.a || self.b) && self.c"
    );
    assert_eq!(
        show(&bin(Op::Implies, boolean(), a.clone(), b)),
        "!self.a || self.b"
    );
    let not = x(
        boolean(),
        ExprKind::Not(Box::new(bin(Op::And, boolean(), a, c))),
    );
    assert_eq!(show(&not), "!(self.a && self.c)");
}

#[test]
fn comparisons_are_chosen_by_the_operand_type() {
    let cmp = |op, s: Scalar| {
        bin(
            op,
            boolean(),
            field(ty(s), "a", false),
            field(ty(s), "b", false),
        )
    };
    assert_eq!(show(&cmp(Op::Lt, Scalar::Int)), "self.a < self.b");
    assert_eq!(show(&cmp(Op::Eq, Scalar::Long)), "self.a == self.b");
    assert_eq!(
        show(&cmp(Op::Ne, Scalar::String)),
        "self.a.as_str() != self.b.as_str()"
    );
    assert_eq!(
        show(&cmp(Op::Ge, Scalar::Decimal)),
        "vr::dec(&self.a)?.compare(&vr::dec(&self.b)?).is_ge()"
    );
    assert_eq!(
        show(&cmp(Op::Eq, Scalar::Decimal)),
        "vr::dec(&self.a)?.compare(&vr::dec(&self.b)?).is_eq()"
    );
    assert_eq!(
        show(&cmp(Op::Eq, Scalar::Uuid)),
        "vr::text::uuid_eq(self.a.as_str(), self.b.as_str())"
    );
    assert_eq!(
        show(&cmp(Op::Lt, Scalar::Datetime)),
        "vr::datetime(&self.a)? < vr::datetime(&self.b)?"
    );
}

#[test]
fn arithmetic_is_checked_by_type() {
    let arith = |op, s: Scalar| {
        bin(
            op,
            ty(s),
            field(ty(s), "a", false),
            field(ty(s), "b", false),
        )
    };
    assert_eq!(
        show(&arith(Op::Add, Scalar::Int)),
        "vr::num::add_i32(self.a, self.b)?"
    );
    assert_eq!(
        show(&arith(Op::Div, Scalar::Long)),
        "vr::num::div_i64(self.a, self.b)?"
    );
    assert_eq!(
        show(&arith(Op::Rem, Scalar::Int)),
        "vr::num::rem_i32(self.a, self.b)?"
    );
    assert_eq!(
        show(&arith(Op::Mul, Scalar::Float)),
        "vr::num::mul_f64(self.a, self.b)?"
    );
    assert_eq!(
        show(&arith(Op::Sub, Scalar::Decimal)),
        "vr::dec(&self.a)?.sub(&vr::dec(&self.b)?)"
    );
    let neg = |s: Scalar| x(ty(s), ExprKind::Neg(Box::new(field(ty(s), "a", false))));
    assert_eq!(show(&neg(Scalar::Int)), "vr::num::neg_i32(self.a)?");
    assert_eq!(show(&neg(Scalar::Float)), "-self.a");
    assert_eq!(show(&neg(Scalar::Decimal)), "vr::dec(&self.a)?.negated()");
}

#[test]
fn dates_and_durations() {
    let d = ExprType::Duration;
    let dt = |n: &str| field(ty(Scalar::Datetime), n, false);
    let span = bin(Op::Sub, d.clone(), dt("end"), dt("start"));
    assert_eq!(
        show(&span),
        "vr::num::sub_i64(vr::datetime(&self.end)?, vr::datetime(&self.start)?)?"
    );
    let days = x(
        d.clone(),
        ExprKind::Call(CallExpr {
            f: Builtin::Days,
            args: vec![int(30)],
        }),
    );
    assert_eq!(show(&days), "vr::num::days_ms(i64::from(30_i32))?");
    let le = bin(Op::Le, boolean(), span, days);
    assert!(show(&le).contains(" <= vr::num::days_ms("));
    let dates = bin(
        Op::Sub,
        d,
        field(ty(Scalar::Date), "a", false),
        field(ty(Scalar::Date), "b", false),
    );
    assert_eq!(
        show(&dates),
        "vr::date_diff(vr::date(&self.a)?, vr::date(&self.b)?)?"
    );
}

#[test]
fn widen_makes_every_conversion_explicit() {
    let w = |from: Scalar, to: Scalar| {
        x(
            ty(to),
            ExprKind::Widen(Box::new(field(ty(from), "a", false))),
        )
    };
    assert_eq!(show(&w(Scalar::Int, Scalar::Long)), "i64::from(self.a)");
    assert_eq!(show(&w(Scalar::Int, Scalar::Float)), "f64::from(self.a)");
    assert_eq!(
        show(&w(Scalar::Int, Scalar::Decimal)),
        "vr::Decimal::from_i64(i64::from(self.a))"
    );
    assert_eq!(
        show(&w(Scalar::Long, Scalar::Decimal)),
        "vr::Decimal::from_i64(self.a)"
    );
}

#[test]
fn builtins() {
    let call = |f, args: Vec<Expr>, t: ExprType| x(t, ExprKind::Call(CallExpr { f, args }));
    let s = field(ty(Scalar::String), "s", false);
    assert_eq!(
        show(&call(Builtin::Len, vec![s.clone()], ty(Scalar::Int))),
        "vr::len_str(self.s.as_str())?"
    );
    let pat = x(
        ty(Scalar::String),
        ExprKind::Lit(ExprLit::String("^a".into())),
    );
    assert_eq!(
        show(&call(Builtin::Matches, vec![s.clone(), pat], boolean())),
        "vr::matches(self.s.as_str(), \"^a\")?"
    );
    assert_eq!(
        show(&call(Builtin::StartsWith, vec![s.clone(), s], boolean())),
        "vr::text::starts_with(self.s.as_str(), self.s.as_str())"
    );
    let big = x(ty(Scalar::Long), ExprKind::Lit(ExprLit::Long(5)));
    assert_eq!(
        show(&call(Builtin::Hours, vec![big], ExprType::Duration)),
        "vr::num::hours_ms(5_i64)?"
    );
}

#[test]
fn lists_membership_and_comprehensions() {
    let list_ty = ExprType::Type(Type::List(Box::new(Type::Scalar(Scalar::String))));
    let items = field(list_ty.clone(), "tags", false);
    assert_eq!(
        show(&x(
            ty(Scalar::Int),
            ExprKind::Call(CallExpr {
                f: Builtin::Len,
                args: vec![items.clone()]
            })
        )),
        "vr::len_n(vr::list(&self.tags, |e| Ok((*e).as_str()))?.len())?"
    );
    let member = bin(
        Op::In,
        boolean(),
        field(ty(Scalar::String), "kind", false),
        x(
            list_ty.clone(),
            ExprKind::List(vec![
                x(
                    ty(Scalar::String),
                    ExprKind::Lit(ExprLit::String("a".into())),
                ),
                x(
                    ty(Scalar::String),
                    ExprKind::Lit(ExprLit::String("b".into())),
                ),
            ]),
        ),
    );
    assert_eq!(
        show(&member),
        "{ let needle = self.kind.as_str(); vr::contains(&vec![\"a\", \"b\"], |e| needle == (*e)) }"
    );
    let var = |n: &str| PathRef {
        root: Root::Var(n.into()),
        steps: vec![],
    };
    let body = bin(
        Op::Gt,
        boolean(),
        x(ty(Scalar::String), ExprKind::Path(var("t"))),
        x(ty(Scalar::String), ExprKind::Path(var("t"))),
    );
    let _ = body;
    let ints = ExprType::Type(Type::List(Box::new(Type::Scalar(Scalar::Int))));
    let q = x(
        boolean(),
        ExprKind::Quant(Box::new(QuantExpr {
            q: Quantifier::All,
            list: field(ints.clone(), "nums", false),
            var: "n".into(),
            body: bin(
                Op::Gt,
                boolean(),
                x(ty(Scalar::Int), ExprKind::Path(var("n"))),
                int(0),
            ),
        })),
    );
    assert_eq!(
        show(&q),
        "vr::all(&vr::list(&self.nums, |e| Ok((*e)))?, |v_n| -> vr::Res<bool> { Ok(*v_n > 0_i32) })?"
    );
    let sum = x(
        ty(Scalar::Int),
        ExprKind::Quant(Box::new(QuantExpr {
            q: Quantifier::Sum,
            list: field(ints, "nums", false),
            var: "n".into(),
            body: x(ty(Scalar::Int), ExprKind::Path(var("n"))),
        })),
    );
    assert_eq!(
        show(&sum),
        "vr::sum_i32(&vr::list(&self.nums, |e| Ok((*e)))?, |v_n| -> vr::Res<i32> { Ok(*v_n) })?"
    );
}

#[test]
fn an_enum_literal_is_a_path_to_its_variant() {
    let ir = veld_emit_conformance::ir_with_rule();
    let Some(decl) = ir.decls.iter().find(|d| d.name == "Greeting") else {
        panic!("no Greeting");
    };
    let e = x(
        ExprType::Type(Type::Decl(decl.id)),
        ExprKind::Lit(ExprLit::Enum(EnumLit {
            decl: decl.id,
            name: "paid".into(),
            wire: "PAID".into(),
        })),
    );
    assert_eq!(show(&e), "crate::app::plain::Greeting::Paid");
}

#[test]
fn what_it_cannot_express_is_reported_not_printed() {
    let ir = veld_emit_conformance::ir_with_rule();
    let tree = Tree::new(&ir, "crate", "src", vec![], true);
    let ctx = Ctx {
        tree: &tree,
        here: "crate::app::greeter",
    };
    let mut p = RustPrinter::new(&ctx);
    let _ = p.now();
    assert_eq!(p.unsupported.len(), 1);
}

// ---- the roots of an endpoint block ------------------------------------------------------------

fn block_scope() -> super::Scope {
    super::Scope {
        params: [("body".to_string(), false), ("limit".to_string(), true)]
            .into_iter()
            .collect(),
        facts: [("room".to_string(), false), ("coupon".to_string(), true)]
            .into_iter()
            .collect(),
        result: true,
        ..super::Scope::default()
    }
}

fn from_root(root: Root, t: ExprType, steps: Vec<Step>) -> Expr {
    x(t, ExprKind::Path(PathRef { root, steps }))
}

fn show_block(e: &Expr) -> String {
    let ir = veld_emit_conformance::ir_with_rule();
    let tree = Tree::new(&ir, "crate", "src", vec![], true);
    let ctx = Ctx {
        tree: &tree,
        here: "crate::app::greeter",
    };
    let mut p = RustPrinter::for_block(&ctx, block_scope());
    let out = print_expr(&mut p, &super::mark_old(e)).code;
    assert!(p.unsupported.is_empty(), "{:?}", p.unsupported);
    out
}

fn now() -> Expr {
    x(ty(Scalar::Datetime), ExprKind::Now(veld_ir::Always))
}

#[test]
fn a_parameter_is_read_from_the_request_and_a_fact_from_its_local() {
    let guests = from_root(
        Root::Param("body".into()),
        ty(Scalar::Int),
        vec![step("guests", false)],
    );
    assert_eq!(show_block(&guests), "p.body.guests");
    let capacity = from_root(
        Root::Fact("room".into()),
        ty(Scalar::Int),
        vec![step("capacity", false)],
    );
    assert_eq!(show_block(&capacity), "(*fact_room).capacity");
    let both = bin(Op::Le, boolean(), guests, capacity);
    assert_eq!(show_block(&both), "p.body.guests <= (*fact_room).capacity");
    // A decimal is parsed where it is read, a datetime too.
    let amount = from_root(
        Root::Param("body".into()),
        ty(Scalar::Decimal),
        vec![step("amount", false)],
    );
    assert_eq!(show_block(&amount), "vr::dec(&p.body.amount)?");
    let start = from_root(
        Root::Param("body".into()),
        ty(Scalar::Datetime),
        vec![step("start", false)],
    );
    let late = bin(Op::Ge, boolean(), start, now());
    assert_eq!(show_block(&late), "vr::datetime(&p.body.start)? >= now");
}

#[test]
fn an_optional_parameter_or_fact_is_read_through_a_helper_and_tested_with_has() {
    let limit = from_root(Root::Param("limit".into()), ty(Scalar::Int), vec![]);
    assert_eq!(show_block(&limit), "(*vr::some(&p.limit)?)");
    let has_limit = x(
        boolean(),
        ExprKind::Has(PathRef {
            root: Root::Param("limit".into()),
            steps: vec![],
        }),
    );
    assert_eq!(show_block(&has_limit), "p.limit.is_some()");
    let percent = from_root(
        Root::Fact("coupon".into()),
        ty(Scalar::Int),
        vec![step("percent", false)],
    );
    assert_eq!(show_block(&percent), "vr::some(&(*fact_coupon))?.percent");
    let has_coupon = x(
        boolean(),
        ExprKind::Has(PathRef {
            root: Root::Fact("coupon".into()),
            steps: vec![],
        }),
    );
    assert_eq!(show_block(&has_coupon), "(*fact_coupon).is_some()");
    // An optional step under an optional root is checked at both levels.
    let deep = x(
        boolean(),
        ExprKind::Has(PathRef {
            root: Root::Fact("coupon".into()),
            steps: vec![step("note", true)],
        }),
    );
    assert_eq!(
        show_block(&deep),
        "(*fact_coupon).as_ref().is_some_and(|h0| h0.note.is_some())"
    );
}

#[test]
fn the_result_and_now_are_in_scope_of_an_ensure() {
    let total = from_root(Root::Result, ty(Scalar::Int), vec![step("total", false)]);
    assert_eq!(show_block(&total), "(*r).total");
    let whole = from_root(Root::Result, ty(Scalar::Int), vec![]);
    assert_eq!(show_block(&whole), "(*r)");
    assert_eq!(show_block(&now()), "now");
}

#[test]
fn old_reads_the_saved_copy_of_the_parameters_and_facts() {
    let nightly = from_root(
        Root::Fact("room".into()),
        ty(Scalar::Decimal),
        vec![step("nightly", false)],
    );
    let old = x(ty(Scalar::Decimal), ExprKind::Old(Box::new(nightly)));
    assert_eq!(show_block(&old), "vr::dec(&(*old_fact_room).nightly)?");
    let guests = from_root(
        Root::Param("body".into()),
        ty(Scalar::Int),
        vec![step("guests", false)],
    );
    let old_guests = x(ty(Scalar::Int), ExprKind::Old(Box::new(guests.clone())));
    assert_eq!(show_block(&old_guests), "old_p.body.guests");
    // Outside `old(...)` the same names read the live values; one expression may hold both.
    let same = bin(Op::Eq, boolean(), guests.clone(), old_guests);
    assert_eq!(show_block(&same), "p.body.guests == old_p.body.guests");
    // The operand of `old` may be a computation.
    let sum = bin(Op::Add, ty(Scalar::Int), guests, int(1));
    let old_sum = x(ty(Scalar::Int), ExprKind::Old(Box::new(sum)));
    assert_eq!(
        show_block(&old_sum),
        "vr::num::add_i32(old_p.body.guests, 1_i32)?"
    );
}

#[test]
fn a_key_reads_an_absent_value_through_the_helper_that_means_no_fact() {
    let ir = veld_emit_conformance::ir_with_rule();
    let tree = Tree::new(&ir, "crate", "src", vec![], true);
    let ctx = Ctx {
        tree: &tree,
        here: "crate::app::greeter",
    };
    let code = from_root(
        Root::Param("body".into()),
        ty(Scalar::String),
        vec![step("couponCode", true)],
    );
    let mut p = RustPrinter::for_block(&ctx, block_scope()).reading_absent();
    assert_eq!(
        p.raw_place(&code).as_deref(),
        Some("(*vr::absent(&p.body.coupon_code)?)")
    );
    // A bare path of a required value is its place, to be cloned as it is.
    let id = from_root(
        Root::Param("body".into()),
        ty(Scalar::Uuid),
        vec![step("roomId", false)],
    );
    let mut plain = RustPrinter::for_block(&ctx, block_scope());
    assert_eq!(plain.raw_place(&id).as_deref(), Some("p.body.room_id"));
    // Anything else is not a place.
    assert_eq!(plain.raw_place(&int(1)), None);
}

#[test]
fn a_name_outside_the_block_the_principal_and_old_in_an_invariant_are_reported() {
    let ir = veld_emit_conformance::ir_with_rule();
    let tree = Tree::new(&ir, "crate", "src", vec![], true);
    let ctx = Ctx {
        tree: &tree,
        here: "crate::app::greeter",
    };
    for root in [Root::Principal, Root::Param("nobody".into())] {
        let mut p = RustPrinter::for_block(&ctx, block_scope());
        let e = from_root(root, ty(Scalar::Int), vec![]);
        let _ = print_expr(&mut p, &e);
        assert_eq!(p.unsupported.len(), 1);
    }
    let mut p = RustPrinter::new(&ctx);
    let old = x(ty(Scalar::Int), ExprKind::Old(Box::new(int(1))));
    let _ = print_expr(&mut p, &old);
    assert_eq!(p.unsupported.len(), 1);
}

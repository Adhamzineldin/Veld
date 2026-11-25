use veld_ir::{
    BinExpr, CallExpr, Const, Decl, DeclId, DeclKind, Enum, EnumLit, ErrorDecl, IR_VERSION, Ir,
    Literal, Model, Origin, PackageId, QuantExpr, Step, Variant,
};

use super::*;
use veld_ir::ExprKind;

const NS: &str = "r";

// ---- IR builders -------------------------------------------------------------------------

fn sc(s: Scalar) -> ExprType {
    ExprType::Type(Type::Scalar(s))
}
fn list_of(s: Scalar) -> ExprType {
    ExprType::Type(Type::List(Box::new(Type::Scalar(s))))
}
fn ex(t: ExprType, e: ExprKind) -> Expr {
    Expr { t, e }
}
fn lit(t: ExprType, l: ExprLit) -> Expr {
    ex(t, ExprKind::Lit(l))
}
fn int(n: i64) -> Expr {
    lit(sc(Scalar::Int), ExprLit::Int(n))
}
fn boolean(b: bool) -> Expr {
    lit(sc(Scalar::Bool), ExprLit::Bool(b))
}
fn mk_steps(list: &[(&str, bool)]) -> Vec<Step> {
    list.iter()
        .map(|(n, o)| Step {
            name: (*n).into(),
            wire: (*n).into(),
            optional: *o,
        })
        .collect()
}
fn path(root: Root, list: &[(&str, bool)], t: ExprType) -> Expr {
    ex(
        t,
        ExprKind::Path(PathRef {
            root,
            steps: mk_steps(list),
        }),
    )
}
fn field(name: &str, t: ExprType) -> Expr {
    path(Root::SelfValue, &[(name, false)], t)
}
fn opt_field(name: &str, t: ExprType) -> Expr {
    path(Root::SelfValue, &[(name, true)], t)
}
fn var(name: &str, t: ExprType) -> Expr {
    path(Root::Var(name.into()), &[], t)
}
fn bin(op: Op, l: Expr, r: Expr, t: ExprType) -> Expr {
    ex(t, ExprKind::Bin(Box::new(BinExpr { op, l, r })))
}
fn logic(op: Op, l: Expr, r: Expr) -> Expr {
    bin(op, l, r, sc(Scalar::Bool))
}
fn call(f: Builtin, args: Vec<Expr>, t: ExprType) -> Expr {
    ex(t, ExprKind::Call(CallExpr { f, args }))
}
fn quant(q: Quantifier, list: Expr, var: &str, body: Expr, t: ExprType) -> Expr {
    let node = QuantExpr {
        q,
        list,
        var: var.into(),
        body,
    };
    ex(t, ExprKind::Quant(Box::new(node)))
}
fn widen(x: Expr, t: ExprType) -> Expr {
    ex(t, ExprKind::Widen(Box::new(x)))
}

fn origin() -> Origin {
    Origin {
        file: "t.veld".into(),
        line: 1,
        col: 1,
    }
}

fn decl(i: u32, name: &str, kind: DeclKind) -> Decl {
    Decl {
        id: DeclId(i),
        package: PackageId(0),
        module: "m".into(),
        name: name.into(),
        qualified: format!("m.{name}"),
        sig: String::new(),
        docs: None,
        deprecated: None,
        origin: origin(),
        kind,
    }
}

/// An IR with an enum (`0`), a model (`1`) and an error (`2`).
fn ir() -> Ir {
    let model = || Model {
        parent: None,
        fields: vec![],
        rules: vec![],
    };
    let variant = |name: &str, wire: &str| Variant {
        name: name.into(),
        wire: wire.into(),
        docs: None,
        deprecated: None,
    };
    let decls = vec![
        decl(
            0,
            "Status",
            DeclKind::Enum(Enum {
                variants: vec![variant("paid", "PAID"), variant("checkedIn", "CHECKED_IN")],
            }),
        ),
        decl(1, "Item", DeclKind::Model(model())),
        decl(
            2,
            "Err",
            DeclKind::Error(ErrorDecl {
                default_status: None,
                fields: model(),
            }),
        ),
        decl(
            3,
            "C",
            DeclKind::Const(Const {
                ty: Type::Scalar(Scalar::Int),
                value: Literal::Int(1),
            }),
        ),
    ];
    Ir {
        ir_version: IR_VERSION.into(),
        packages: vec![],
        decls,
        services: vec![],
        consumes: Default::default(),
    }
}

fn show(e: &Expr) -> String {
    print(&ir(), NS, e)
}

fn status() -> ExprType {
    ExprType::Type(Type::Decl(DeclId(0)))
}

fn paid() -> Expr {
    lit(
        status(),
        ExprLit::Enum(EnumLit {
            decl: DeclId(0),
            name: "paid".into(),
            wire: "PAID".into(),
        }),
    )
}

// ---- literals ----------------------------------------------------------------------------

#[test]
fn literals_follow_the_type_of_the_node() {
    assert_eq!(show(&boolean(true)), "true");
    assert_eq!(show(&int(5)), "5");
    assert_eq!(show(&int(-5)), "-5");
    assert_eq!(show(&lit(sc(Scalar::Long), ExprLit::Int(5))), "5n");
    assert_eq!(show(&lit(sc(Scalar::Long), ExprLit::Long(-9))), "-9n");
    assert_eq!(show(&lit(sc(Scalar::Float), ExprLit::Int(2))), "2");
    assert_eq!(show(&lit(sc(Scalar::Float), ExprLit::Float(0.5))), "0.5");
    assert_eq!(
        show(&lit(sc(Scalar::Float), ExprLit::Float(1.0e308))),
        "1e+308"
    );
    assert_eq!(
        show(&lit(sc(Scalar::Decimal), ExprLit::Int(7))),
        "r.dec('7')"
    );
    assert_eq!(
        show(&lit(sc(Scalar::Decimal), ExprLit::Decimal("120.00".into()))),
        "r.dec('120.00')"
    );
    let text = lit(sc(Scalar::String), ExprLit::String("it's".into()));
    assert_eq!(show(&text), "'it\\'s'");
    assert_eq!(show(&paid()), "'PAID'");
}

// ---- paths -------------------------------------------------------------------------------

#[test]
fn a_required_step_goes_through_required_and_an_optional_one_is_asserted() {
    assert_eq!(show(&field("a", sc(Scalar::Int))), "r.required(self.a)");
    assert_eq!(show(&opt_field("nick", sc(Scalar::String))), "self.nick!");
    let nested = path(
        Root::SelfValue,
        &[("address", true), ("zip", false)],
        sc(Scalar::String),
    );
    assert_eq!(show(&nested), "r.required(self.address!.zip)");
    let quoted = field("odd-name", sc(Scalar::Int));
    assert_eq!(show(&quoted), "r.required(self['odd-name'])");
}

#[test]
fn a_read_converts_at_the_leaf() {
    let read = |name: &str, s: Scalar| show(&field(name, sc(s)));
    assert_eq!(read("n", Scalar::Long), "r.toLong(r.required(self.n))");
    assert_eq!(read("p", Scalar::Decimal), "r.dec(r.required(self.p))");
    assert_eq!(read("d", Scalar::Date), "r.toDate(r.required(self.d))");
    assert_eq!(
        read("t", Scalar::Datetime),
        "r.toDatetime(r.required(self.t))"
    );
    assert_eq!(read("c", Scalar::Time), "r.toTime(r.required(self.c))");
    assert_eq!(read("u", Scalar::Uuid), "r.toUuid(r.required(self.u))");
    for plain in [
        Scalar::Int,
        Scalar::Float,
        Scalar::Bool,
        Scalar::String,
        Scalar::Bytes,
    ] {
        assert_eq!(read("x", plain), "r.required(self.x)", "{plain:?}");
    }
    let optional_long = opt_field("n", sc(Scalar::Long));
    assert_eq!(show(&optional_long), "r.toLong(self.n!)");
}

#[test]
fn a_list_read_converts_its_elements_once() {
    let longs = field("ids", list_of(Scalar::Long));
    assert_eq!(
        show(&longs),
        "r.required(self.ids).map((e0) => r.toLong(e0))"
    );
    assert_eq!(
        show(&field("tags", list_of(Scalar::String))),
        "r.required(self.tags)"
    );
    let nested = field(
        "grid",
        ExprType::Type(Type::List(Box::new(Type::List(Box::new(Type::Scalar(
            Scalar::Decimal,
        )))))),
    );
    assert_eq!(
        show(&nested),
        "r.required(self.grid).map((e0) => e0.map((e1) => r.dec(e1)))"
    );
}

#[test]
fn a_lambda_variable_over_scalars_is_a_bare_identifier() {
    let body = bin(
        Op::Gt,
        var("p", sc(Scalar::Long)),
        lit(sc(Scalar::Long), ExprLit::Long(0)),
        sc(Scalar::Bool),
    );
    let all = quant(
        Quantifier::All,
        field("ids", list_of(Scalar::Long)),
        "p",
        body,
        sc(Scalar::Bool),
    );
    assert_eq!(
        show(&all),
        "r.required(self.ids).map((e0) => r.toLong(e0)).every((l_p) => l_p > 0n)"
    );
}

#[test]
fn a_step_of_a_lambda_variable_converts_at_the_leaf() {
    let price = path(
        Root::Var("i".into()),
        &[("price", false)],
        sc(Scalar::Decimal),
    );
    assert_eq!(show(&price), "r.dec(r.required(l_i.price))");
}

#[test]
fn roots_other_than_self_and_var_are_unavailable() {
    let e = path(Root::Result, &[("a", false)], sc(Scalar::Int));
    assert_eq!(show(&e), "r.unavailable('a root other than `self`')");
    let now = ex(sc(Scalar::Datetime), ExprKind::Now(veld_ir::Always));
    assert_eq!(show(&now), "r.unavailable('now')");
    let old = ex(sc(Scalar::Int), ExprKind::Old(Box::new(int(1))));
    assert_eq!(show(&old), "r.unavailable('old')");
}

// ---- has ---------------------------------------------------------------------------------

#[test]
fn has_checks_every_optional_step_and_never_reads_an_absent_one() {
    let has = |steps: &[(&str, bool)]| {
        let p = PathRef {
            root: Root::SelfValue,
            steps: mk_steps(steps),
        };
        show(&ex(sc(Scalar::Bool), ExprKind::Has(p)))
    };
    assert_eq!(has(&[("nick", true)]), "self.nick !== undefined");
    assert_eq!(
        has(&[("address", true), ("zip", true)]),
        "self.address !== undefined && self.address!.zip !== undefined"
    );
    assert_eq!(
        has(&[("a", false), ("b", true)]),
        "r.required(self.a).b !== undefined"
    );
    assert_eq!(has(&[("a", false)]), "true");
}

// ---- operators ---------------------------------------------------------------------------

#[test]
fn logic_uses_native_operators_with_the_right_parentheses() {
    let a = || field("a", sc(Scalar::Bool));
    let b = || field("b", sc(Scalar::Bool));
    let c = || field("c", sc(Scalar::Bool));
    assert_eq!(
        show(&logic(Op::And, a(), b())),
        "r.required(self.a) && r.required(self.b)"
    );
    let mixed = logic(Op::Or, a(), logic(Op::And, b(), c()));
    assert_eq!(
        show(&mixed),
        "r.required(self.a) || r.required(self.b) && r.required(self.c)"
    );
    let grouped = logic(Op::And, logic(Op::Or, a(), b()), c());
    assert_eq!(
        show(&grouped),
        "(r.required(self.a) || r.required(self.b)) && r.required(self.c)"
    );
    let implies = logic(Op::Implies, a(), b());
    assert_eq!(show(&implies), "!r.required(self.a) || r.required(self.b)");
    let right = logic(Op::Implies, a(), logic(Op::Implies, b(), c()));
    assert_eq!(
        show(&right),
        "!r.required(self.a) || (!r.required(self.b) || r.required(self.c))"
    );
    let not = ex(
        sc(Scalar::Bool),
        ExprKind::Not(Box::new(logic(Op::Or, a(), b()))),
    );
    assert_eq!(show(&not), "!(r.required(self.a) || r.required(self.b))");
    let double = ex(
        sc(Scalar::Bool),
        ExprKind::Not(Box::new(ex(sc(Scalar::Bool), ExprKind::Not(Box::new(a()))))),
    );
    assert_eq!(show(&double), "!!r.required(self.a)");
}

#[test]
fn comparisons_are_native_except_for_decimals() {
    let cmp = |op: Op, s: Scalar| {
        let l = field("a", sc(s));
        let r = field("b", sc(s));
        show(&logic(op, l, r))
    };
    assert_eq!(
        cmp(Op::Eq, Scalar::Int),
        "r.required(self.a) === r.required(self.b)"
    );
    assert_eq!(
        cmp(Op::Ne, Scalar::String),
        "r.required(self.a) !== r.required(self.b)"
    );
    assert_eq!(
        cmp(Op::Lt, Scalar::Float),
        "r.required(self.a) < r.required(self.b)"
    );
    assert_eq!(
        cmp(Op::Le, Scalar::Bool),
        "r.required(self.a) <= r.required(self.b)"
    );
    assert_eq!(
        cmp(Op::Ge, Scalar::Long),
        "r.toLong(r.required(self.a)) >= r.toLong(r.required(self.b))"
    );
    assert_eq!(
        cmp(Op::Gt, Scalar::Datetime),
        "r.toDatetime(r.required(self.a)) > r.toDatetime(r.required(self.b))"
    );
    assert_eq!(
        cmp(Op::Eq, Scalar::Uuid),
        "r.toUuid(r.required(self.a)) === r.toUuid(r.required(self.b))"
    );
    assert_eq!(
        cmp(Op::Eq, Scalar::Decimal),
        "r.decCmp(r.dec(r.required(self.a)), r.dec(r.required(self.b))) === 0"
    );
    assert_eq!(
        cmp(Op::Lt, Scalar::Decimal),
        "r.decCmp(r.dec(r.required(self.a)), r.dec(r.required(self.b))) < 0"
    );
}

#[test]
fn a_comparison_of_comparisons_keeps_its_grouping() {
    let l = logic(Op::Lt, int(1), int(2));
    let r = logic(Op::Lt, int(3), int(4));
    assert_eq!(show(&logic(Op::Eq, l, r)), "1 < 2 === 3 < 4");
}

#[test]
fn arithmetic_is_always_a_checked_helper() {
    let arith = |op: Op, s: Scalar| {
        let l = field("a", sc(s));
        let r = field("b", sc(s));
        show(&bin(op, l, r, sc(s)))
    };
    assert_eq!(
        arith(Op::Add, Scalar::Int),
        "r.addInt(r.required(self.a), r.required(self.b))"
    );
    assert_eq!(
        arith(Op::Sub, Scalar::Int),
        "r.subInt(r.required(self.a), r.required(self.b))"
    );
    assert_eq!(
        arith(Op::Mul, Scalar::Int),
        "r.mulInt(r.required(self.a), r.required(self.b))"
    );
    assert_eq!(
        arith(Op::Div, Scalar::Int),
        "r.divInt(r.required(self.a), r.required(self.b))"
    );
    assert_eq!(
        arith(Op::Rem, Scalar::Int),
        "r.remInt(r.required(self.a), r.required(self.b))"
    );
    assert_eq!(
        arith(Op::Add, Scalar::Long),
        "r.addLong(r.toLong(r.required(self.a)), r.toLong(r.required(self.b)))"
    );
    assert_eq!(
        arith(Op::Div, Scalar::Long),
        "r.divLong(r.toLong(r.required(self.a)), r.toLong(r.required(self.b)))"
    );
    assert_eq!(
        arith(Op::Mul, Scalar::Float),
        "r.mulFloat(r.required(self.a), r.required(self.b))"
    );
    assert_eq!(
        arith(Op::Rem, Scalar::Float),
        "r.remFloat(r.required(self.a), r.required(self.b))"
    );
    assert_eq!(
        arith(Op::Add, Scalar::Decimal),
        "r.decAdd(r.dec(r.required(self.a)), r.dec(r.required(self.b)))"
    );
    assert_eq!(
        arith(Op::Mul, Scalar::Decimal),
        "r.decMul(r.dec(r.required(self.a)), r.dec(r.required(self.b)))"
    );
}

#[test]
fn temporal_arithmetic_uses_the_long_helpers() {
    let dt = || field("a", sc(Scalar::Datetime));
    let dur = || call(Builtin::Days, vec![int(30)], ExprType::Duration);
    let dt_ms = "r.toDatetime(r.required(self.a))";
    assert_eq!(
        show(&bin(Op::Add, dt(), dur(), sc(Scalar::Datetime))),
        format!("r.addLong({dt_ms}, r.days(30))")
    );
    assert_eq!(
        show(&bin(Op::Sub, dt(), dt(), ExprType::Duration)),
        format!("r.subLong({dt_ms}, {dt_ms})")
    );
    assert_eq!(
        show(&bin(Op::Add, dur(), dur(), ExprType::Duration)),
        "r.addLong(r.days(30), r.days(30))"
    );
    let d = || field("d", sc(Scalar::Date));
    assert_eq!(
        show(&bin(Op::Sub, d(), d(), ExprType::Duration)),
        "r.dateDiff(r.toDate(r.required(self.d)), r.toDate(r.required(self.d)))"
    );
    let cmp = logic(Op::Le, dur(), dur());
    assert_eq!(show(&cmp), "r.days(30) <= r.days(30)");
}

#[test]
fn negation_is_checked_where_it_can_overflow() {
    let neg = |x: Expr, t: ExprType| show(&ex(t, ExprKind::Neg(Box::new(x))));
    assert_eq!(
        neg(field("n", sc(Scalar::Int)), sc(Scalar::Int)),
        "r.negInt(r.required(self.n))"
    );
    assert_eq!(
        neg(field("n", sc(Scalar::Long)), sc(Scalar::Long)),
        "r.negLong(r.toLong(r.required(self.n)))"
    );
    assert_eq!(
        neg(field("n", sc(Scalar::Float)), sc(Scalar::Float)),
        "-r.required(self.n)"
    );
    assert_eq!(
        neg(
            lit(sc(Scalar::Float), ExprLit::Float(-1.5)),
            sc(Scalar::Float)
        ),
        "-(-1.5)"
    );
    assert_eq!(
        neg(field("n", sc(Scalar::Decimal)), sc(Scalar::Decimal)),
        "r.decNeg(r.dec(r.required(self.n)))"
    );
    let dur = call(Builtin::Hours, vec![int(1)], ExprType::Duration);
    assert_eq!(neg(dur, ExprType::Duration), "r.negLong(r.hours(1))");
}

#[test]
fn widening_converts_exactly() {
    let n = || field("n", sc(Scalar::Int));
    assert_eq!(
        show(&widen(n(), sc(Scalar::Long))),
        "BigInt(r.required(self.n))"
    );
    assert_eq!(show(&widen(n(), sc(Scalar::Float))), "r.required(self.n)");
    assert_eq!(
        show(&widen(n(), sc(Scalar::Decimal))),
        "r.decFromInt(r.required(self.n))"
    );
    let big = field("b", sc(Scalar::Long));
    assert_eq!(
        show(&widen(big, sc(Scalar::Decimal))),
        "r.decFromLong(r.toLong(r.required(self.b)))"
    );
}

// ---- in ----------------------------------------------------------------------------------

#[test]
fn in_uses_includes_a_key_test_or_the_decimal_helper() {
    let list = ex(
        ExprType::Type(Type::List(Box::new(Type::Decl(DeclId(0))))),
        ExprKind::List(vec![paid()]),
    );
    let enum_in = logic(Op::In, field("status", status()), list);
    assert_eq!(show(&enum_in), "['PAID'].includes(r.required(self.status))");

    let longs = logic(
        Op::In,
        field("n", sc(Scalar::Long)),
        field("ids", list_of(Scalar::Long)),
    );
    assert_eq!(
        show(&longs),
        "r.required(self.ids).map((e0) => r.toLong(e0)).includes(r.toLong(r.required(self.n)))"
    );

    let decimals = logic(
        Op::In,
        field("p", sc(Scalar::Decimal)),
        field("ps", list_of(Scalar::Decimal)),
    );
    assert_eq!(
        show(&decimals),
        "r.decIn(r.dec(r.required(self.p)), r.required(self.ps).map((e0) => r.dec(e0)))"
    );

    let map_t = ExprType::Type(Type::Map(Box::new(Type::Scalar(Scalar::Int))));
    let key = lit(sc(Scalar::String), ExprLit::String("max".into()));
    let in_map = logic(Op::In, key, field("limits", map_t));
    assert_eq!(show(&in_map), "r.hasKey(r.required(self.limits), 'max')");
}

// ---- built-in functions ------------------------------------------------------------------

#[test]
fn len_depends_on_the_operand_type() {
    let len = |x: Expr| show(&call(Builtin::Len, vec![x], sc(Scalar::Int)));
    assert_eq!(
        len(field("s", sc(Scalar::String))),
        "r.lenString(r.required(self.s))"
    );
    assert_eq!(
        len(field("b", sc(Scalar::Bytes))),
        "r.lenBytes(r.required(self.b))"
    );
    assert_eq!(
        len(field("xs", list_of(Scalar::Int))),
        "r.required(self.xs).length"
    );
    let map_t = ExprType::Type(Type::Map(Box::new(Type::Scalar(Scalar::Int))));
    assert_eq!(len(field("m", map_t)), "r.lenMap(r.required(self.m))");
}

#[test]
fn string_functions_and_durations() {
    let s = || field("s", sc(Scalar::String));
    let lit_s = |t: &str| lit(sc(Scalar::String), ExprLit::String(t.into()));
    let b = sc(Scalar::Bool);
    assert_eq!(
        show(&call(Builtin::StartsWith, vec![s(), lit_s("a")], b.clone())),
        "r.required(self.s).startsWith('a')"
    );
    assert_eq!(
        show(&call(Builtin::EndsWith, vec![s(), lit_s("b")], b.clone())),
        "r.required(self.s).endsWith('b')"
    );
    assert_eq!(
        show(&call(Builtin::Contains, vec![s(), lit_s("c")], b.clone())),
        "r.required(self.s).includes('c')"
    );
    assert_eq!(
        show(&call(Builtin::Matches, vec![s(), lit_s("^\\d$")], b)),
        "r.matches(r.required(self.s), '^\\\\d$')"
    );
    let units = [
        (Builtin::Days, "days"),
        (Builtin::Hours, "hours"),
        (Builtin::Minutes, "minutes"),
        (Builtin::Seconds, "seconds"),
        (Builtin::Ms, "ms"),
    ];
    for (f, name) in units {
        let big = lit(sc(Scalar::Long), ExprLit::Long(2));
        assert_eq!(
            show(&call(f, vec![big], ExprType::Duration)),
            format!("r.{name}(2n)")
        );
    }
}

// ---- comprehensions and lists ------------------------------------------------------------

#[test]
fn comprehensions_use_array_methods_and_checked_helpers() {
    let items_t = ExprType::Type(Type::List(Box::new(Type::Decl(DeclId(1)))));
    let items = || field("items", items_t.clone());
    let qty = |v: &str| path(Root::Var(v.into()), &[("qty", false)], sc(Scalar::Int));
    let positive = || bin(Op::Gt, qty("i"), int(0), sc(Scalar::Bool));
    assert_eq!(
        show(&quant(
            Quantifier::All,
            items(),
            "i",
            positive(),
            sc(Scalar::Bool)
        )),
        "r.required(self.items).every((l_i) => r.required(l_i.qty) > 0)"
    );
    assert_eq!(
        show(&quant(
            Quantifier::Any,
            items(),
            "i",
            positive(),
            sc(Scalar::Bool)
        )),
        "r.required(self.items).some((l_i) => r.required(l_i.qty) > 0)"
    );
    assert_eq!(
        show(&quant(
            Quantifier::Count,
            items(),
            "i",
            positive(),
            sc(Scalar::Int)
        )),
        "r.countWhere(r.required(self.items), (l_i) => r.required(l_i.qty) > 0)"
    );
    assert_eq!(
        show(&quant(
            Quantifier::Sum,
            items(),
            "i",
            qty("i"),
            sc(Scalar::Int)
        )),
        "r.sumInt(r.required(self.items), (l_i) => r.required(l_i.qty))"
    );
}

#[test]
fn sum_picks_the_helper_of_its_body_type() {
    let sum = |elem: Scalar| {
        let body = var("p", sc(elem));
        show(&quant(
            Quantifier::Sum,
            field("xs", list_of(elem)),
            "p",
            body,
            sc(elem),
        ))
    };
    assert_eq!(
        sum(Scalar::Int),
        "r.sumInt(r.required(self.xs), (l_p) => l_p)"
    );
    assert_eq!(
        sum(Scalar::Long),
        "r.sumLong(r.required(self.xs).map((e0) => r.toLong(e0)), (l_p) => l_p)"
    );
    assert_eq!(
        sum(Scalar::Float),
        "r.sumFloat(r.required(self.xs), (l_p) => l_p)"
    );
    assert_eq!(
        sum(Scalar::Decimal),
        "r.sumDecimal(r.required(self.xs).map((e0) => r.dec(e0)), (l_p) => l_p)"
    );
}

#[test]
fn a_nested_comprehension_keeps_both_variables() {
    let inner_t = ExprType::Type(Type::List(Box::new(Type::Decl(DeclId(1)))));
    let parts = path(Root::Var("r".into()), &[("parts", false)], inner_t);
    let body = quant(
        Quantifier::All,
        parts,
        "p",
        bin(Op::Ne, var("p", sc(Scalar::Int)), int(0), sc(Scalar::Bool)),
        sc(Scalar::Bool),
    );
    let outer_t = ExprType::Type(Type::List(Box::new(Type::Decl(DeclId(1)))));
    let e = quant(
        Quantifier::All,
        field("rows", outer_t),
        "r",
        body,
        sc(Scalar::Bool),
    );
    assert_eq!(
        show(&e),
        "r.required(self.rows).every((l_r) => r.required(l_r.parts).every((l_p) => l_p !== 0))"
    );
}

#[test]
fn a_variable_name_never_collides_with_a_reserved_word() {
    assert_eq!(var_ident("class"), "l_class");
    assert_eq!(var_ident("self"), "l_self");
    assert_eq!(var_ident("é"), "l_ue9");
}

#[test]
fn a_list_literal_prints_its_items() {
    let e = ex(list_of(Scalar::Int), ExprKind::List(vec![int(1), int(-2)]));
    assert_eq!(show(&e), "[1, -2]");
    let empty = ex(list_of(Scalar::Int), ExprKind::List(vec![]));
    assert_eq!(show(&empty), "[]");
}

#[test]
fn wire_names_come_from_the_fields() {
    let field = |name: &str, wire: &str| veld_ir::Field {
        name: name.into(),
        wire: wire.into(),
        ty: Type::Scalar(Scalar::Int),
        optional: false,
        default: None,
        readonly: false,
        constraints: veld_ir::Constraints::default(),
        docs: None,
        deprecated: None,
        inherited_from: None,
        example: None,
        origin: origin(),
    };
    let fields = vec![field("roomId", "room_id")];
    assert_eq!(wire_name(&fields, "roomId"), "room_id");
    assert_eq!(wire_name(&fields, "other"), "other");
}

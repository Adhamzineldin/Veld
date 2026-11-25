use super::*;
use crate::tree::Tree;
use veld_codegen::print_expr;
use veld_ir::{BinExpr, CallExpr, Expr, ExprKind, QuantExpr, Scalar, Step};

const SOURCE: &str = r#"enum Color { red, green }

model Inner {
  n: int
}

model Line {
  qty: int
  big: long
  price: decimal
  ratio: float
  name: string
  id: uuid
  stamp: datetime
  day: date
  clock: time
  color: Color
  flag: bool
  nick?: string
  count?: int
  tags: string[]
  nums: int[]
  by: Map<string, int>
  inner: Inner
  innerOpt?: Inner
  inners: Inner[]
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

fn field(name: &str, t: ExprType) -> Expr {
    path(Root::SelfValue, vec![step(name, false)], t)
}

fn int(n: i64) -> Expr {
    ex(sc(Scalar::Int), ExprKind::Lit(ExprLit::Int(n)))
}

fn bin(op: Op, l: Expr, r: Expr, t: ExprType) -> Expr {
    ex(t, ExprKind::Bin(Box::new(BinExpr { op, l, r })))
}

fn cmp(op: Op, l: Expr, r: Expr) -> Expr {
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

/// Prints `expr` as the rules of `Line` see it, and what the printer could not express.
fn java(expr: &Expr) -> (String, Vec<String>) {
    let ir = veld_emit_conformance::compile("rule_expr", SOURCE);
    let services = ir.services.iter().map(|s| s.id).collect();
    let tree = Tree::new(&ir, "g", "g.runtime", services);
    let line = ir.decls.iter().find(|d| d.name == "Line").unwrap();
    let mut fc = FileCtx::new(&tree, "g.app", "Line");
    let mut printer = JavaExpr::new(&mut fc, line.id);
    let code = print_expr(&mut printer, expr).code;
    (code, printer.unsupported().to_vec())
}

fn code(expr: &Expr) -> String {
    let (code, unsupported) = java(expr);
    assert!(unsupported.is_empty(), "{unsupported:?}");
    code
}

#[test]
fn literals_print_by_the_type_of_the_node() {
    let lit = |l, t| code(&ex(t, ExprKind::Lit(l)));
    assert_eq!(lit(ExprLit::Bool(true), sc(Scalar::Bool)), "true");
    assert_eq!(lit(ExprLit::Int(5), sc(Scalar::Int)), "5");
    assert_eq!(lit(ExprLit::Int(-5), sc(Scalar::Int)), "-5");
    assert_eq!(lit(ExprLit::Int(5), sc(Scalar::Long)), "5L");
    assert_eq!(lit(ExprLit::Int(5), sc(Scalar::Float)), "5.0");
    assert_eq!(
        lit(ExprLit::Int(5), sc(Scalar::Decimal)),
        "new BigDecimal(\"5\")"
    );
    assert_eq!(
        lit(ExprLit::Long(9_007_199_254_740_993), sc(Scalar::Long)),
        "9007199254740993L"
    );
    assert_eq!(lit(ExprLit::Float(0.5), sc(Scalar::Float)), "0.5");
    assert_eq!(
        lit(ExprLit::Decimal("120.00".into()), sc(Scalar::Decimal)),
        "new BigDecimal(\"120.00\")"
    );
    assert_eq!(
        lit(ExprLit::String("a\"b".into()), sc(Scalar::String)),
        "\"a\\\"b\""
    );
}

#[test]
fn an_enum_literal_is_the_constant_of_its_class() {
    let ir = veld_emit_conformance::compile("rule_expr", SOURCE);
    let color = ir.decls.iter().find(|d| d.name == "Color").unwrap();
    let lit = ExprLit::Enum(veld_ir::EnumLit {
        decl: color.id,
        name: "green".into(),
        wire: "green".into(),
    });
    let e = ex(ExprType::Type(Type::Decl(color.id)), ExprKind::Lit(lit));
    assert_eq!(code(&e), "Color.GREEN");
}

#[test]
fn paths_read_accessors_and_unbox_what_can_be_null() {
    assert_eq!(code(&field("qty", sc(Scalar::Int))), "v.qty()");
    assert_eq!(code(&field("count", sc(Scalar::Int))), "(int) v.count()");
    assert_eq!(
        code(&field("stamp", sc(Scalar::Datetime))),
        "Rules.instantMs(v.stamp())"
    );
    assert_eq!(
        code(&field("day", sc(Scalar::Date))),
        "Rules.dateDays(v.day())"
    );
    assert_eq!(
        code(&field("clock", sc(Scalar::Time))),
        "Rules.timeMs(v.clock())"
    );
    let nested = path(
        Root::SelfValue,
        vec![step("inner", false), step("n", false)],
        sc(Scalar::Int),
    );
    assert_eq!(code(&nested), "v.inner().n()");
}

#[test]
fn has_checks_every_optional_step() {
    let has = |steps| {
        ex(
            sc(Scalar::Bool),
            ExprKind::Has(PathRef {
                root: Root::SelfValue,
                steps,
            }),
        )
    };
    assert_eq!(code(&has(vec![step("nick", true)])), "v.nick() != null");
    assert_eq!(
        code(&has(vec![step("innerOpt", true), step("n", false)])),
        "v.innerOpt() != null"
    );
    assert_eq!(code(&has(vec![step("qty", false)])), "true");
}

#[test]
fn logic_is_native_and_parenthesised_by_precedence() {
    let t = || field("flag", sc(Scalar::Bool));
    let and = cmp(Op::And, t(), t());
    let or = cmp(Op::Or, and.clone(), t());
    assert_eq!(code(&and), "v.flag() && v.flag()");
    assert_eq!(code(&or), "v.flag() && v.flag() || v.flag()");
    let and_of_or = cmp(Op::And, cmp(Op::Or, t(), t()), t());
    assert_eq!(code(&and_of_or), "(v.flag() || v.flag()) && v.flag()");
    assert_eq!(code(&cmp(Op::Implies, t(), t())), "!v.flag() || v.flag()");
    assert_eq!(
        code(&cmp(Op::Implies, and, t())),
        "!(v.flag() && v.flag()) || v.flag()"
    );
    assert_eq!(
        code(&ex(sc(Scalar::Bool), ExprKind::Not(Box::new(t())))),
        "!v.flag()"
    );
}

#[test]
fn arithmetic_goes_through_checked_helpers_by_type() {
    let q = || field("qty", sc(Scalar::Int));
    let int_t = || sc(Scalar::Int);
    assert_eq!(
        code(&bin(Op::Add, q(), int(1), int_t())),
        "Rules.addInt(v.qty(), 1)"
    );
    assert_eq!(
        code(&bin(Op::Div, q(), int(2), int_t())),
        "Rules.divInt(v.qty(), 2)"
    );
    assert_eq!(
        code(&bin(Op::Rem, q(), int(2), int_t())),
        "Rules.remInt(v.qty(), 2)"
    );
    let big = || field("big", sc(Scalar::Long));
    assert_eq!(
        code(&bin(Op::Mul, big(), big(), sc(Scalar::Long))),
        "Rules.mulLong(v.big(), v.big())"
    );
    let ratio = || field("ratio", sc(Scalar::Float));
    assert_eq!(
        code(&bin(Op::Sub, ratio(), ratio(), sc(Scalar::Float))),
        "Rules.subDouble(v.ratio(), v.ratio())"
    );
    let price = || field("price", sc(Scalar::Decimal));
    assert_eq!(
        code(&bin(Op::Add, price(), price(), sc(Scalar::Decimal))),
        "v.price().add(v.price())"
    );
    assert_eq!(
        code(&bin(Op::Mul, price(), price(), sc(Scalar::Decimal))),
        "v.price().multiply(v.price())"
    );
    let neg = |e, t| ex(t, ExprKind::Neg(Box::new(e)));
    assert_eq!(code(&neg(q(), int_t())), "Rules.negInt(v.qty())");
    assert_eq!(code(&neg(ratio(), sc(Scalar::Float))), "-v.ratio()");
    assert_eq!(
        code(&neg(price(), sc(Scalar::Decimal))),
        "v.price().negate()"
    );
}

#[test]
fn comparisons_use_the_exact_form_of_each_type() {
    let q = || field("qty", sc(Scalar::Int));
    assert_eq!(code(&cmp(Op::Lt, q(), int(3))), "v.qty() < 3");
    assert_eq!(code(&cmp(Op::Eq, q(), int(3))), "v.qty() == 3");
    let count = || field("count", sc(Scalar::Int));
    assert_eq!(
        code(&cmp(Op::Eq, count(), count())),
        "(int) v.count() == (int) v.count()"
    );
    let price = || field("price", sc(Scalar::Decimal));
    assert_eq!(
        code(&cmp(Op::Eq, price(), price())),
        "v.price().compareTo(v.price()) == 0"
    );
    assert_eq!(
        code(&cmp(Op::Ge, price(), price())),
        "v.price().compareTo(v.price()) >= 0"
    );
    let name = || field("name", sc(Scalar::String));
    assert_eq!(
        code(&cmp(Op::Eq, name(), name())),
        "v.name().equals(v.name())"
    );
    assert_eq!(
        code(&cmp(Op::Ne, name(), name())),
        "!v.name().equals(v.name())"
    );
    let id = || field("id", sc(Scalar::Uuid));
    assert_eq!(code(&cmp(Op::Eq, id(), id())), "v.id().equals(v.id())");
    let at = || field("stamp", sc(Scalar::Datetime));
    assert_eq!(
        code(&cmp(Op::Gt, at(), at())),
        "Rules.instantMs(v.stamp()) > Rules.instantMs(v.stamp())"
    );
}

#[test]
fn time_and_duration_arithmetic_is_checked_long_arithmetic() {
    let at = || field("stamp", sc(Scalar::Datetime));
    let day = || field("day", sc(Scalar::Date));
    let n = || field("big", sc(Scalar::Long));
    let days = call(Builtin::Days, vec![n()], ExprType::Duration);
    assert_eq!(code(&days), "Rules.days(v.big())");
    let diff = bin(Op::Sub, at(), at(), ExprType::Duration);
    assert_eq!(
        code(&diff),
        "Rules.subLong(Rules.instantMs(v.stamp()), Rules.instantMs(v.stamp()))"
    );
    let later = bin(Op::Add, at(), days, sc(Scalar::Datetime));
    assert_eq!(
        code(&later),
        "Rules.addLong(Rules.instantMs(v.stamp()), Rules.days(v.big()))"
    );
    let dates = bin(Op::Sub, day(), day(), ExprType::Duration);
    assert_eq!(
        code(&dates),
        "Rules.dateDiff(Rules.dateDays(v.day()), Rules.dateDays(v.day()))"
    );
    let neg = ex(
        ExprType::Duration,
        ExprKind::Neg(Box::new(call(Builtin::Ms, vec![n()], ExprType::Duration))),
    );
    assert_eq!(code(&neg), "Rules.negLong(Rules.ms(v.big()))");
}

#[test]
fn built_ins_print_natively_where_java_is_exact() {
    let name = || field("name", sc(Scalar::String));
    let int_t = || sc(Scalar::Int);
    let text = |s: &str| ex(sc(Scalar::String), ExprKind::Lit(ExprLit::String(s.into())));
    assert_eq!(
        code(&call(Builtin::Len, vec![name()], int_t())),
        "Rules.len(v.name())"
    );
    let tags = field(
        "tags",
        ExprType::Type(Type::List(Box::new(Type::Scalar(Scalar::String)))),
    );
    assert_eq!(
        code(&call(Builtin::Len, vec![tags], int_t())),
        "v.tags().size()"
    );
    let by = field(
        "by",
        ExprType::Type(Type::Map(Box::new(Type::Scalar(Scalar::Int)))),
    );
    assert_eq!(
        code(&call(Builtin::Len, vec![by], int_t())),
        "v.by().size()"
    );
    let b = sc(Scalar::Bool);
    assert_eq!(
        code(&call(Builtin::Matches, vec![name(), text("^a")], b.clone())),
        "Rules.matches(v.name(), \"^a\")"
    );
    assert_eq!(
        code(&call(
            Builtin::StartsWith,
            vec![name(), text("a")],
            b.clone()
        )),
        "v.name().startsWith(\"a\")"
    );
    assert_eq!(
        code(&call(Builtin::EndsWith, vec![name(), text("a")], b.clone())),
        "v.name().endsWith(\"a\")"
    );
    assert_eq!(
        code(&call(Builtin::Contains, vec![name(), text("a")], b)),
        "v.name().contains(\"a\")"
    );
}

#[test]
fn in_picks_the_helper_by_element_type() {
    let b = sc(Scalar::Bool);
    let ints = ExprType::Type(Type::List(Box::new(Type::Scalar(Scalar::Int))));
    let list = ex(ints, ExprKind::List(vec![int(1), int(2)]));
    assert_eq!(
        code(&bin(Op::In, field("qty", sc(Scalar::Int)), list, b.clone())),
        "Rules.in(List.of(1, 2), v.qty())"
    );
    let decs = ExprType::Type(Type::List(Box::new(Type::Scalar(Scalar::Decimal))));
    let dec = |t: &str| {
        ex(
            sc(Scalar::Decimal),
            ExprKind::Lit(ExprLit::Decimal(t.into())),
        )
    };
    let list = ex(decs, ExprKind::List(vec![dec("1.5")]));
    assert_eq!(
        code(&bin(
            Op::In,
            field("price", sc(Scalar::Decimal)),
            list,
            b.clone()
        )),
        "Rules.inDecimal(List.of(new BigDecimal(\"1.5\")), v.price())"
    );
    let by = field(
        "by",
        ExprType::Type(Type::Map(Box::new(Type::Scalar(Scalar::Int)))),
    );
    let key = ex(
        sc(Scalar::String),
        ExprKind::Lit(ExprLit::String("k".into())),
    );
    assert_eq!(code(&bin(Op::In, key, by, b)), "v.by().containsKey(\"k\")");
}

#[test]
fn comprehensions_are_lambdas_over_prefixed_variables() {
    let b = sc(Scalar::Bool);
    let int_t = || sc(Scalar::Int);
    let nums = || {
        field(
            "nums",
            ExprType::Type(Type::List(Box::new(Type::Scalar(Scalar::Int)))),
        )
    };
    let x = || path(Root::Var("x".into()), vec![], int_t());
    let positive = || cmp(Op::Gt, x(), int(0));
    assert_eq!(
        code(&quant(Quantifier::All, nums(), "x", positive(), b.clone())),
        "Rules.all(v.nums(), it_x -> (int) it_x > 0)"
    );
    assert_eq!(
        code(&quant(Quantifier::Count, nums(), "x", positive(), int_t())),
        "Rules.count(v.nums(), it_x -> (int) it_x > 0)"
    );
    assert_eq!(
        code(&quant(Quantifier::Sum, nums(), "x", x(), int_t())),
        "Rules.sumInt(v.nums(), it_x -> (int) it_x)"
    );
    let inners = field(
        "inners",
        ExprType::Type(Type::List(Box::new(Type::Decl(
            veld_emit_conformance::compile("rule_expr", SOURCE)
                .decls
                .iter()
                .find(|d| d.name == "Inner")
                .unwrap()
                .id,
        )))),
    );
    let n = path(Root::Var("i".into()), vec![step("n", false)], int_t());
    assert_eq!(
        code(&quant(
            Quantifier::Any,
            inners,
            "i",
            cmp(Op::Eq, n, int(1)),
            b
        )),
        "Rules.any(v.inners(), it_i -> it_i.n() == 1)"
    );
}

#[test]
fn widening_is_an_explicit_conversion() {
    let q = || field("qty", sc(Scalar::Int));
    let widen = |t| ex(t, ExprKind::Widen(Box::new(q())));
    assert_eq!(code(&widen(sc(Scalar::Long))), "(long) v.qty()");
    assert_eq!(code(&widen(sc(Scalar::Float))), "(double) v.qty()");
    assert_eq!(
        code(&widen(sc(Scalar::Decimal))),
        "BigDecimal.valueOf(v.qty())"
    );
}

#[test]
fn what_java_cannot_express_is_reported_not_printed() {
    let (_, now) = java(&ex(sc(Scalar::Datetime), ExprKind::Now(veld_ir::Always)));
    assert_eq!(now.len(), 1);
    let param = path(Root::Param("body".into()), vec![], sc(Scalar::Int));
    assert_eq!(java(&param).1.len(), 1);
}

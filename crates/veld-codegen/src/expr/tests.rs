use super::*;
use veld_ir::{
    Always, Const, Decl, DeclId, Enum, EnumLit, ErrorDecl, IR_VERSION, Literal, Model, Origin,
    PackageId, Root, Step, Union, UnionArm, Variant,
};

const IMPLIES: u8 = 1;
const OR: u8 = 2;
const AND: u8 = 3;
const EQ: u8 = 4;
const CMP: u8 = 5;
const ADD: u8 = 6;
const MUL: u8 = 7;
const UNARY: u8 = 8;

// ---- IR builders -------------------------------------------------------------------------

fn sc(s: Scalar) -> ExprType {
    ExprType::Type(Type::Scalar(s))
}
fn bool_t() -> ExprType {
    sc(Scalar::Bool)
}
fn int_t() -> ExprType {
    sc(Scalar::Int)
}
fn list_of(s: Scalar) -> ExprType {
    ExprType::Type(Type::List(Box::new(Type::Scalar(s))))
}
fn ex(t: ExprType, e: ExprKind) -> Expr {
    Expr { t, e }
}
fn int(n: i64) -> Expr {
    ex(int_t(), ExprKind::Lit(ExprLit::Int(n)))
}
fn boolean(b: bool) -> Expr {
    ex(bool_t(), ExprKind::Lit(ExprLit::Bool(b)))
}
fn path(root: Root, steps: &[(&str, bool)], t: ExprType) -> Expr {
    ex(t, ExprKind::Path(path_ref(root, steps)))
}
fn path_ref(root: Root, steps: &[(&str, bool)]) -> PathRef {
    PathRef {
        root,
        steps: steps
            .iter()
            .map(|(n, o)| Step {
                name: (*n).into(),
                wire: (*n).into(),
                optional: *o,
            })
            .collect(),
    }
}
fn field(name: &str, t: ExprType) -> Expr {
    path(Root::SelfValue, &[(name, false)], t)
}
fn flag(name: &str) -> Expr {
    field(name, bool_t())
}
fn bin(op: Op, l: Expr, r: Expr, t: ExprType) -> Expr {
    ex(t, ExprKind::Bin(Box::new(BinExpr { op, l, r })))
}
fn logic(op: Op, l: Expr, r: Expr) -> Expr {
    bin(op, l, r, bool_t())
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
fn var(name: &str, t: ExprType) -> Expr {
    path(Root::Var(name.into()), &[], t)
}

// ---- the test printer --------------------------------------------------------------------

#[derive(Default)]
struct Dbg {
    log: Vec<String>,
}

fn tn(t: &ExprType) -> String {
    match t {
        ExprType::Duration => "duration".into(),
        ExprType::Type(Type::Scalar(s)) => format!("{s:?}").to_lowercase(),
        ExprType::Type(Type::List(i)) => format!("list<{}>", tn(&ExprType::Type((**i).clone()))),
        ExprType::Type(Type::Map(i)) => format!("map<{}>", tn(&ExprType::Type((**i).clone()))),
        ExprType::Type(Type::Page(i)) => format!("page<{}>", tn(&ExprType::Type((**i).clone()))),
        ExprType::Type(Type::Decl(d)) => format!("decl{}", d.0),
    }
}

fn op_info(op: Op) -> (&'static str, u8) {
    match op {
        Op::Implies => ("=>", IMPLIES),
        Op::Or => ("||", OR),
        Op::And => ("&&", AND),
        Op::Eq => ("==", EQ),
        Op::Ne => ("!=", EQ),
        Op::Lt => ("<", CMP),
        Op::Le => ("<=", CMP),
        Op::Gt => (">", CMP),
        Op::Ge => (">=", CMP),
        Op::In => ("in", CMP),
        Op::Add => ("+", ADD),
        Op::Sub => ("-", ADD),
        Op::Mul => ("*", MUL),
        Op::Div => ("/", MUL),
        Op::Rem => ("%", MUL),
    }
}

fn render_path(p: &PathRef) -> String {
    let root = match &p.root {
        Root::SelfValue => "self".to_string(),
        Root::Principal => "principal".to_string(),
        Root::Result => "result".to_string(),
        Root::Var(v) => v.clone(),
        Root::Param(n) => format!("param:{n}"),
        Root::Fact(n) => format!("fact:{n}"),
    };
    p.steps.iter().fold(root, |acc, s| {
        let dot = if s.optional { "?." } else { "." };
        format!("{acc}{dot}{}", s.name)
    })
}

impl ExprPrinter for Dbg {
    fn lit(&mut self, lit: &ExprLit, _ty: &ExprType) -> Printed {
        Printed::atom(match lit {
            ExprLit::Bool(b) => b.to_string(),
            ExprLit::Int(n) => n.to_string(),
            ExprLit::Long(n) => format!("{n}L"),
            ExprLit::Float(f) => format!("{f:?}"),
            ExprLit::Decimal(d) => format!("{d}d"),
            ExprLit::String(s) => format!("{s:?}"),
            ExprLit::Enum(e) => format!("enum:{}", e.name),
        })
    }
    fn path(&mut self, path: &PathRef, ty: &ExprType) -> Printed {
        Printed::atom(format!("{}:{}", render_path(path), tn(ty)))
    }
    fn has(&mut self, path: &PathRef) -> Printed {
        Printed::atom(format!("has({})", render_path(path)))
    }
    fn now(&mut self) -> Printed {
        Printed::atom("now")
    }
    fn not(&mut self, operand: Printed) -> Printed {
        Printed::new(format!("!{}", operand.at(UNARY)), UNARY)
    }
    fn neg(&mut self, operand: Printed, ty: &ExprType) -> Printed {
        Printed::new(format!("-{}:{}", operand.at(UNARY), tn(ty)), UNARY)
    }
    fn bin(
        &mut self,
        op: Op,
        l: Printed,
        r: Printed,
        left: &ExprType,
        right: &ExprType,
        out: &ExprType,
    ) -> Printed {
        let (sym, prec) = op_info(op);
        self.log.push(format!(
            "bin {sym} {} {} -> {}",
            tn(left),
            tn(right),
            tn(out)
        ));
        Printed::new(format!("{} {sym} {}", l.at(prec), r.at(prec + 1)), prec)
    }
    fn call(
        &mut self,
        f: Builtin,
        args: Vec<Printed>,
        arg_types: &[&ExprType],
        out: &ExprType,
    ) -> Printed {
        let codes: Vec<String> = args.iter().map(|a| a.code.clone()).collect();
        let tys: Vec<String> = arg_types.iter().map(|t| tn(t)).collect();
        self.log
            .push(format!("call {f:?} [{}] -> {}", tys.join(","), tn(out)));
        Printed::atom(format!("{f:?}({})", codes.join(", ")).to_lowercase())
    }
    fn enter_var(&mut self, var: &str, elem: &ExprType) {
        self.log.push(format!("enter {var}:{}", tn(elem)));
    }
    fn exit_var(&mut self, var: &str) {
        self.log.push(format!("exit {var}"));
    }
    fn quant(
        &mut self,
        q: Quantifier,
        list: Printed,
        elem: &ExprType,
        var: &str,
        body: Printed,
        body_ty: &ExprType,
    ) -> Printed {
        self.log.push(format!(
            "quant {q:?} elem={} body={}",
            tn(elem),
            tn(body_ty)
        ));
        let name = format!("{q:?}").to_lowercase();
        Printed::atom(format!("{name}({}, {var} -> {})", list.code, body.code))
    }
    fn list(&mut self, items: Vec<Printed>, elem: Option<&ExprType>) -> Printed {
        let codes: Vec<String> = items.iter().map(|i| i.code.clone()).collect();
        self.log
            .push(format!("list elem={}", elem.map_or("none".into(), tn)));
        Printed::atom(format!("[{}]", codes.join(", ")))
    }
    fn widen(&mut self, operand: Printed, from: &ExprType, to: &ExprType) -> Printed {
        Printed::atom(format!("widen<{}->{}>({})", tn(from), tn(to), operand.code))
    }
    fn old(&mut self, operand: Printed, ty: &ExprType) -> Printed {
        Printed::atom(format!("old<{}>({})", tn(ty), operand.code))
    }
}

fn show(e: &Expr) -> String {
    print_expr(&mut Dbg::default(), e).code
}

// ---- Printed -----------------------------------------------------------------------------

#[test]
fn at_wraps_only_when_binding_looser_than_required() {
    let p = Printed::new("a || b", OR);
    assert_eq!(p.at(AND), "(a || b)");
    assert_eq!(p.at(OR), "a || b");
    assert_eq!(p.at(IMPLIES), "a || b");
    assert_eq!(Printed::atom("x").at(Printed::ATOM), "x");
    assert_eq!(Printed::atom("x").prec, Printed::ATOM);
}

// ---- traversal ---------------------------------------------------------------------------

#[test]
fn literals_cover_every_form() {
    assert_eq!(show(&boolean(true)), "true");
    assert_eq!(show(&int(-4)), "-4");
    let long = ex(sc(Scalar::Long), ExprKind::Lit(ExprLit::Long(9)));
    assert_eq!(show(&long), "9L");
    let float = ex(sc(Scalar::Float), ExprKind::Lit(ExprLit::Float(1.5)));
    assert_eq!(show(&float), "1.5");
    let dec = ex(
        sc(Scalar::Decimal),
        ExprKind::Lit(ExprLit::Decimal("2.50".into())),
    );
    assert_eq!(show(&dec), "2.50d");
    let s = ex(
        sc(Scalar::String),
        ExprKind::Lit(ExprLit::String("hi".into())),
    );
    assert_eq!(show(&s), "\"hi\"");
    let e = ExprLit::Enum(EnumLit {
        decl: DeclId(0),
        name: "Open".into(),
        wire: "open".into(),
    });
    let enum_t = ExprType::Type(Type::Decl(DeclId(0)));
    assert_eq!(show(&ex(enum_t, ExprKind::Lit(e))), "enum:Open");
}

#[test]
fn paths_has_and_now() {
    let p = path(
        Root::Principal,
        &[("org", false), ("name", true)],
        sc(Scalar::String),
    );
    assert_eq!(show(&p), "principal.org?.name:string");
    let has = ex(
        bool_t(),
        ExprKind::Has(path_ref(Root::Param("p".into()), &[("x", true)])),
    );
    assert_eq!(show(&has), "has(param:p?.x)");
    let now = ex(sc(Scalar::Datetime), ExprKind::Now(Always));
    assert_eq!(show(&now), "now");
    assert_eq!(
        show(&path(Root::Fact("f".into()), &[], int_t())),
        "fact:f:int"
    );
    assert_eq!(show(&path(Root::Result, &[], int_t())), "result:int");
}

#[test]
fn unary_operators_and_precedence() {
    let not = ex(bool_t(), ExprKind::Not(Box::new(flag("a"))));
    assert_eq!(show(&not), "!self.a:bool");
    let not_or = ex(
        bool_t(),
        ExprKind::Not(Box::new(logic(Op::Or, flag("a"), flag("b")))),
    );
    assert_eq!(show(&not_or), "!(self.a:bool || self.b:bool)");
    let neg = ex(int_t(), ExprKind::Neg(Box::new(field("n", int_t()))));
    assert_eq!(show(&neg), "-self.n:int:int");
}

#[test]
fn parentheses_follow_precedence() {
    let (a, b, c) = (|| flag("a"), || flag("b"), || flag("c"));
    let keep = logic(Op::And, logic(Op::Or, a(), b()), c());
    assert_eq!(show(&keep), "(self.a:bool || self.b:bool) && self.c:bool");
    let drop = logic(Op::Or, a(), logic(Op::And, b(), c()));
    assert_eq!(show(&drop), "self.a:bool || self.b:bool && self.c:bool");
    let right = logic(Op::Implies, a(), logic(Op::Implies, b(), c()));
    assert_eq!(show(&right), "self.a:bool => (self.b:bool => self.c:bool)");
    let left = logic(Op::Implies, logic(Op::Implies, a(), b()), c());
    assert_eq!(show(&left), "self.a:bool => self.b:bool => self.c:bool");
}

#[test]
fn bin_receives_operand_and_result_types() {
    let sum = bin(Op::Add, field("a", int_t()), int(1), int_t());
    let cmp = logic(Op::Lt, sum, int(10));
    let mut p = Dbg::default();
    let out = print_expr(&mut p, &cmp);
    assert_eq!(out.code, "self.a:int + 1 < 10");
    assert_eq!(out.prec, CMP);
    assert_eq!(p.log, ["bin + int int -> int", "bin < int int -> bool"]);
}

#[test]
fn calls_pass_args_in_order_with_types() {
    let s = |n: &str| field(n, sc(Scalar::String));
    let call = ex(
        bool_t(),
        ExprKind::Call(CallExpr {
            f: Builtin::StartsWith,
            args: vec![s("x"), s("y")],
        }),
    );
    let mut p = Dbg::default();
    let out = print_expr(&mut p, &call);
    assert_eq!(out.code, "startswith(self.x:string, self.y:string)");
    assert_eq!(p.log, ["call StartsWith [string,string] -> bool"]);
    let days = ex(
        ExprType::Duration,
        ExprKind::Call(CallExpr {
            f: Builtin::Days,
            args: vec![int(2)],
        }),
    );
    assert_eq!(show(&days), "days(2)");
}

#[test]
fn lists_widen_and_old() {
    let items = ex(list_of(Scalar::Int), ExprKind::List(vec![int(1), int(2)]));
    let mut p = Dbg::default();
    assert_eq!(print_expr(&mut p, &items).code, "[1, 2]");
    assert_eq!(p.log, ["list elem=int"]);

    let empty = ex(int_t(), ExprKind::List(vec![]));
    let mut p = Dbg::default();
    assert_eq!(print_expr(&mut p, &empty).code, "[]");
    assert_eq!(p.log, ["list elem=none"]);

    let widen = ex(
        sc(Scalar::Long),
        ExprKind::Widen(Box::new(field("n", int_t()))),
    );
    assert_eq!(show(&widen), "widen<int->long>(self.n:int)");
    let old = ex(int_t(), ExprKind::Old(Box::new(field("n", int_t()))));
    assert_eq!(show(&old), "old<int>(self.n:int)");
}

#[test]
fn comprehension_brackets_the_body_with_enter_and_exit() {
    let list = field("xs", list_of(Scalar::Int));
    let body = logic(Op::Gt, var("x", int_t()), int(0));
    let q = quant(Quantifier::All, list, "x", body, bool_t());
    let mut p = Dbg::default();
    let out = print_expr(&mut p, &q);
    assert_eq!(out.code, "all(self.xs:list<int>, x -> x:int > 0)");
    assert_eq!(
        p.log,
        [
            "enter x:int",
            "bin > int int -> bool",
            "exit x",
            "quant All elem=int body=bool",
        ]
    );
}

#[test]
fn nested_comprehensions_nest_enter_and_exit() {
    let inner = quant(
        Quantifier::Count,
        var("row", list_of(Scalar::Int)),
        "c",
        logic(Op::Gt, var("c", int_t()), int(0)),
        int_t(),
    );
    let outer_list = ex(
        ExprType::Type(Type::List(Box::new(Type::List(Box::new(Type::Scalar(
            Scalar::Int,
        )))))),
        ExprKind::Path(path_ref(Root::SelfValue, &[("grid", false)])),
    );
    let outer = quant(Quantifier::Sum, outer_list, "row", inner, int_t());
    let mut p = Dbg::default();
    let out = print_expr(&mut p, &outer);
    assert_eq!(
        out.code,
        "sum(self.grid:list<list<int>>, row -> count(row:list<int>, c -> c:int > 0))"
    );
    assert_eq!(
        p.log,
        [
            "enter row:list<int>",
            "enter c:int",
            "bin > int int -> bool",
            "exit c",
            "quant Count elem=int body=bool",
            "exit row",
            "quant Sum elem=list<int> body=int",
        ]
    );
}

#[test]
fn page_lists_give_their_element_type_and_non_lists_fall_back_to_json() {
    let page = ExprType::Type(Type::Page(Box::new(Type::Scalar(Scalar::Uuid))));
    assert_eq!(element_type(&page), Some(sc(Scalar::Uuid)));
    assert_eq!(element_type(&list_of(Scalar::Bool)), Some(bool_t()));
    assert_eq!(element_type(&int_t()), None);
    assert_eq!(element_type(&ExprType::Duration), None);
    let map = ExprType::Type(Type::Map(Box::new(Type::Scalar(Scalar::Int))));
    assert_eq!(element_type(&map), None);

    let q = quant(
        Quantifier::Any,
        field("n", int_t()),
        "x",
        boolean(true),
        bool_t(),
    );
    let mut p = Dbg::default();
    print_expr(&mut p, &q);
    assert_eq!(p.log[0], "enter x:json");
    assert_eq!(p.log[2], "quant Any elem=json body=bool");
}

#[test]
fn works_through_a_trait_object() {
    let mut p = Dbg::default();
    let dynamic: &mut dyn ExprPrinter = &mut p;
    assert_eq!(print_expr(dynamic, &int(3)).code, "3");
}

// ---- ValueKind ---------------------------------------------------------------------------

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

fn kind_ir() -> Ir {
    let model = || Model {
        parent: None,
        fields: vec![],
        rules: vec![],
    };
    let decls = vec![
        decl(
            0,
            "E",
            DeclKind::Enum(Enum::plain(vec![Variant {
                name: "A".into(),
                wire: "a".into(),
                docs: None,
                deprecated: None,
            }])),
        ),
        decl(1, "M", DeclKind::Model(model())),
        decl(
            2,
            "U",
            DeclKind::Union(Union {
                discriminator: "kind".into(),
                arms: vec![UnionArm {
                    label: "m".into(),
                    decl: DeclId(1),
                }],
            }),
        ),
        decl(
            3,
            "Err",
            DeclKind::Error(ErrorDecl {
                default_status: None,
                fields: model(),
            }),
        ),
        decl(
            4,
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
        examples: vec![],
    }
}

#[test]
fn value_kind_of_every_scalar() {
    let ir = kind_ir();
    let expected = [
        (Scalar::String, ValueKind::String),
        (Scalar::Int, ValueKind::Int),
        (Scalar::Long, ValueKind::Long),
        (Scalar::Float, ValueKind::Float),
        (Scalar::Decimal, ValueKind::Decimal),
        (Scalar::Bool, ValueKind::Bool),
        (Scalar::Date, ValueKind::Date),
        (Scalar::Datetime, ValueKind::Datetime),
        (Scalar::Time, ValueKind::Time),
        (Scalar::Uuid, ValueKind::Uuid),
        (Scalar::Bytes, ValueKind::Bytes),
        (Scalar::Json, ValueKind::Json),
    ];
    assert_eq!(expected.len(), Scalar::ALL.len());
    for (scalar, kind) in expected {
        assert_eq!(ValueKind::of(&sc(scalar), &ir), kind, "{scalar:?}");
    }
}

#[test]
fn value_kind_of_compound_and_declared_types() {
    let ir = kind_ir();
    let of = |t: Type| ValueKind::of(&ExprType::Type(t), &ir);
    let inner = || Box::new(Type::Scalar(Scalar::Int));
    assert_eq!(ValueKind::of(&ExprType::Duration, &ir), ValueKind::Duration);
    assert_eq!(of(Type::List(inner())), ValueKind::List);
    assert_eq!(of(Type::Map(inner())), ValueKind::Map);
    assert_eq!(of(Type::Page(inner())), ValueKind::Page);
    assert_eq!(of(Type::Decl(DeclId(0))), ValueKind::Enum);
    assert_eq!(of(Type::Decl(DeclId(1))), ValueKind::Model);
    assert_eq!(of(Type::Decl(DeclId(2))), ValueKind::Model);
    assert_eq!(of(Type::Decl(DeclId(3))), ValueKind::Model);
    assert_eq!(of(Type::Decl(DeclId(4))), ValueKind::Json);
    assert_eq!(of(Type::Decl(DeclId(99))), ValueKind::Json);
}

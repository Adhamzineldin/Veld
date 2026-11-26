//! Derived fields in the IR (`spec/rules.md` section 17.2): the keys that stay out, the reads of
//! an expression and the order in which the values are computed.

use crate::endpoint_rules::tests::origin;
use crate::rules::{Always, BinExpr, Op, PathRef, QuantExpr, Quantifier, Root, Step};
use crate::*;

fn int() -> ExprType {
    ExprType::Type(Type::Scalar(Scalar::Int))
}

fn read(name: &str) -> Expr {
    Expr {
        t: int(),
        e: ExprKind::Path(PathRef {
            root: Root::SelfValue,
            steps: vec![Step {
                name: name.into(),
                wire: name.into(),
                optional: false,
            }],
        }),
    }
}

fn add(l: Expr, r: Expr) -> Expr {
    Expr {
        t: int(),
        e: ExprKind::Bin(Box::new(BinExpr { op: Op::Add, l, r })),
    }
}

fn plain(name: &str) -> Field {
    Field {
        name: name.into(),
        wire: name.into(),
        ty: Type::Scalar(Scalar::Int),
        optional: false,
        default: None,
        readonly: false,
        version: false,
        derived: None,
        derived_id: None,
        pii: false,
        secret: false,
        visible: None,
        visible_principal: None,
        inherited_from: None,
        constraints: Constraints::default(),
        example: None,
        docs: None,
        deprecated: None,
        origin: origin(1),
    }
}

fn derived(name: &str, expr: Expr) -> Field {
    Field {
        derived: Some(expr),
        derived_id: Some(format!("veld:p/m.M.{name}@0000000000000000")),
        ..plain(name)
    }
}

fn model(fields: Vec<Field>) -> Decl {
    Decl {
        id: DeclId(0),
        package: PackageId(0),
        module: "m".into(),
        name: "M".into(),
        qualified: "m.M".into(),
        sig: "veld:p/M@0".into(),
        docs: None,
        deprecated: None,
        origin: origin(1),
        kind: DeclKind::Model(Model {
            parent: None,
            fields,
            rules: vec![],
            retain: None,
        }),
    }
}

fn ir_of(d: Decl) -> Ir {
    Ir {
        ir_version: IR_VERSION.into(),
        packages: vec![],
        decls: vec![d],
        services: vec![],
        consumes: Default::default(),
        examples: vec![],
        policies: vec![],
        channels: vec![],
    }
}

fn names(d: &Decl) -> Vec<&str> {
    d.derived_fields().iter().map(|f| f.name.as_str()).collect()
}

#[test]
fn a_plain_field_writes_neither_key() {
    let v = serde_json::to_value(plain("a")).unwrap();
    assert!(v.get("derived").is_none() && v.get("derived_id").is_none());
}

#[test]
fn a_derived_field_writes_both_keys_and_reads_them_back() {
    let f = derived("total", add(read("a"), read("b")));
    let v = serde_json::to_value(&f).unwrap();
    assert!(v.get("derived").is_some());
    assert_eq!(v["derived_id"], "veld:p/m.M.total@0000000000000000");
    let back: Field = serde_json::from_value(v).unwrap();
    assert_eq!(back, f);
}

#[test]
fn the_reads_of_an_expression_are_the_fields_of_self_in_order_of_first_use() {
    let e = add(add(read("b"), read("a")), read("b"));
    assert_eq!(e.self_reads(), ["b", "a"]);
}

#[test]
fn a_lambda_variable_is_not_a_read_but_the_names_inside_a_comprehension_are() {
    let body = add(
        Expr {
            t: int(),
            e: ExprKind::Path(PathRef {
                root: Root::Var("i".into()),
                steps: vec![Step {
                    name: "price".into(),
                    wire: "price".into(),
                    optional: false,
                }],
            }),
        },
        read("fee"),
    );
    let sum = Expr {
        t: int(),
        e: ExprKind::Quant(Box::new(QuantExpr {
            q: Quantifier::Sum,
            list: read("items"),
            var: "i".into(),
            body,
        })),
    };
    assert_eq!(sum.self_reads(), ["items", "fee"]);
    let now = Expr {
        t: int(),
        e: ExprKind::Now(Always),
    };
    assert!(now.self_reads().is_empty());
}

#[test]
fn derived_fields_come_in_declaration_order_when_nothing_depends_on_anything() {
    let d = model(vec![
        plain("a"),
        derived("y", read("a")),
        derived("x", read("a")),
    ]);
    assert_eq!(names(&d), ["y", "x"]);
}

#[test]
fn a_derived_field_comes_after_the_derived_fields_it_reads() {
    let d = model(vec![
        plain("a"),
        derived("total", add(read("tax"), read("net"))),
        derived("tax", read("net")),
        derived("net", read("a")),
        derived("other", read("a")),
    ]);
    assert_eq!(names(&d), ["tax", "net", "total", "other"]);
}

#[test]
fn a_cycle_does_not_loop() {
    let d = model(vec![derived("a", read("b")), derived("b", read("a"))]);
    assert_eq!(names(&d), ["a", "b"]);
}

#[test]
fn a_model_with_a_derived_field_uses_the_feature_and_one_without_does_not() {
    let with = ir_of(model(vec![derived("t", read("a"))]));
    let mut f = BTreeSet::new();
    with.decl_features(DeclId(0), &mut f);
    assert!(f.contains(&Feature::RulesDerived));
    let without = ir_of(model(vec![plain("a")]));
    let mut f = BTreeSet::new();
    without.decl_features(DeclId(0), &mut f);
    assert!(!f.contains(&Feature::RulesDerived));
    assert!(Feature::OPT_IN.contains(&Feature::RulesDerived));
}

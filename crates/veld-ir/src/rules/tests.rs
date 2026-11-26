use serde_json::{Value, json};

use super::*;
use crate::{Model, Origin, Scalar};

fn ty(s: Scalar) -> ExprType {
    ExprType::Type(Type::Scalar(s))
}

fn node(t: ExprType, e: ExprKind) -> Expr {
    Expr { t, e }
}

fn int(n: i64) -> Expr {
    node(ty(Scalar::Int), ExprKind::Lit(ExprLit::Int(n)))
}

fn path(root: Root, name: &str, t: ExprType) -> Expr {
    node(
        t,
        ExprKind::Path(PathRef {
            root,
            steps: vec![Step {
                name: name.into(),
                wire: name.into(),
                optional: false,
            }],
        }),
    )
}

fn bin(op: Op, t: ExprType, l: Expr, r: Expr) -> Expr {
    node(t, ExprKind::Bin(Box::new(BinExpr { op, l, r })))
}

fn literals() -> Vec<Expr> {
    [
        ExprLit::Bool(true),
        ExprLit::Int(5),
        ExprLit::Long(9_007_199_254_740_993),
        ExprLit::Float(0.5),
        ExprLit::Decimal("120.00".into()),
        ExprLit::String("x".into()),
        ExprLit::Enum(EnumLit {
            decl: DeclId(3),
            name: "paid".into(),
            wire: "PAID".into(),
        }),
    ]
    .into_iter()
    .map(|l| node(ty(Scalar::String), ExprKind::Lit(l)))
    .collect()
}

fn roots() -> Vec<Expr> {
    [
        Root::SelfValue,
        Root::Var("i".into()),
        Root::Param("body".into()),
        Root::Fact("room".into()),
        Root::Principal,
        Root::Result,
    ]
    .into_iter()
    .map(|r| path(r, "a", ty(Scalar::Int)))
    .collect()
}

fn others() -> Vec<Expr> {
    let has = node(
        ty(Scalar::Bool),
        ExprKind::Has(PathRef {
            root: Root::SelfValue,
            steps: vec![],
        }),
    );
    let call = node(
        ExprType::Duration,
        ExprKind::Call(CallExpr {
            f: Builtin::Days,
            args: vec![int(2)],
        }),
    );
    let list = node(
        ExprType::Type(Type::List(Box::new(Type::Scalar(Scalar::Int)))),
        ExprKind::List(vec![int(1), int(2)]),
    );
    let quant = node(
        ty(Scalar::Int),
        ExprKind::Quant(Box::new(QuantExpr {
            q: Quantifier::Sum,
            list,
            var: "i".into(),
            body: path(Root::Var("i".into()), "n", ty(Scalar::Int)),
        })),
    );
    vec![
        call,
        quant,
        node(ty(Scalar::Long), ExprKind::Widen(Box::new(int(1)))),
        node(ty(Scalar::Int), ExprKind::Old(Box::new(int(1)))),
        node(ty(Scalar::Datetime), ExprKind::Now(Always)),
        node(ty(Scalar::Bool), ExprKind::Not(Box::new(has))),
        node(ty(Scalar::Int), ExprKind::Neg(Box::new(int(3)))),
    ]
}

/// One expression that uses every `ExprKind`, every literal form and every root.
fn everything() -> Expr {
    let all: Vec<Expr> = [literals(), roots(), others()].concat();
    bin(
        Op::And,
        ty(Scalar::Bool),
        int(1),
        node(ty(Scalar::Bool), ExprKind::List(all)),
    )
}

fn rule(expr: Expr) -> Rule {
    Rule {
        flag: None,
        name: "endAfterStart".into(),
        id: "veld:bookings/Booking.endAfterStart@91c0d2aa0b1c2d3e".into(),
        expr,
        message: Some("A booking must end after it starts".into()),
        reads: vec!["end".into(), "start".into()],
        reads_readonly: false,
        inherited_from: None,
        docs: None,
        deprecated: None,
        rollout: None,
        origin: Origin {
            file: "a.veld".into(),
            line: 4,
            col: 3,
        },
    }
}

#[test]
fn a_rule_using_every_expression_kind_round_trips() {
    let r = rule(everything());
    let text = serde_json::to_string(&r).unwrap();
    let back: Rule = serde_json::from_str(&text).unwrap();
    assert_eq!(back, r);
    for key in [
        "lit", "path", "now", "not", "neg", "bin", "has", "call", "quant", "list", "widen", "old",
    ] {
        assert!(text.contains(&format!("\"{key}\":")), "{key} in {text}");
    }
}

#[test]
fn a_binary_node_has_the_documented_json_form() {
    let e = bin(
        Op::Gt,
        ty(Scalar::Bool),
        path(Root::SelfValue, "end", ty(Scalar::Datetime)),
        path(Root::SelfValue, "start", ty(Scalar::Datetime)),
    );
    let step = |n: &str| json!({"name": n, "wire": n, "optional": false});
    assert_eq!(
        serde_json::to_value(&e).unwrap(),
        json!({
            "t": {"scalar": "bool"},
            "e": {"bin": {
                "op": "gt",
                "l": {"t": {"scalar": "datetime"}, "e": {"path": {"root": "self", "steps": [step("end")]}}},
                "r": {"t": {"scalar": "datetime"}, "e": {"path": {"root": "self", "steps": [step("start")]}}}
            }}
        })
    );
}

#[test]
fn literals_have_the_documented_json_forms() {
    let lit = |l: ExprLit| serde_json::to_value(l).unwrap();
    assert_eq!(lit(ExprLit::Bool(true)), json!({"bool": true}));
    assert_eq!(lit(ExprLit::Int(5)), json!({"int": 5}));
    assert_eq!(
        lit(ExprLit::Long(9_007_199_254_740_993)),
        json!({"long": "9007199254740993"})
    );
    assert_eq!(
        lit(ExprLit::Decimal("120.00".into())),
        json!({"decimal": "120.00"})
    );
    let paid = ExprLit::Enum(EnumLit {
        decl: DeclId(7),
        name: "paid".into(),
        wire: "PAID".into(),
    });
    assert_eq!(
        lit(paid),
        json!({"enum": {"decl": 7, "name": "paid", "wire": "PAID"}})
    );
    let bad: Result<ExprLit, _> = serde_json::from_value(json!({"long": "12x"}));
    assert!(bad.is_err());
}

#[test]
fn roots_and_the_special_nodes_have_the_documented_json_forms() {
    let v = |r: Root| serde_json::to_value(r).unwrap();
    assert_eq!(v(Root::SelfValue), json!("self"));
    assert_eq!(v(Root::Principal), json!("principal"));
    assert_eq!(v(Root::Result), json!("result"));
    assert_eq!(v(Root::Var("i".into())), json!({"var": "i"}));
    assert_eq!(v(Root::Param("body".into())), json!({"param": "body"}));
    assert_eq!(v(Root::Fact("room".into())), json!({"fact": "room"}));
    assert_eq!(
        serde_json::to_value(node(ty(Scalar::Datetime), ExprKind::Now(Always))).unwrap(),
        json!({"t": {"scalar": "datetime"}, "e": {"now": true}})
    );
    assert_eq!(
        serde_json::to_value(node(ExprType::Duration, ExprKind::Now(Always))).unwrap()["t"],
        json!("duration")
    );
    let not_true: Result<ExprKind, _> = serde_json::from_value(json!({"now": false}));
    assert!(not_true.is_err());
}

#[test]
fn operator_and_builtin_names_match_the_spec() {
    let ops = [
        (Op::And, "and"),
        (Op::Or, "or"),
        (Op::Implies, "implies"),
        (Op::Eq, "eq"),
        (Op::Ne, "ne"),
        (Op::Lt, "lt"),
        (Op::Le, "le"),
        (Op::Gt, "gt"),
        (Op::Ge, "ge"),
        (Op::In, "in"),
        (Op::Add, "add"),
        (Op::Sub, "sub"),
        (Op::Mul, "mul"),
        (Op::Div, "div"),
        (Op::Rem, "rem"),
    ];
    for (op, name) in ops {
        assert_eq!(serde_json::to_value(op).unwrap(), json!(name));
    }
    let builtins = [
        (Builtin::Len, "len"),
        (Builtin::Matches, "matches"),
        (Builtin::StartsWith, "starts_with"),
        (Builtin::EndsWith, "ends_with"),
        (Builtin::Contains, "contains"),
        (Builtin::Days, "days"),
        (Builtin::Hours, "hours"),
        (Builtin::Minutes, "minutes"),
        (Builtin::Seconds, "seconds"),
        (Builtin::Ms, "ms"),
    ];
    for (b, name) in builtins {
        assert_eq!(serde_json::to_value(b).unwrap(), json!(name));
    }
}

#[test]
fn a_model_without_rules_has_no_rules_key() {
    let m = Model {
        parent: None,
        fields: vec![],
        rules: vec![],
        retain: None,
    };
    let v = serde_json::to_value(&m).unwrap();
    assert!(v.get("rules").is_none(), "{v}");
    // An IR written before rules existed still loads.
    let old: Model = serde_json::from_value(json!({"parent": null, "fields": []})).unwrap();
    assert!(old.rules.is_empty());
}

#[test]
fn a_model_with_a_rule_serialises_the_rules_key_and_round_trips() {
    let m = Model {
        parent: None,
        fields: vec![],
        rules: vec![rule(everything())],
        retain: None,
    };
    let v: Value = serde_json::to_value(&m).unwrap();
    assert_eq!(v["rules"][0]["name"], json!("endAfterStart"));
    let back: Model = serde_json::from_value(v).unwrap();
    assert_eq!(back, m);
}

#[test]
fn an_enforced_rule_has_no_rollout_key_and_an_audited_one_writes_audit() {
    let mut r = rule(int(1));
    let v = serde_json::to_value(&r).unwrap();
    assert!(v.get("rollout").is_none(), "{v}");
    assert!(!r.is_audited());
    r.rollout = Some(Rollout::Audit);
    assert!(r.is_audited());
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["rollout"], json!("audit"));
    let back: Rule = serde_json::from_value(v).unwrap();
    assert_eq!(back, r);
    let bad: Result<Rollout, _> = serde_json::from_value(json!("shadow"));
    assert!(bad.is_err());
}

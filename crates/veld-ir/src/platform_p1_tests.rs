//! The IR keys of platform phase P1 (`spec/platform.md` sections 2 to 5): each is left out of the
//! JSON when absent, and a field or an endpoint that has one uses its feature.

use crate::endpoint_rules::tests::origin;
use crate::rules::{Always, PathRef, Root};
use crate::*;

fn plain(name: &str) -> Field {
    Field {
        name: name.into(),
        wire: name.into(),
        ty: Type::Scalar(Scalar::String),
        optional: true,
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

fn now_is_true() -> Expr {
    Expr {
        t: ExprType::Type(Type::Scalar(Scalar::Bool)),
        e: ExprKind::Bin(Box::new(rules::BinExpr {
            op: rules::Op::Le,
            l: Expr {
                t: ExprType::Type(Type::Scalar(Scalar::Datetime)),
                e: ExprKind::Now(Always),
            },
            r: Expr {
                t: ExprType::Type(Type::Scalar(Scalar::Datetime)),
                e: ExprKind::Path(PathRef {
                    root: Root::SelfValue,
                    steps: vec![],
                }),
            },
        })),
    }
}

#[test]
fn a_plain_field_writes_no_visible_key() {
    let v = serde_json::to_value(plain("a")).unwrap();
    assert!(v.get("visible").is_none());
}

#[test]
fn a_visible_field_writes_the_key_and_reads_it_back() {
    let f = Field {
        visible: Some(now_is_true()),
        ..plain("phone")
    };
    let v = serde_json::to_value(&f).unwrap();
    assert!(v.get("visible").is_some());
    let back: Field = serde_json::from_value(v).unwrap();
    assert_eq!(back, f);
}

#[test]
fn a_visible_field_uses_the_feature_and_a_plain_one_does_not() {
    let mut used = std::collections::BTreeSet::new();
    field_features(&plain("a"), &mut used);
    assert!(!used.contains(&Feature::Visibility));
    let shown = Field {
        visible: Some(now_is_true()),
        ..plain("a")
    };
    field_features(&shown, &mut used);
    assert!(used.contains(&Feature::Visibility));
}

#[test]
fn visibility_is_opt_in_and_named() {
    assert!(Feature::OPT_IN.contains(&Feature::Visibility));
    assert!(Feature::ALL.contains(&Feature::Visibility));
    assert_eq!(Feature::Visibility.name(), "visibility");
}

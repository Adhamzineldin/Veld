//! The saga in the IR (`spec/platform.md` sections 17 and 18).

use serde_json::json;

use super::*;
use crate::{
    DeclId, ExampleOutcome, ExampleValue, ExprKind, ExprLit, ExprType, Feature, PathRef, Root,
    Scalar, Step, Type,
};

fn origin(line: u32) -> Origin {
    Origin {
        file: "checkout.veld".into(),
        line,
        col: 3,
    }
}

fn string_ty() -> ExprType {
    ExprType::Type(Type::Scalar(Scalar::String))
}

fn expr_path(root: Root, name: &str) -> Expr {
    Expr {
        t: string_ty(),
        e: ExprKind::Path(PathRef {
            root,
            steps: vec![Step {
                name: name.into(),
                wire: name.into(),
                optional: false,
            }],
        }),
    }
}

/// Two steps: the first with a compensation, built from an object and from a step's field; the
/// second without one.
pub(crate) fn sample_saga() -> Saga {
    let hold = SagaCall {
        service: ServiceId(1),
        endpoint: "hold".into(),
        args: vec![SagaArg {
            name: "body".into(),
            value: SagaValue::Object {
                object: vec![SagaField {
                    name: "roomId".into(),
                    wire: "room_id".into(),
                    value: SagaValue::Expr(expr_path(Root::Param("body".into()), "roomId")),
                }],
            },
        }],
    };
    let release = SagaCall {
        service: ServiceId(1),
        endpoint: "release".into(),
        args: vec![SagaArg {
            name: "id".into(),
            value: SagaValue::Expr(expr_path(Root::Fact("hold".into()), "id")),
        }],
    };
    let confirm = SagaCall {
        service: ServiceId(2),
        endpoint: "confirm".into(),
        args: vec![SagaArg {
            name: "id".into(),
            value: SagaValue::Expr(Expr {
                t: string_ty(),
                e: ExprKind::Lit(ExprLit::String("fixed".into())),
            }),
        }],
    };
    Saga {
        steps: vec![
            SagaStep {
                name: "hold".into(),
                call: hold,
                compensate: Some(release),
                origin: origin(4),
            },
            SagaStep {
                name: "confirm".into(),
                call: confirm,
                compensate: None,
                origin: origin(5),
            },
        ],
        timeout_ms: 300_000,
    }
}

/// A saga example per form of answer and of expectation.
pub(crate) fn sample_saga_example() -> ExampleSaga {
    ExampleSaga {
        steps: vec![
            SagaGiven {
                step: "hold".into(),
                answer: SagaAnswer::Answer(ExampleOutcome::Ok(Some(ExampleValue::Json(
                    json!({"id": "h-1"}),
                )))),
            },
            SagaGiven {
                step: "confirm".into(),
                answer: SagaAnswer::Unavailable,
            },
        ],
        undos: vec![SagaGiven {
            step: "hold".into(),
            answer: SagaAnswer::Answer(ExampleOutcome::Error(crate::ExampleErrorOutcome {
                decl: DeclId(2),
                pattern: None,
            })),
        }],
        expect: Some(SagaExpect::Stuck),
    }
}

#[test]
fn the_saga_matches_the_json_of_the_spec() {
    let v = serde_json::to_value(sample_saga()).unwrap();
    assert_eq!(v["timeout_ms"], "300000");
    let hold = &v["steps"][0];
    assert_eq!(hold["name"], "hold");
    assert_eq!(hold["call"]["service"], 1);
    assert_eq!(hold["call"]["endpoint"], "hold");
    // An object argument is `{ "object": [..] }`, an expression is the expression itself.
    assert_eq!(
        hold["call"]["args"][0]["value"]["object"][0]["wire"],
        "room_id"
    );
    assert_eq!(
        hold["compensate"]["args"][0]["value"]["e"]["path"]["root"],
        json!({"fact": "hold"})
    );
    // A step without a compensation has no `compensate` key.
    assert!(v["steps"][1].get("compensate").is_none());
}

#[test]
fn the_saga_reads_back() {
    let saga = sample_saga();
    let text = serde_json::to_string(&saga).unwrap();
    let back: Saga = serde_json::from_str(&text).unwrap();
    assert_eq!(back, saga);
}

#[test]
fn the_example_of_a_saga_reads_back_and_is_absent_when_empty() {
    let ex = sample_saga_example();
    let text = serde_json::to_string(&ex).unwrap();
    let back: ExampleSaga = serde_json::from_str(&text).unwrap();
    assert_eq!(back, ex);
    let v = serde_json::to_value(&ex).unwrap();
    assert_eq!(v["steps"][1]["answer"], "unavailable");
    assert_eq!(v["expect"], "stuck");
    assert_eq!(
        serde_json::to_value(SagaExpect::Compensated(vec!["a".into(), "b".into()])).unwrap(),
        json!({"compensated": ["a", "b"]})
    );
    let empty = serde_json::to_value(ExampleSaga::default()).unwrap();
    assert_eq!(empty, json!({}));
}

#[test]
fn calls_lists_a_step_before_its_compensation() {
    let saga = sample_saga();
    let names: Vec<&str> = saga.calls().map(|c| c.endpoint.as_str()).collect();
    assert_eq!(names, ["hold", "release", "confirm"]);
}

#[test]
fn every_expression_of_a_value_is_listed() {
    let saga = sample_saga();
    let first = &saga.steps[0].call.args[0].value;
    assert_eq!(first.exprs().len(), 1);
    let both = SagaValue::Object {
        object: vec![
            SagaField {
                name: "a".into(),
                wire: "a".into(),
                value: SagaValue::Expr(expr_path(Root::Param("x".into()), "a")),
            },
            SagaField {
                name: "b".into(),
                wire: "b".into(),
                value: SagaValue::Expr(expr_path(Root::Param("x".into()), "b")),
            },
        ],
    };
    assert_eq!(both.exprs().len(), 2);
}

#[test]
fn sagas_is_an_opt_in_feature_with_a_name() {
    assert!(Feature::OPT_IN.contains(&Feature::Sagas));
    assert!(Feature::ALL.contains(&Feature::Sagas));
    assert_eq!(Feature::Sagas.name(), "sagas");
    assert_eq!(Feature::from_name("sagas"), Some(Feature::Sagas));
}

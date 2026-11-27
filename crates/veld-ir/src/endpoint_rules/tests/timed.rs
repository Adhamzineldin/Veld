//! Timed moves in the IR (`spec/platform.md` section 10): `after_ms` on a flow and `since` on a
//! transition, each omitted from the JSON when absent.

use super::*;

#[test]
fn a_flow_without_after_has_no_key() {
    let v = serde_json::to_value(flow("pending", "paid")).unwrap();
    assert_eq!(v, json!({ "from": "pending", "to": "paid" }));
}

#[test]
fn a_timed_flow_writes_its_milliseconds_as_text() {
    let timed = Flow {
        after_ms: Some("86400000".into()),
        ..flow("pending", "cancelled")
    };
    let v = serde_json::to_value(&timed).unwrap();
    assert_eq!(v["after_ms"], "86400000");
    let back: Flow = serde_json::from_value(v).unwrap();
    assert_eq!(back, timed);
    assert!(timed.is_timed());
    assert!(!flow("pending", "paid").is_timed());
}

#[test]
fn a_transition_writes_since_only_when_it_is_timed() {
    let plain = sample_transition();
    assert!(!plain.is_timed());
    let v = serde_json::to_value(&plain).unwrap();
    assert!(v.get("since").is_none());
    let timed = EndpointTransition {
        since: vec![step("pendingSince")],
        ..plain
    };
    assert!(timed.is_timed());
    let v = serde_json::to_value(&timed).unwrap();
    assert_eq!(v["since"][0]["name"], "pendingSince");
    let back: EndpointTransition = serde_json::from_value(v).unwrap();
    assert_eq!(back, timed);
}

#[test]
fn a_timed_edge_uses_the_opt_in_feature() {
    assert!(Feature::OPT_IN.contains(&Feature::RulesTimed));
    assert_eq!(Feature::from_name("rules_timed"), Some(Feature::RulesTimed));
    let timed = Enum {
        flows: vec![Flow {
            after_ms: Some("1000".into()),
            ..flow("pending", "cancelled")
        }],
        ..machine(vec![], vec![])
    };
    assert!(timed.has_timed_edge());
    assert!(!machine(vec![flow("pending", "paid")], vec![]).has_timed_edge());
}

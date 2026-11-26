//! `@timeout` and `@slo` in the IR (`spec/platform.md` sections 12 and 13): the keys are omitted
//! when absent and written as decimal text.

use super::*;

#[test]
fn nothing_is_written_for_an_endpoint_without_the_annotations() {
    let v = serde_json::to_value(endpoint_with(EndpointRules::default())).unwrap();
    assert!(v.get("timeout_ms").is_none());
    assert!(v.get("slo").is_none());
}

#[test]
fn a_deadline_and_a_service_level_are_written_as_text() {
    let mut ep = endpoint_with(EndpointRules::default());
    ep.timeout_ms = Some("2000".into());
    ep.slo = Some(Slo {
        p99_ms: Some("300".into()),
        errors: Some("0.1".into()),
        ..Slo::default()
    });
    let v = serde_json::to_value(&ep).unwrap();
    assert_eq!(v["timeout_ms"], "2000");
    assert_eq!(v["slo"], json!({ "p99_ms": "300", "errors": "0.1" }));
    let back: Endpoint = serde_json::from_value(v).unwrap();
    assert_eq!(back, ep);
}

#[test]
fn the_latencies_come_fastest_share_first() {
    let slo = Slo {
        p99_ms: Some("300".into()),
        p50_ms: Some("50".into()),
        ..Slo::default()
    };
    assert_eq!(slo.latencies(), [("p50", "50"), ("p99", "300")]);
}

#[test]
fn the_two_features_are_opt_in() {
    for f in [Feature::Timeouts, Feature::Slo] {
        assert!(Feature::OPT_IN.contains(&f));
        assert_eq!(Feature::from_name(f.name()), Some(f));
    }
}

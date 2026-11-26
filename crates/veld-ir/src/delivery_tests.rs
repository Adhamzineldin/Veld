//! The delivery annotations of a channel in the IR, and the feature `events_delivery`
//! (`spec/events.md` sections 10.8 and 13).

use serde_json::json;

use crate::endpoint_rules::tests::origin;
use crate::events_tests::world;
use crate::*;

fn retry(max: u32, backoff: Backoff) -> Option<Retry> {
    Some(Retry { max, backoff })
}

#[test]
fn a_channel_without_the_annotations_keeps_its_json() {
    let ir = world();
    for c in &ir.channels {
        let v = serde_json::to_value(c).unwrap();
        let o = v.as_object().unwrap();
        for gone in ["retry", "dead_letter", "ordered"] {
            assert!(!o.contains_key(gone), "{gone}");
        }
        assert!(!c.has_delivery_options());
    }
}

#[test]
fn the_annotations_have_the_documented_json_shape_and_round_trip() {
    let mut c = world().channels[0].clone();
    c.retry = retry(5, Backoff::Exponential);
    c.dead_letter = true;
    c.ordered = true;
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(v["retry"], json!({ "max": 5, "backoff": "exponential" }));
    assert_eq!(v["dead_letter"], json!(true));
    assert_eq!(v["ordered"], json!(true));
    let back: Channel = serde_json::from_value(v).unwrap();
    assert_eq!(back, c);
    c.retry = retry(0, Backoff::Fixed);
    c.dead_letter = false;
    c.ordered = false;
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(v["retry"], json!({ "max": 0, "backoff": "fixed" }));
    assert!(v.get("dead_letter").is_none() && v.get("ordered").is_none());
    assert!(c.has_delivery_options());
}

#[test]
fn called_once_is_a_key_that_stays_out_when_false() {
    let plain = ExpectedCall {
        service: ServiceId(0),
        handler: "h".into(),
        called: true,
        once: false,
    };
    let once = ExpectedCall {
        once: true,
        ..plain.clone()
    };
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        json!({"service": 0, "handler": "h", "called": true})
    );
    let v = serde_json::to_value(&once).unwrap();
    assert_eq!(
        v,
        json!({"service": 0, "handler": "h", "called": true, "once": true})
    );
    assert_eq!(serde_json::from_value::<ExpectedCall>(v).unwrap(), once);
}

#[test]
fn the_feature_is_opt_in_and_has_a_name() {
    assert_eq!(Feature::EventsDelivery.name(), "events_delivery");
    assert_eq!(
        Feature::from_name("events_delivery"),
        Some(Feature::EventsDelivery)
    );
    assert!(Feature::OPT_IN.contains(&Feature::EventsDelivery));
    assert!(!Feature::OPT_IN.contains(&Feature::Events));
}

/// Service 0 publishes channels 0 and 1; service 1 subscribes to channel 2; service 2 has none.
#[test]
fn a_service_uses_the_feature_through_the_channels_it_publishes_or_subscribes_to() {
    let mut ir = world();
    assert!(
        !ir.event_features(&[ServiceId(0)])
            .contains(&Feature::EventsDelivery)
    );
    ir.channels[1].ordered = true;
    ir.channels[1].origin = origin(77);
    assert!(
        ir.event_features(&[ServiceId(0)])
            .contains(&Feature::EventsDelivery)
    );
    assert!(
        !ir.event_features(&[ServiceId(1)])
            .contains(&Feature::EventsDelivery)
    );
    assert!(
        !ir.event_features(&[ServiceId(2)])
            .contains(&Feature::EventsDelivery)
    );
    assert_eq!(ir.first_delivery_origin(ServiceId(0)), Some(&origin(77)));
    assert_eq!(ir.first_delivery_origin(ServiceId(1)), None);
    ir.channels[2].retry = retry(3, Backoff::Fixed);
    ir.channels[2].origin = origin(55);
    assert!(
        ir.event_features(&[ServiceId(1)])
            .contains(&Feature::EventsDelivery)
    );
    assert_eq!(ir.first_delivery_origin(ServiceId(1)), Some(&origin(55)));
    // The first channel of the `publishes` lines wins, in source order.
    ir.channels[0].dead_letter = true;
    ir.channels[0].origin = origin(44);
    assert_eq!(ir.first_delivery_origin(ServiceId(0)), Some(&origin(44)));
}

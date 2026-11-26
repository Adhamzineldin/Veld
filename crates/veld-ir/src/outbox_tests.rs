//! `emits` in the IR and the feature `events_outbox` (`spec/events.md` sections 11 and 13).

use serde_json::json;

use crate::endpoint_rules::TransitionEmit;
use crate::*;

#[test]
fn a_flow_without_emits_keeps_its_json_and_one_with_it_has_the_event_id() {
    let plain = Flow {
        from: "pending".into(),
        to: "paid".into(),
        emits: None,
    };
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        json!({"from": "pending", "to": "paid"})
    );
    let emitting = Flow {
        emits: Some(DeclId(4)),
        ..plain
    };
    let v = serde_json::to_value(&emitting).unwrap();
    assert_eq!(v, json!({"from": "pending", "to": "paid", "emits": 4}));
    assert_eq!(serde_json::from_value::<Flow>(v).unwrap(), emitting);
    // An IR written before `emits` existed still reads.
    let old: Flow = serde_json::from_value(json!({"from": "a", "to": "b"})).unwrap();
    assert_eq!(old.emits, None);
}

#[test]
fn a_transition_emit_is_an_event_and_a_channel_index() {
    let e = TransitionEmit {
        event: DeclId(7),
        channel: ChannelId(2),
    };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v, json!({"event": 7, "channel": 2}));
    assert_eq!(serde_json::from_value::<TransitionEmit>(v).unwrap(), e);
}

#[test]
fn the_feature_is_opt_in_and_has_a_name() {
    assert_eq!(Feature::EventsOutbox.name(), "events_outbox");
    assert_eq!(
        Feature::from_name("events_outbox"),
        Some(Feature::EventsOutbox)
    );
    assert!(Feature::OPT_IN.contains(&Feature::EventsOutbox));
    assert!(Feature::ALL.contains(&Feature::EventsOutbox));
}

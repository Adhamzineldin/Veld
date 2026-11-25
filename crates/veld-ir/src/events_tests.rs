//! JSON of events: the exact text, the keys that stay out, and the round trips
//! (`spec/events.md` section 13).

use serde_json::{Value, json};

use crate::endpoint_rules::tests::{origin, sample_event_examples};
use crate::*;

fn channel() -> Channel {
    Channel {
        name: "BookingEvents".into(),
        package: PackageId(0),
        module: "bookings".into(),
        qualified: "bookings.BookingEvents".into(),
        topic: "shop.bookings.v1".into(),
        events: vec![DeclId(1), DeclId(2)],
        key: Some(ChannelKey {
            name: "bookingId".into(),
            wire: "bookingId".into(),
            ty: Type::Scalar(Scalar::Uuid),
        }),
        docs: None,
        deprecated: None,
        origin: origin(3),
    }
}

fn service(publishes: Vec<ChannelId>, subscriptions: Vec<Subscription>) -> Service {
    Service {
        id: ServiceId(0),
        package: PackageId(0),
        name: "Notifications".into(),
        prefix: String::new(),
        base_url: None,
        docs: None,
        auth: Auth::none(),
        middleware: vec![],
        endpoints: vec![],
        facts: vec![],
        rules_digest: None,
        principal: None,
        publishes,
        subscriptions,
        origin: origin(1),
    }
}

#[test]
fn a_service_without_events_has_neither_key() {
    let v = serde_json::to_value(service(vec![], vec![])).unwrap();
    let o = v.as_object().unwrap();
    assert!(!o.contains_key("publishes"));
    assert!(!o.contains_key("subscriptions"));
    assert_eq!(v["prefix"], json!(""));
}

#[test]
fn an_ir_without_channels_has_no_channels_key() {
    let ir = Ir {
        ir_version: IR_VERSION.into(),
        packages: vec![],
        decls: vec![],
        services: vec![],
        consumes: Default::default(),
        examples: vec![],
        policies: vec![],
        channels: vec![],
    };
    let text = serde_json::to_string(&ir).unwrap();
    assert_eq!(
        text,
        r#"{"ir_version":"1.1","packages":[],"decls":[],"services":[],"consumes":{}}"#
    );
}

#[test]
fn a_channel_has_the_documented_json_shape() {
    let v = serde_json::to_value(channel()).unwrap();
    assert_eq!(v["topic"], "shop.bookings.v1");
    assert_eq!(v["events"], json!([1, 2]));
    assert_eq!(
        v["key"],
        json!({"name": "bookingId", "wire": "bookingId", "type": {"scalar": "uuid"}})
    );
    let none = Channel {
        key: None,
        ..channel()
    };
    assert_eq!(serde_json::to_value(none).unwrap()["key"], Value::Null);
}

#[test]
fn a_subscription_round_trips_in_source_order() {
    let sub = Subscription {
        channel: ChannelId(0),
        group: "notifications".into(),
        handlers: vec![
            Handler {
                event: DeclId(1),
                name: "sendReceipt".into(),
                docs: None,
                origin: origin(5),
            },
            Handler {
                event: DeclId(2),
                name: "sendApology".into(),
                docs: Some("d".into()),
                origin: origin(6),
            },
        ],
        origin: origin(4),
    };
    let s = service(vec![ChannelId(0)], vec![sub]);
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["publishes"], json!([0]));
    assert_eq!(v["subscriptions"][0]["handlers"][1]["name"], "sendApology");
    let back: Service = serde_json::from_value(v).unwrap();
    assert_eq!(back, s);
    assert!(s.uses_events());
    assert_eq!(s.first_event_origin(), Some(&origin(1)));
}

#[test]
fn an_event_declaration_omits_empty_rules_and_digest() {
    let kind = DeclKind::Event(Event {
        fields: vec![],
        rules: vec![],
        rules_digest: None,
    });
    assert_eq!(
        serde_json::to_string(&kind).unwrap(),
        r#"{"kind":"event","fields":[]}"#
    );
    let with = DeclKind::Event(Event {
        fields: vec![],
        rules: vec![],
        rules_digest: Some("3fa9c2e1b7d04a55".into()),
    });
    let text = serde_json::to_string(&with).unwrap();
    assert_eq!(
        text,
        r#"{"kind":"event","fields":[],"rules_digest":"3fa9c2e1b7d04a55"}"#
    );
    assert_eq!(serde_json::from_str::<DeclKind>(&text).unwrap(), with);
}

#[test]
fn event_examples_round_trip_and_leave_the_call_keys_out() {
    for ex in sample_event_examples() {
        let v = serde_json::to_value(&ex).unwrap();
        let o = v.as_object().unwrap();
        for gone in ["service", "endpoint", "outcome"] {
            assert!(!o.contains_key(gone), "{gone}");
        }
        assert!(o.contains_key("event"));
        let back: Example = serde_json::from_value(v).unwrap();
        assert_eq!(back, ex);
    }
}

#[test]
fn the_expectation_is_a_word_or_a_list() {
    let exs = sample_event_examples();
    let calls = serde_json::to_value(&exs[0]).unwrap();
    assert_eq!(
        calls["expect"][0],
        json!({"service": 0, "handler": "sendReceipt", "called": true})
    );
    let rejected = serde_json::to_value(&exs[1]).unwrap();
    assert_eq!(rejected["expect"], json!("rejected"));
    assert!(serde_json::from_value::<EventExpect>(json!("maybe")).is_err());
}

#[test]
fn the_events_feature_has_a_name() {
    assert_eq!(Feature::Events.name(), "events");
    assert_eq!(Feature::from_name("events"), Some(Feature::Events));
}

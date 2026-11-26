//! JSON of events: the exact text, the keys that stay out, and the round trips
//! (`spec/events.md` section 13).

use serde_json::{Value, json};

use crate::endpoint_rules::tests::{origin, sample_event_examples};
use crate::*;

pub(crate) fn channel() -> Channel {
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
        retry: None,
        dead_letter: false,
        ordered: false,
        origin: origin(3),
    }
}

pub(crate) fn service(publishes: Vec<Publication>, subscriptions: Vec<Subscription>) -> Service {
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
    let s = service(
        vec![Publication {
            channel: ChannelId(0),
            origin: origin(2),
        }],
        vec![sub],
    );
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["publishes"][0]["channel"], json!(0));
    assert_eq!(
        v["publishes"][0]["origin"],
        serde_json::to_value(origin(2)).unwrap()
    );
    assert_eq!(v["subscriptions"][0]["handlers"][1]["name"], "sendApology");
    let back: Service = serde_json::from_value(v).unwrap();
    assert_eq!(back, s);
    assert!(s.uses_events());
    assert_eq!(s.first_event_origin(), Some(&origin(2)));
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

// ---- the helpers the event generators use ----------------------------------------------------

fn field(name: &str, ty: Type) -> Field {
    Field {
        name: name.into(),
        wire: name.into(),
        ty,
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

fn decl(id: u32, name: &str, kind: DeclKind) -> Decl {
    Decl {
        id: DeclId(id),
        package: PackageId(0),
        module: "m".into(),
        name: name.into(),
        qualified: format!("m.{name}"),
        sig: format!("veld:p/{name}@0"),
        docs: None,
        deprecated: None,
        origin: origin(id + 1),
        kind,
    }
}

fn model(parent: Option<DeclId>, fields: Vec<Field>) -> DeclKind {
    DeclKind::Model(Model {
        parent,
        fields,
        rules: vec![],
        retain: None,
    })
}

fn event(fields: Vec<Field>) -> DeclKind {
    DeclKind::Event(Event {
        fields,
        rules: vec![],
        rules_digest: None,
    })
}

fn publication(channel: u32, line: u32) -> Publication {
    Publication {
        channel: ChannelId(channel),
        origin: origin(line),
    }
}

/// Decls: 0 Base, 1 Money (extends Base), 2 Paid (a Money field, a list of Tag), 3 Left, 4 Other,
/// 5 Tag, 6 Unused. Channels: 0 carries Paid and Left, 1 carries Other, 2 carries Left.
pub(crate) fn world() -> Ir {
    let c = |name: &str, events: Vec<u32>| Channel {
        events: events.into_iter().map(DeclId).collect(),
        name: name.into(),
        qualified: format!("m.{name}"),
        ..channel()
    };
    let channels = vec![c("A", vec![2, 3]), c("B", vec![4]), c("C", vec![3])];
    let svc = |id: u32, publishes: Vec<Publication>, subs: Vec<Subscription>| Service {
        id: ServiceId(id),
        ..service(publishes, subs)
    };
    let sub = |channel: u32| Subscription {
        channel: ChannelId(channel),
        group: "g".into(),
        handlers: vec![],
        origin: origin(9),
    };
    Ir {
        ir_version: IR_VERSION.into(),
        packages: vec![],
        decls: vec![
            decl(0, "Base", model(None, vec![])),
            decl(1, "Money", model(Some(DeclId(0)), vec![])),
            decl(
                2,
                "Paid",
                event(vec![
                    field("price", Type::Decl(DeclId(1))),
                    field("tags", Type::List(Box::new(Type::Decl(DeclId(5))))),
                ]),
            ),
            decl(3, "Left", event(vec![])),
            decl(4, "Other", event(vec![])),
            decl(5, "Tag", model(None, vec![])),
            decl(6, "Unused", model(None, vec![])),
        ],
        services: vec![
            svc(0, vec![publication(0, 4), publication(1, 4)], vec![sub(0)]),
            svc(1, vec![], vec![sub(2)]),
            svc(2, vec![], vec![]),
        ],
        consumes: Default::default(),
        examples: vec![],
        policies: vec![],
        channels,
    }
}

fn ids(v: &[DeclId]) -> Vec<u32> {
    v.iter().map(|d| d.0).collect()
}

#[test]
fn the_channels_of_services_are_listed_once_in_id_order() {
    let ir = world();
    let all = |s: &[u32]| -> Vec<u32> {
        let s: Vec<ServiceId> = s.iter().map(|i| ServiceId(*i)).collect();
        ir.event_channels(&s).iter().map(|c| c.0).collect()
    };
    assert_eq!(all(&[0]), [0, 1]);
    assert_eq!(all(&[1, 0]), [0, 1, 2]);
    assert!(all(&[2]).is_empty() && all(&[]).is_empty());
}

#[test]
fn the_events_of_services_are_those_of_their_channels_without_duplicates() {
    let ir = world();
    assert_eq!(ids(&ir.events_of(&[ServiceId(0)])), [2, 3, 4]);
    // `Left` is carried by channels A and C: once.
    assert_eq!(ids(&ir.events_of(&[ServiceId(0), ServiceId(1)])), [2, 3, 4]);
    assert_eq!(ids(&ir.events_of(&[ServiceId(1)])), [3]);
    assert!(ir.events_of(&[ServiceId(2)]).is_empty());
}

#[test]
fn what_the_events_reach_is_listed_through_fields_lists_and_parents() {
    let ir = world();
    // Paid reaches Money, Money's parent Base and Tag; Unused is reached by nothing.
    assert_eq!(ids(&ir.event_reachable_decls(&[ServiceId(0)])), [0, 1, 5]);
    assert!(ir.event_reachable_decls(&[ServiceId(1)]).is_empty());
    assert_eq!(ids(&ir.event_only_decls(&[ServiceId(0)])), [0, 1, 5]);
}

#[test]
fn the_publishes_origin_is_the_first_event_origin_and_a_service_lists_its_channels() {
    let ir = world();
    let s = ir.service(ServiceId(0));
    assert_eq!(s.first_event_origin(), Some(&origin(4)));
    assert_eq!(
        s.published_channels().map(|c| c.0).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        s.channels_used().iter().map(|c| c.0).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        ir.service(ServiceId(1)).first_event_origin(),
        Some(&origin(9))
    );
    assert_eq!(ir.service(ServiceId(2)).first_event_origin(), None);
}

#[test]
fn the_features_of_events_and_of_what_they_reach_belong_to_the_service_that_uses_them() {
    let mut ir = world();
    let only = |f: &[Feature]| f.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(ir.event_features(&[ServiceId(1)]), only(&[Feature::Events]));
    assert!(ir.event_features(&[ServiceId(2)]).is_empty());
    // Service 1 handles `Left` (decl 3): a `bytes` field with a constraint and a page.
    if let DeclKind::Event(e) = &mut ir.decls[3].kind {
        let mut blob = field("blob", Type::Scalar(Scalar::Bytes));
        blob.constraints.max_len = Some(4);
        e.fields.push(blob);
        e.fields.push(field(
            "rows",
            Type::Page(Box::new(Type::Scalar(Scalar::String))),
        ));
    }
    let want = only(&[
        Feature::Events,
        Feature::Bytes,
        Feature::Constraints,
        Feature::Page,
    ]);
    assert_eq!(ir.event_features(&[ServiceId(1)]), want);
    assert_eq!(ir.features_used(&[ServiceId(1)]), want);
    // Service 0 reaches `Money` (decl 1) through `Paid`: a declaration an event reaches counts.
    if let DeclKind::Model(m) = &mut ir.decls[1].kind {
        m.fields.push(field("raw", Type::Scalar(Scalar::Bytes)));
    }
    assert!(ir.event_features(&[ServiceId(0)]).contains(&Feature::Bytes));
    // A service with no events does not use them.
    assert!(ir.features_used(&[ServiceId(2)]).is_empty());
}

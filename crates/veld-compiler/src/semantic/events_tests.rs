//! Events (`spec/events.md`): checks, lowering, identity and the rules digest.

use crate::testing::{codes, ir, multi};
use veld_ir::{DeclKind, EventExpect, Feature};

const BOOKINGS: &str = r#"
event BookingPaid {
  bookingId: uuid
  amount: decimal
  nights: int = 1
  rule positive: amount > 0
}
event BookingCancelled { bookingId: uuid }

@topic("shop.bookings.v1")
channel BookingEvents {
  carries BookingPaid, BookingCancelled
  key bookingId
}

service Bookings at "/bookings" {
  publishes BookingEvents
  get "/ping" ping() -> 204
}

service Notifications {
  subscribes BookingEvents as "notifications" {
    on BookingPaid -> sendReceipt
    on BookingCancelled -> sendApology
  }
}
"#;

fn with(extra: &str) -> String {
    format!("{BOOKINGS}\n{extra}")
}

#[test]
fn events_channels_and_subscriptions_reach_the_ir() {
    let ir = ir(BOOKINGS);
    let paid = ir.decls.iter().find(|d| d.name == "BookingPaid").unwrap();
    let DeclKind::Event(e) = &paid.kind else {
        panic!("not an event")
    };
    assert_eq!(e.fields.len(), 3);
    assert_eq!(e.rules.len(), 1);
    assert!(e.rules_digest.is_some());
    assert!(
        e.rules[0].id.starts_with("veld:app/BookingPaid.positive@"),
        "{}",
        e.rules[0].id
    );
    assert_eq!(ir.channels.len(), 1);
    let c = &ir.channels[0];
    assert_eq!(c.topic, "shop.bookings.v1");
    assert_eq!(c.events.len(), 2);
    assert_eq!(c.key.as_ref().unwrap().name, "bookingId");
    let bookings = ir.services.iter().find(|s| s.name == "Bookings").unwrap();
    assert_eq!(bookings.publishes.len(), 1);
    assert_eq!(bookings.prefix, "/bookings");
    let n = ir
        .services
        .iter()
        .find(|s| s.name == "Notifications")
        .unwrap();
    assert_eq!(n.prefix, "");
    assert_eq!(n.subscriptions[0].group, "notifications");
    assert_eq!(n.subscriptions[0].handlers[1].name, "sendApology");
    assert!(n.uses_events() && bookings.uses_events());
    assert!(
        ir.features_used(&[n.id])
            .iter()
            .any(|f| *f == Feature::Events)
    );
}

#[test]
fn the_default_topic_is_package_and_channel() {
    let ir = ir(
        "event E { id: uuid }\nchannel Orders { carries E }\nservice P { publishes Orders }\nservice S { subscribes Orders as \"g\" { on E -> h } }",
    );
    assert_eq!(ir.channels[0].topic, "app.Orders");
}

#[test]
fn a_contract_without_events_has_no_event_keys() {
    let ir = ir("model A { a: int }\nservice S at \"/s\" { get \"/a\" a() -> A }");
    let text = serde_json::to_string(&ir).unwrap();
    for key in [
        "channels",
        "publishes",
        "subscriptions",
        "\"event\"",
        "\"expect\"",
    ] {
        assert!(!text.contains(key), "{key}");
    }
}

#[test]
fn an_event_is_not_a_type() {
    assert_eq!(codes("event P { id: uuid }\nmodel H { p: P }"), ["E1001"]);
    assert_eq!(codes("event P { id: uuid }\nevent Q { p: P }"), ["E1001"]);
    assert_eq!(
        codes("event P { id: uuid }\nservice S at \"/s\" { get \"/p\" p() -> P }"),
        ["E1001"]
    );
}

#[test]
fn carries_checks() {
    let src = "event P { id: uuid }\nmodel M { id: uuid }\nchannel A { carries P, P }\nchannel B { carries M }\nchannel C { }";
    assert_eq!(codes(src), ["E1002", "E1002", "E1002"]);
}

#[test]
fn topics_are_checked_and_unique() {
    let bad = "event P { id: uuid }\n@topic(\"a b\")\nchannel A { carries P }";
    assert_eq!(codes(bad), ["E1003"]);
    let dlq = "event P { id: uuid }\n@topic(\"x.dlq\")\nchannel A { carries P }";
    assert_eq!(codes(dlq), ["E1003"]);
    let dup = "event P { id: uuid }\n@topic(\"t\")\nchannel A { carries P }\n@topic(\"t\")\nchannel B { carries P }";
    assert_eq!(codes(dup), ["E1004"]);
}

#[test]
fn the_key_is_a_required_field_of_one_key_type() {
    let ev = "event A { id: uuid\n n?: int\n f: float\n s: string }\nevent B { id: string }\n";
    for (chan, ok) in [
        ("channel C { carries A key id }", true),
        ("channel C { carries A key s }", true),
        ("channel C { carries A key nope }", false),
        ("channel C { carries A key n }", false),
        ("channel C { carries A key f }", false),
        ("channel C { carries A, B key id }", false),
        ("channel C { carries A key id key s }", false),
    ] {
        let found = codes(&format!("{ev}{chan}"));
        assert_eq!(found.is_empty(), ok, "{chan}: {found:?}");
        if !ok {
            assert_eq!(found, ["E1005"], "{chan}");
        }
    }
}

#[test]
fn publishes_and_subscribes_check_their_names() {
    let base = "event P { id: uuid }\nmodel M { a: int }\nchannel A { carries P }\n";
    let dup_pub = format!(
        "{base}service S {{ publishes A, A }}\nservice T {{ subscribes A as \"g\" {{ on P -> h }} }}"
    );
    assert_eq!(codes(&dup_pub), ["E1006"]);
    let not_channel = format!("{base}service S {{ publishes M }}");
    assert_eq!(codes(&not_channel), ["E1006"]);
    let two = format!(
        "{base}service S {{ publishes A }}\nservice T {{ subscribes A as \"g\" {{ on P -> h }}\n subscribes A as \"g2\" {{ on P -> i }} }}"
    );
    assert_eq!(codes(&two), ["E1006"]);
    let group = format!(
        "{base}service S {{ publishes A }}\nservice T {{ subscribes A as \"a b\" {{ on P -> h }} }}"
    );
    assert_eq!(codes(&group), ["E1007"]);
}

#[test]
fn on_lines_check_the_channel_and_the_names() {
    let base = "event P { id: uuid }\nevent Q { id: uuid }\nchannel A { carries P }\nservice S { publishes A }\n";
    let not_carried = format!("{base}service T {{ subscribes A as \"g\" {{ on Q -> h }} }}");
    assert_eq!(codes(&not_carried), ["E1008"]);
    let twice = format!("{base}service T {{ subscribes A as \"g\" {{ on P -> h\n on P -> i }} }}");
    assert_eq!(codes(&twice), ["E1008"]);
    let empty = format!("{base}service T {{ subscribes A as \"g\" {{ }} }}");
    assert_eq!(codes(&empty), ["E1008"]);
    let clash = format!(
        "{base}service T at \"/t\" {{ get \"/h\" h() -> 204\n subscribes A as \"g\" {{ on P -> h }} }}"
    );
    assert_eq!(codes(&clash), ["E0509"]);
}

#[test]
fn a_service_with_endpoints_needs_at() {
    assert_eq!(codes("service S { get \"/x\" x() -> 204 }"), ["E0501"]);
    assert!(codes("service S { }").is_empty());
}

#[test]
fn the_graph_of_publishers_and_subscribers() {
    let base = "event P { id: uuid }\nevent Q { id: uuid }\nchannel A { carries P, Q }\n";
    let orphan = format!("{base}service S {{ subscribes A as \"g\" {{ on P -> h\n on Q -> i }} }}");
    assert_eq!(codes(&orphan), ["E1009"]);
    let nobody = format!("{base}service S {{ publishes A }}");
    assert_eq!(codes(&nobody), ["W1001"]);
    let partial = format!(
        "{base}service S {{ publishes A }}\nservice T {{ subscribes A as \"g\" {{ on P -> h }} }}"
    );
    assert_eq!(codes(&partial), ["W1002"]);
    let cycle = format!(
        "{base}service S {{ publishes A\n subscribes A as \"g\" {{ on P -> h\n on Q -> i }} }}"
    );
    assert!(codes(&cycle).is_empty());
}

#[test]
fn a_channel_of_a_library_is_exempt_from_e1009() {
    use crate::{CompileInput, PackageInput, SourceFile, compile};
    use veld_ir::PackageKind;
    let pkg = |name: &str, kind, text: &str| PackageInput {
        name: name.into(),
        kind,
        files: vec![SourceFile {
            path: "app.veld".into(),
            text: text.into(),
        }],
        base_url: None,
    };
    let out = compile(&CompileInput {
        packages: vec![
            pkg(
                "lib",
                PackageKind::Library,
                "event P { id: uuid }\nchannel A { carries P }",
            ),
            pkg(
                "svc",
                PackageKind::Service,
                "use lib:app.{A, P}\nservice S { subscribes A as \"g\" { on P -> h } }",
            ),
        ],
        consumes: Default::default(),
    });
    assert!(
        out.diagnostics.iter().all(|d| d.code != "E1009"),
        "{:?}",
        out.diagnostics
    );
    assert!(out.ir.is_some());
}

#[test]
fn channels_and_events_import_across_packages() {
    let out = multi(&[
        (
            "bookings",
            "events.veld",
            "event P { id: uuid }\nchannel A { carries P }\nservice B at \"/b\" { publishes A\n get \"/x\" x() -> 204 }",
        ),
        (
            "notify",
            "app.veld",
            "use bookings:events.{A, P}\nservice N { subscribes A as \"g\" { on P -> h } }\nexample \"e\" { given event P { }\n expect N.h called }",
        ),
    ]);
    assert!(out.ir.is_some(), "{:?}", out.diagnostics);
    let ir = out.ir.unwrap();
    assert_eq!(ir.channels.len(), 1);
    let ex = &ir.examples[0];
    assert!(matches!(ex.expect, Some(EventExpect::Calls(_))));
}

#[test]
fn event_examples_choose_the_channel_and_name_handlers() {
    let two = with(
        "channel Audit { carries BookingPaid }\nservice Auditor { publishes Audit }\nexample \"x\" { given event BookingPaid { amount: 1.00 }\n expect Notifications.sendReceipt called }",
    );
    assert_eq!(codes(&two), ["E1010"]);
    let ok = with(
        "channel Audit { carries BookingPaid }\nservice Auditor { publishes Audit }\nexample \"x\" { given event BookingPaid { amount: 1.00 } on BookingEvents\n expect Notifications.sendReceipt called }",
    );
    assert!(codes(&ok).iter().all(|c| *c != "E1010"));
    let nope = with(
        "example \"x\" { given event BookingPaid { amount: 1.00 }\n expect Notifications.nope called }",
    );
    assert_eq!(codes(&nope), ["E1010"]);
    let mixed = with(
        "example \"x\" { given event BookingPaid { amount: 1.00 }\n expect rejected\n expect Notifications.sendReceipt called }",
    );
    assert_eq!(codes(&mixed), ["E1010"]);
    let bad_field = with("example \"x\" { given event BookingPaid { nope: 1 }\n expect rejected }");
    assert_eq!(codes(&bad_field), ["E0952"]);
}

#[test]
fn readonly_is_refused_on_an_event_field() {
    assert_eq!(codes("event P { id: uuid @readonly }"), ["E0602"]);
}

#[test]
fn annotations_of_later_phases_are_unknown() {
    let src = "event P { id: uuid }\n@retry(max: 3)\nchannel A { carries P }";
    assert_eq!(codes(src), ["E0601"]);
    assert_eq!(
        codes("event P { id: uuid }\n@ordered\nchannel A { carries P }"),
        ["E0601"]
    );
    assert_eq!(
        codes("event P { id: uuid }\n@deadLetter\nchannel A { carries P }"),
        ["E0601"]
    );
}

#[test]
fn topic_is_only_for_channels() {
    assert_eq!(codes("@topic(\"x\")\nmodel M { a: int }"), ["E0602"]);
}

#[test]
fn flow_emits_is_still_a_syntax_error() {
    assert!(codes("enum S { a, b\n flow a -> b emits P }").contains(&"E0003"));
}

#[test]
fn an_event_and_a_model_with_the_same_fields_have_different_signatures() {
    let ir = ir("event Same { a: int, b: string }\nmodel Other { a: int, b: string }");
    let sig = |n: &str| ir.decls.iter().find(|d| d.name == n).unwrap().sig.clone();
    assert!(sig("Same").starts_with("veld:app/Same@"));
    assert_ne!(
        sig("Same").rsplit('@').next(),
        sig("Other").rsplit('@').next()
    );
}

#[test]
fn an_event_that_reaches_no_invariant_has_no_digest() {
    let ir = ir("event P { id: uuid }");
    let DeclKind::Event(e) = &ir.decls.iter().find(|d| d.name == "P").unwrap().kind else {
        panic!("not an event")
    };
    assert!(e.rules_digest.is_none());
}

#[test]
fn the_digest_is_the_hash_of_the_sorted_rule_ids_of_what_the_event_reaches() {
    use crate::signature::{Json, canonical, hex16};
    let ir = ir(
        "model Inner { n: int\n rule small: n < 10 }\nevent P { inner: Inner\n n: int\n rule pos: n > 0 }",
    );
    let p = ir.decls.iter().find(|d| d.name == "P").unwrap();
    let inner = ir.decls.iter().find(|d| d.name == "Inner").unwrap();
    let mut ids = vec![p.rules()[0].id.clone(), inner.rules()[0].id.clone()];
    ids.sort();
    let expected =
        hex16(canonical(&Json::Arr(ids.into_iter().map(Json::Str).collect())).as_bytes());
    let DeclKind::Event(e) = &p.kind else {
        panic!("not an event")
    };
    assert_eq!(e.rules_digest.as_deref(), Some(expected.as_str()));
}

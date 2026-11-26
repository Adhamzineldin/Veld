//! The AsyncAPI export (`spec/events.md` section 16.1).

use serde_json::{Value, json};
use veld_ir::ServiceId;

use super::{NoChannels, asyncapi};
use crate::testutil::ir;

const SRC: &str = r#"enum Reason { guest, hotel }

/// Published when a booking is paid.
event BookingPaid {
  bookingId: uuid
  amount: decimal
  reason?: Reason
  rule positive: amount > 0
}
event BookingCancelled { bookingId: uuid }

@topic("shop.bookings.v1")
@retry(max: 3, backoff: exponential)
@deadLetter
@ordered
channel BookingEvents {
  carries BookingPaid, BookingCancelled
  key bookingId
}

channel Audit { carries BookingPaid }

model Pong { ok: bool }

@auth(none)
service Bookings at "/bookings" {
  publishes BookingEvents, Audit
  get "/ping" ping() -> Pong
}

service Notifications {
  subscribes BookingEvents as "notifications" {
    on BookingPaid -> sendReceipt
  }
}
"#;

fn doc() -> Value {
    let ir = ir(SRC);
    asyncapi(&ir, &[]).unwrap()
}

fn keys(v: &Value) -> Vec<&str> {
    v.as_object().unwrap().keys().map(String::as_str).collect()
}

#[test]
fn the_document_is_stable() {
    assert_eq!(doc(), doc());
    insta::assert_snapshot!(serde_json::to_string_pretty(&doc()).unwrap());
}

#[test]
fn the_top_level_keys_come_in_a_fixed_order() {
    assert_eq!(
        keys(&doc()),
        [
            "asyncapi",
            "info",
            "defaultContentType",
            "channels",
            "operations",
            "components",
            "x-veld-ir-version"
        ]
    );
    assert_eq!(doc()["asyncapi"], "3.0.0");
    assert_eq!(doc()["info"]["title"], "Bookings, Notifications");
}

#[test]
fn a_channel_has_the_topic_as_its_address_and_a_message_per_event() {
    let d = doc();
    let c = &d["channels"]["app_app.BookingEvents"];
    assert_eq!(c["address"], "shop.bookings.v1");
    assert_eq!(
        keys(&c["messages"]),
        ["app.BookingCancelled", "app.BookingPaid"]
    );
    let m = &c["messages"]["app.BookingPaid"];
    assert_eq!(m["name"], "BookingPaid");
    assert_eq!(m["summary"], "Published when a booking is paid.");
    assert_eq!(
        m["contentType"],
        "application/cloudevents+json; charset=UTF-8"
    );
    assert!(
        m["x-veld-signature"]
            .as_str()
            .unwrap()
            .starts_with("veld:app/BookingPaid@")
    );
    assert!(m["x-veld-rules-digest"].is_string());
    assert!(
        c["messages"]["app.BookingCancelled"]
            .get("x-veld-rules-digest")
            .is_none()
    );
}

#[test]
fn the_payload_is_the_cloudevents_envelope_with_the_event_as_data() {
    let d = doc();
    let p = &d["channels"]["app_app.BookingEvents"]["messages"]["app.BookingPaid"]["payload"];
    assert_eq!(
        p["schemaFormat"],
        "application/schema+json;version=draft-07"
    );
    let s = &p["schema"];
    assert_eq!(
        s["properties"]["data"],
        json!({ "$ref": "#/components/schemas/app.BookingPaid" })
    );
    let required: Vec<&str> = s["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(required.contains(&"subject") && required.contains(&"data"));
    // A channel without a key has no `subject` to require.
    let a = &d["channels"]["app_app.Audit"]["messages"]["app.BookingPaid"]["payload"]["schema"];
    assert!(
        !a["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "subject")
    );
}

#[test]
fn the_event_schemas_are_the_ones_of_the_openapi_export() {
    let d = doc();
    let schemas = &d["components"]["schemas"];
    assert_eq!(
        keys(schemas),
        ["app.BookingCancelled", "app.BookingPaid", "app.Reason"]
    );
    assert_eq!(schemas["app.BookingPaid"]["type"], "object");
    assert_eq!(
        schemas["app.BookingPaid"]["properties"]["bookingId"]["format"],
        "uuid"
    );
    assert_eq!(schemas["app.Reason"]["enum"], json!(["guest", "hotel"]));
    assert!(schemas.get("app.Pong").is_none());
}

#[test]
fn the_delivery_options_and_the_key_are_extensions_of_the_channel() {
    let d = doc();
    let c = &d["channels"]["app_app.BookingEvents"];
    assert_eq!(
        c["x-veld-retry"],
        json!({ "max": 3, "backoff": "exponential" })
    );
    assert_eq!(
        c["x-veld-dead-letter"],
        json!({ "topic": "shop.bookings.v1.dlq" })
    );
    assert_eq!(c["x-veld-ordered"], true);
    assert_eq!(c["x-veld-key"]["wire"], "bookingId");
    assert_eq!(c["x-veld-key"]["type"]["format"], "uuid");
    let plain = &d["channels"]["app_app.Audit"];
    for k in [
        "x-veld-retry",
        "x-veld-dead-letter",
        "x-veld-ordered",
        "x-veld-key",
    ] {
        assert!(plain.get(k).is_none(), "{k}");
    }
}

#[test]
fn a_publisher_sends_and_a_subscriber_receives_the_messages_of_its_lines() {
    let d = doc();
    let ops = &d["operations"];
    assert_eq!(
        keys(ops),
        [
            "app.Bookings.publish.app_app.Audit",
            "app.Bookings.publish.app_app.BookingEvents",
            "app.Notifications.subscribe.app_app.BookingEvents"
        ]
    );
    let send = &ops["app.Bookings.publish.app_app.BookingEvents"];
    assert_eq!(send["action"], "send");
    assert_eq!(
        send["channel"],
        json!({ "$ref": "#/channels/app_app.BookingEvents" })
    );
    assert_eq!(send["x-veld-source"], "veld:app/Bookings");
    assert_eq!(send["messages"].as_array().unwrap().len(), 2);
    let recv = &ops["app.Notifications.subscribe.app_app.BookingEvents"];
    assert_eq!(recv["action"], "receive");
    assert_eq!(recv["x-veld-group"], "notifications");
    assert_eq!(
        recv["messages"],
        json!([{ "$ref": "#/channels/app_app.BookingEvents/messages/app.BookingPaid" }])
    );
    assert_eq!(
        recv["x-veld-handlers"],
        json!({ "app.BookingPaid": "sendReceipt" })
    );
}

#[test]
fn every_reference_points_into_the_document() {
    fn walk(root: &Value, v: &Value) {
        match v {
            Value::Object(m) => {
                if let Some(Value::String(r)) = m.get("$ref") {
                    let pointer = r.trim_start_matches('#');
                    assert!(root.pointer(pointer).is_some(), "{r}");
                }
                m.values().for_each(|x| walk(root, x));
            }
            Value::Array(a) => a.iter().for_each(|x| walk(root, x)),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    let d = doc();
    walk(&d, &d);
}

#[test]
fn one_service_exports_only_its_channels() {
    let ir = ir(SRC);
    let n: Vec<ServiceId> = ir
        .services
        .iter()
        .filter(|s| s.name == "Notifications")
        .map(|s| s.id)
        .collect();
    let d = asyncapi(&ir, &n).unwrap();
    assert_eq!(keys(&d["channels"]), ["app_app.BookingEvents"]);
    assert_eq!(keys(&d["operations"]).len(), 1);
    assert_eq!(d["info"]["title"], "Notifications");
}

#[test]
fn a_workspace_without_channels_has_nothing_to_export() {
    let plain = ir("model M { x: int }\nservice S at \"/s\" { get \"/\" f() -> M }\n");
    assert_eq!(
        asyncapi(&plain, &[]),
        Err(NoChannels("the workspace declares no channel"))
    );
    // A service that uses no channel, in a workspace that has one.
    let ir = ir(&format!(
        "{SRC}service Quiet at \"/q\" {{ get \"/\" q() -> Pong }}\n"
    ));
    let quiet: Vec<ServiceId> = ir
        .services
        .iter()
        .filter(|s| s.name == "Quiet")
        .map(|s| s.id)
        .collect();
    assert!(asyncapi(&ir, &quiet).is_err());
}

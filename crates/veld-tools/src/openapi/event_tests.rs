//! The OpenAPI export and events (`spec/events.md` section 5): an event has no HTTP surface.

use serde_json::Value;
use veld_ir::ServiceId;

use super::openapi;
use crate::testutil::ir;

const SRC: &str = r#"model Pong { ok: bool }
event Paid { id: uuid }
channel C { carries Paid }

@auth(none)
service Api at "/api" {
  publishes C
  get "/ping" ping() -> Pong
}

service Mailer {
  subscribes C as "mailer" { on Paid -> send }
}
"#;

fn schemas(doc: &Value) -> Vec<String> {
    doc["components"]["schemas"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn an_event_is_not_a_schema_and_a_service_without_endpoints_has_no_paths() {
    let ir = ir(SRC);
    let all: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    let doc = openapi(&ir, &all);
    assert_eq!(doc["paths"].as_object().map(|p| p.len()), Some(1));
    assert!(doc["paths"]["/api/ping"]["get"].is_object());
    assert!(
        !schemas(&doc).iter().any(|s| s.contains("Paid")),
        "{:?}",
        schemas(&doc)
    );
    let mailer: Vec<ServiceId> = ir
        .services
        .iter()
        .filter(|s| s.name == "Mailer")
        .map(|s| s.id)
        .collect();
    let doc = openapi(&ir, &mailer);
    assert_eq!(doc["paths"].as_object().map(|p| p.len()), Some(0));
    assert!(schemas(&doc).is_empty());
    assert_eq!(doc["info"]["title"], "Mailer");
}

#[test]
fn a_types_only_export_skips_events_too() {
    let doc = openapi(&ir(SRC), &[]);
    assert!(schemas(&doc).iter().any(|s| s.ends_with("Pong")));
    assert!(!schemas(&doc).iter().any(|s| s.contains("Paid")));
}

#[test]
fn the_document_of_a_contract_does_not_change_when_it_gains_events() {
    let plain = ir(
        "model Pong { ok: bool }\n@auth(none)\nservice Api at \"/api\" {\n  get \"/ping\" ping() -> Pong\n}\n",
    );
    let with = ir(SRC);
    let pick = |ir: &veld_ir::Ir| -> Vec<ServiceId> {
        ir.services
            .iter()
            .filter(|s| s.name == "Api")
            .map(|s| s.id)
            .collect()
    };
    assert_eq!(openapi(&plain, &pick(&plain)), openapi(&with, &pick(&with)));
}

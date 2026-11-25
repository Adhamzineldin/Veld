//! The playground engine on a contract with events (`spec/events.md`): it compiles, formats and
//! exports it, a client ignores the events and a server says `E0801`.

use crate::api::{ProjectIn, SourceIn, TargetIn, compile, export_openapi, format, generate};

const EVENTS: &str = r#"model Pong { ok: bool }

event Paid { id: uuid }

channel Payments {
  carries Paid
  key id
}

@auth(none)
service Bookings at "/bookings" {
  publishes Payments
  get "/ping" ping() -> Pong
}

service Mailer {
  subscribes Payments as "mailer" {
    on Paid -> sendReceipt
  }
}

example "a paid event reaches the mailer" {
  given event Paid { id: "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21" }
  expect Mailer.sendReceipt called
}
"#;

fn files() -> Vec<SourceIn> {
    vec![SourceIn {
        path: "hello/hello.veld".into(),
        text: EVENTS.into(),
    }]
}

fn target(emitter: &str, role: &str, framework: Option<&str>) -> TargetIn {
    TargetIn {
        emitter: emitter.into(),
        role: role.into(),
        framework: framework.map(Into::into),
        ..TargetIn::default()
    }
}

#[test]
fn a_contract_with_events_compiles() {
    let r = compile(&files(), &ProjectIn::default());
    assert!(r.ok, "{:?}", r.diagnostics);
    assert_eq!(r.summary.map(|s| (s.services, s.endpoints)), Some((2, 1)));
}

#[test]
fn a_server_refuses_events_and_a_client_ignores_them() {
    let p = ProjectIn::default();
    for (emitter, framework) in [
        ("node", Some("express")),
        ("java", Some("jdk")),
        ("rust", Some("axum")),
    ] {
        let r = generate(&files(), &target(emitter, "server", framework), &p);
        assert!(!r.ok && r.files.is_empty(), "{emitter}");
        assert!(
            r.diagnostics
                .iter()
                .any(|d| d.code == "E0801" && d.message.contains("events")),
            "{emitter}: {:?}",
            r.diagnostics
        );
    }
    for (emitter, framework) in [
        ("typescript", Some("react")),
        ("java", None),
        ("rust", None),
    ] {
        let r = generate(&files(), &target(emitter, "client", framework), &p);
        assert!(
            r.ok && !r.files.is_empty(),
            "{emitter}: {:?}",
            r.diagnostics
        );
    }
}

#[test]
fn a_contract_with_events_formats_and_exports() {
    let f = format(EVENTS);
    assert!(f.ok, "{:?}", f.diagnostics);
    assert_eq!(format(&f.text).text, f.text);
    let o = export_openapi(&files(), &ProjectIn::default());
    assert!(
        o.ok && o.text.contains("\"openapi\""),
        "{:?}",
        o.diagnostics
    );
    assert!(!o.text.contains("Paid"), "an event is no schema");
}

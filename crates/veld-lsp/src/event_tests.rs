//! The language server on a contract with events (`spec/events.md`): no diagnostics for a valid
//! contract, and the outline, hover, tokens and formatting do not trip over the new syntax.

use std::fs;

use lsp_types::SymbolKind;

use crate::workspace::{Analysis, Workspace};
use crate::{diag, hover, outline};

const TOML: &str = "[project]\nname = \"p\"\n[services.s]\ncontracts = \"c\"\n";

const EVENTS: &str = r#"/// Published when a booking is paid.
event Paid {
  id: uuid
  amount: decimal
  rule positive: amount > 0
}

@topic("shop.paid")
channel Payments {
  carries Paid
  key id
}

@auth(none)
service Bookings at "/bookings" {
  publishes Payments
  get "/ping" ping() -> Pong
}

model Pong { ok: bool }

service Mailer {
  subscribes Payments as "mailer" {
    on Paid -> sendReceipt
  }
}

example "a paid event reaches the mailer" {
  given event Paid { id: "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21", amount: 1.00 }
  expect Mailer.sendReceipt called
}
"#;

fn analysis(src: &str) -> (tempfile::TempDir, Analysis) {
    let tmp = tempfile::tempdir().unwrap();
    for (p, t) in [("veld.toml", TOML), ("c/m.veld", src)] {
        let path = tmp.path().join(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, t).unwrap();
    }
    let a = Workspace::new(Some(tmp.path().to_path_buf())).analyze(None);
    (tmp, a)
}

#[test]
fn a_valid_events_contract_has_no_diagnostics() {
    let (_tmp, a) = analysis(EVENTS);
    let all = diag::all(&a);
    let found: Vec<_> = all.values().flatten().map(|d| d.message.clone()).collect();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn an_event_has_an_outline_symbol_a_hover_and_tokens() {
    let (tmp, a) = analysis(EVENTS);
    let file = a.file_for(&tmp.path().join("c/m.veld")).unwrap();
    let symbols = outline::document_symbols(&a, file);
    let paid = symbols.iter().find(|s| s.name == "Paid").unwrap();
    assert_eq!(paid.kind, SymbolKind::EVENT);
    assert_eq!(paid.children.as_ref().map(Vec::len), Some(2));
    let mailer = symbols.iter().find(|s| s.name == "Mailer").unwrap();
    assert_eq!(mailer.detail, None, "a service without `at` has no prefix");
    assert!(symbols.iter().any(|s| s.name == "Payments"));
    assert!(!outline::semantic_tokens(&a, file).is_empty());

    let text = a.index.sources.text(file).unwrap();
    let at = text.find("event Paid").unwrap() + "event ".len() + 1;
    let md = hover::hover(&a, file, u32::try_from(at).unwrap())
        .unwrap()
        .markdown;
    assert!(md.contains("event Paid"), "{md}");
    assert!(md.contains("- kind: event"), "{md}");
    assert!(md.contains("sig: `"), "{md}");
    let at = text.find("service Mailer").unwrap() + "service ".len() + 1;
    let md = hover::hover(&a, file, u32::try_from(at).unwrap())
        .unwrap()
        .markdown;
    assert!(md.contains("service Mailer"), "{md}");
    assert!(!md.contains(" at "), "{md}");
}

#[test]
fn formatting_a_events_contract_does_not_fail() {
    let (tmp, a) = analysis(EVENTS);
    let file = a.file_for(&tmp.path().join("c/m.veld")).unwrap();
    assert!(outline::format_edits(&a, file).is_some());
}

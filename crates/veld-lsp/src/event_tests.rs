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

// ---- completion and hover (spec/events.md section 15) ---------------------------------------

fn complete_at(src: &str) -> Vec<String> {
    let at = src.find('^').unwrap();
    let text = src.replace('^', "");
    let (tmp, a) = analysis(&text);
    let file = a.file_for(&tmp.path().join("c/m.veld")).unwrap();
    let mut labels: Vec<String> = crate::complete::complete(&a, file, at)
        .into_iter()
        .map(|i| i.label)
        .collect();
    labels.sort();
    labels
}

const DECLS: &str = "event Paid { id: uuid }\nevent Left { id: uuid }\nevent Other { id: uuid }\nmodel M { x: int }\n";
const CHANNELS: &str =
    "channel Pays { carries Paid, Left\n key id }\nchannel Others { carries Other\n key id }\n";

#[test]
fn events_are_completed_after_carries_without_the_ones_already_listed() {
    let src = format!("{DECLS}channel C {{\n  carries ^\n}}\n");
    assert_eq!(complete_at(&src), ["Left", "Other", "Paid"]);
    let src = format!("{DECLS}channel C {{\n  carries Paid, ^\n}}\n");
    assert_eq!(complete_at(&src), ["Left", "Other"]);
    let src = format!("{DECLS}channel C {{\n  carries Pa^\n}}\n");
    assert!(complete_at(&src).contains(&"Paid".to_owned()));
}

#[test]
fn events_are_completed_after_given_event_and_channels_after_its_on() {
    let src = format!("{DECLS}{CHANNELS}example \"x\" {{\n  given event ^\n}}\n");
    assert_eq!(complete_at(&src), ["Left", "Other", "Paid"]);
    let src = format!("{DECLS}{CHANNELS}example \"x\" {{\n  given event Paid {{ }} on ^\n}}\n");
    assert_eq!(complete_at(&src), ["Others", "Pays"]);
}

#[test]
fn channels_are_completed_after_publishes_and_subscribes() {
    let src = format!("{DECLS}{CHANNELS}service S {{\n  publishes ^\n}}\n");
    assert_eq!(complete_at(&src), ["Others", "Pays"]);
    let src = format!("{DECLS}{CHANNELS}service S {{\n  publishes Pays, ^\n}}\n");
    assert_eq!(complete_at(&src), ["Others", "Pays"]);
    let src = format!("{DECLS}{CHANNELS}service S {{\n  subscribes ^\n}}\n");
    assert_eq!(complete_at(&src), ["Others", "Pays"]);
}

#[test]
fn an_on_line_offers_the_events_of_its_channel() {
    let sub = |channel: &str| {
        format!(
            "{DECLS}{CHANNELS}service S {{\n  subscribes {channel} as \"g\" {{\n    on ^\n  }}\n}}\n"
        )
    };
    assert_eq!(complete_at(&sub("Pays")), ["Left", "Paid"]);
    assert_eq!(complete_at(&sub("Others")), ["Other"]);
}

#[test]
fn event_words_do_not_leak_into_other_positions() {
    let src = format!("{DECLS}model N {{\n  on: int\n  x: ^\n}}\n");
    let found = complete_at(&src);
    assert!(!found.contains(&"Paid".to_owned()), "{found:?}");
    let src = format!("{DECLS}{CHANNELS}service S {{\n  publishes Pays ^\n}}\n");
    assert!(!complete_at(&src).contains(&"Others".to_owned()));
}

#[test]
fn hover_on_a_channel_shows_the_topic_the_key_and_the_signatures() {
    let (tmp, a) = analysis(EVENTS);
    let file = a.file_for(&tmp.path().join("c/m.veld")).unwrap();
    let text = a.index.sources.text(file).unwrap();
    // On the declaration, on the `publishes` line and on the `subscribes` line.
    for needle in [
        "channel Payments",
        "publishes Payments",
        "subscribes Payments",
    ] {
        let at = text.find(needle).unwrap() + needle.len() - 2;
        let md = hover::hover(&a, file, u32::try_from(at).unwrap())
            .unwrap_or_else(|| panic!("no hover on `{needle}`"))
            .markdown;
        assert!(md.contains("channel Payments"), "{md}");
        assert!(md.contains("- topic: `shop.paid`"), "{md}");
        assert!(md.contains("- key: `id` (uuid)"), "{md}");
        assert!(md.contains("`Paid`, sig: `veld:"), "{md}");
    }
}

#[test]
fn a_word_that_is_only_called_like_a_channel_gets_no_channel_hover() {
    let src = EVENTS.replace("ping() -> Pong", "Payments() -> Pong");
    let (tmp, a) = analysis(&src);
    let file = a.file_for(&tmp.path().join("c/m.veld")).unwrap();
    let text = a.index.sources.text(file).unwrap();
    let at = text.find("Payments() ->").unwrap() + 2;
    let md = hover::hover(&a, file, u32::try_from(at).unwrap()).map(|h| h.markdown);
    assert!(md.is_none_or(|m| !m.contains("topic")));
}

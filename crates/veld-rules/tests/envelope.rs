//! The event envelope goldens of `tests/wire/events` against the reference implementation
//! (`spec/events.md` sections 6 to 8). Every golden is decoded and encoded again, and the encoder
//! must produce each one from its inputs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value as Json;
use veld_codegen::canonical::canonical;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{ChannelId, DeclId, Ir, PackageKind, ServiceId};
use veld_rules::envelope::{ConsumeError, Inputs, decode, encode, source_of};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The package every golden contract is compiled as: it is part of `source` and `type`.
const PACKAGE: &str = "bookings";

fn compile_contract(path: &str) -> Ir {
    let text = std::fs::read_to_string(root().join(path))
        .unwrap()
        .replace("\r\n", "\n");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: PACKAGE.into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let errors: Vec<String> = out
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect();
    assert!(errors.is_empty(), "{path}: {errors:?}");
    out.ir.unwrap()
}

fn cases(file: &str) -> Vec<Json> {
    let path = root().join("tests/wire/events").join(file);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn event_id(ir: &Ir, name: &str) -> DeclId {
    ir.decls
        .iter()
        .find(|d| d.is_event() && d.name == name)
        .unwrap_or_else(|| panic!("no event {name}"))
        .id
}

fn channel_id(ir: &Ir, name: &str) -> ChannelId {
    let at = ir.channels.iter().position(|c| c.name == name).unwrap();
    ChannelId(u32::try_from(at).unwrap())
}

fn service_id(ir: &Ir, name: &str) -> ServiceId {
    ir.services.iter().find(|s| s.name == name).unwrap().id
}

fn time_ms(text: &str) -> i64 {
    veld_rules::helpers::instant::parse_datetime(text).unwrap()
}

fn str_of<'a>(case: &'a Json, key: &str) -> &'a str {
    case[key].as_str().unwrap()
}

#[test]
fn the_encoder_produces_every_golden_from_its_inputs() {
    let mut seen = 0;
    for case in cases("envelopes.json") {
        let ir = compile_contract(str_of(&case, "contract"));
        let source = source_of(&ir, service_id(&ir, str_of(&case, "service")));
        let bytes = encode(
            &ir,
            channel_id(&ir, str_of(&case, "channel")),
            event_id(&ir, str_of(&case, "event")),
            &case["value"],
            &Inputs {
                id: str_of(&case, "id"),
                source: &source,
                time_ms: time_ms(str_of(&case, "time")),
            },
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            str_of(&case, "canonical"),
            "{}",
            case["name"]
        );
        seen += 1;
    }
    assert!(seen >= 13, "{seen}");
}

#[test]
fn every_golden_decodes_and_encodes_again_to_the_same_bytes() {
    for case in cases("envelopes.json") {
        let ir = compile_contract(str_of(&case, "contract"));
        let channel = channel_id(&ir, str_of(&case, "channel"));
        let text = str_of(&case, "canonical");
        // The golden is canonical JSON: it is its own canonical form.
        let parsed: Json = serde_json::from_str(text).unwrap();
        assert_eq!(canonical(&parsed), text, "{}", case["name"]);
        let got = decode(&ir, &ir.channel(channel).events, text.as_bytes()).unwrap();
        assert!(got.rules_checked && !got.type_differs, "{}", case["name"]);
        let again = encode(
            &ir,
            channel,
            got.event,
            &got.data,
            &Inputs {
                id: &got.id,
                source: &got.source,
                time_ms: time_ms(got.time.as_deref().unwrap()),
            },
        )
        .unwrap();
        assert_eq!(String::from_utf8(again).unwrap(), text, "{}", case["name"]);
    }
}

fn outcome(ir: &Ir, case: &Json) -> String {
    let handled: Vec<DeclId> = case["handled"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| event_id(ir, n.as_str().unwrap()))
        .collect();
    match decode(ir, &handled, str_of(case, "message").as_bytes()) {
        Ok(r) if !r.rules_checked => "delivered, rules not checked".into(),
        Ok(r) if r.type_differs => "delivered, other type hash".into(),
        Ok(_) => "delivered".into(),
        Err(ConsumeError::UnknownType(_)) => "ignored".into(),
        Err(e) => e.rejection().unwrap().into(),
    }
}

#[test]
fn consuming_gives_the_outcome_of_every_rejection_golden() {
    let mut ir_of: BTreeMap<String, Ir> = BTreeMap::new();
    let all = cases("consume.json");
    assert!(all.len() >= 12, "{}", all.len());
    for case in all {
        let path = str_of(&case, "contract").to_owned();
        let ir = ir_of
            .entry(path.clone())
            .or_insert_with(|| compile_contract(&path));
        assert_eq!(
            outcome(ir, &case),
            str_of(&case, "outcome"),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn a_value_that_must_not_be_sent_is_not_encoded() {
    let ir = compile_contract("tests/events/bookings.veld");
    let go = |value: Json, id: &str| {
        encode(
            &ir,
            channel_id(&ir, "BookingEvents"),
            event_id(&ir, "BookingPaid"),
            &value,
            &Inputs {
                id,
                source: "veld:bookings/Bookings",
                time_ms: 0,
            },
        )
    };
    let base = serde_json::json!({ "bookingId": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21",
        "amount": "0.00", "paidAt": "2026-06-10T11:59:58Z" });
    assert!(matches!(go(base.clone(), "i"), Err(e) if e.to_string().contains("invariant")));
    let mut ok = base.clone();
    ok["amount"] = "1.00".into();
    assert!(go(ok.clone(), "i").is_ok());
    assert!(go(ok, "").is_err());
    let mut no_id = base;
    no_id.as_object_mut().unwrap().remove("bookingId");
    assert!(go(no_id, "i").is_err());
}

#[test]
fn the_golden_files_are_where_the_readme_says() {
    let dir: &Path = &root().join("tests/wire/events");
    for f in ["README.md", "envelopes.json", "consume.json", "edges.veld"] {
        assert!(dir.join(f).exists(), "{f}");
    }
}

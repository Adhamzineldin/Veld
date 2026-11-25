use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{ChannelId, DeclId, Ir, PackageKind, ServiceId};

use super::*;

const SRC: &str = "event Paid { id: string\n amount: int = 1\n rule pos: amount > 0 }\n\
    event Plain { id: string }\n\
    channel Pays { carries Paid, Plain\n key id }\n\
    service S { publishes Pays }\n";

fn ir() -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "p".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "p.veld".into(),
                text: SRC.into(),
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    out.ir.unwrap()
}

fn event(ir: &Ir, name: &str) -> DeclId {
    ir.decls.iter().find(|d| d.name == name).unwrap().id
}

fn send(ir: &Ir, name: &str, value: serde_json::Value) -> Result<Vec<u8>, PublishError> {
    encode(
        ir,
        ChannelId(0),
        event(ir, name),
        &value,
        &Inputs {
            id: "i1",
            source: &source_of(ir, ServiceId(0)),
            time_ms: 1_000,
        },
    )
}

#[test]
fn the_type_name_drops_the_hash() {
    assert_eq!(type_name("veld:p/Paid@0123"), "veld:p/Paid");
    assert_eq!(type_name("veld:p/Paid"), "veld:p/Paid");
}

#[test]
fn the_source_is_the_package_and_the_service() {
    assert_eq!(source_of(&ir(), ServiceId(0)), "veld:p/S");
}

#[test]
fn a_message_has_the_attributes_of_the_spec_in_canonical_order() {
    let ir = ir();
    let bytes = send(&ir, "Paid", json!({ "id": "k" })).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with(r#"{"data":{"amount":1,"id":"k"},"datacontenttype":"application/json","id":"i1","source":"veld:p/S","specversion":"1.0","subject":"k","time":"1970-01-01T00:00:01Z","type":"veld:p/Paid@"#), "{text}");
    assert!(text.contains(r#""veldrules":""#), "{text}");
    let plain = send(&ir, "Plain", json!({ "id": "k" })).unwrap();
    assert!(!String::from_utf8(plain).unwrap().contains("veldrules"));
}

#[test]
fn nothing_is_sent_for_an_invalid_event() {
    let ir = ir();
    assert!(matches!(
        send(&ir, "Paid", json!({ "id": "k", "amount": 0 })),
        Err(PublishError::Invalid(v)) if v.len() == 1
    ));
    assert!(matches!(
        send(&ir, "Paid", json!({ "amount": 3 })),
        Err(PublishError::Shape(_))
    ));
    assert!(matches!(
        send(&ir, "Paid", json!({ "id": "" })),
        Err(PublishError::EmptyKey(_))
    ));
}

fn receive(ir: &Ir, text: &str) -> Result<Received, ConsumeError> {
    decode(ir, &[event(ir, "Paid")], text.as_bytes())
}

#[test]
fn a_consumer_gets_what_a_publisher_sent() {
    let ir = ir();
    let bytes = send(&ir, "Paid", json!({ "id": "k", "amount": 5 })).unwrap();
    let got = decode(&ir, &[event(&ir, "Paid")], &bytes).unwrap();
    assert_eq!(got.id, "i1");
    assert_eq!(got.subject.as_deref(), Some("k"));
    assert_eq!(got.data, json!({ "id": "k", "amount": 5 }));
    assert!(got.rules_checked && !got.type_differs && !got.rules_differ());
}

#[test]
fn the_failures_of_section_8_are_typed() {
    let ir = ir();
    for bad in ["", "[]", "{}", r#"{"specversion":"1.0"}"#] {
        assert!(
            matches!(receive(&ir, bad), Err(ConsumeError::Malformed(_))),
            "{bad}"
        );
    }
    let ty = ir.decl(event(&ir, "Paid")).sig.clone();
    let msg = |extra: &str, data: &str| {
        format!(
            r#"{{"specversion":"1.0","id":"a","source":"s","type":"{ty}"{extra},"data":{data}}}"#
        )
    };
    assert!(matches!(
        receive(&ir, &msg(r#","datacontenttype":"text/plain""#, "{}")),
        Err(ConsumeError::Malformed(_))
    ));
    assert!(matches!(
        receive(&ir, &msg("", r#"{"amount":2}"#)),
        Err(ConsumeError::Shape(_))
    ));
    let digest = match &ir.decl(event(&ir, "Paid")).kind {
        veld_ir::DeclKind::Event(e) => e.rules_digest.clone().unwrap(),
        _ => unreachable!(),
    };
    let own = format!(r#","veldrules":"{digest}""#);
    let broken = r#"{"id":"k","amount":0}"#;
    assert!(matches!(
        receive(&ir, &msg(&own, broken)),
        Err(ConsumeError::Rule(_))
    ));
    assert!(receive(&ir, &msg(r#","veldrules":"0000000000000000""#, broken)).is_ok());
    let other = msg("", "{}").replace("Paid@", "Nope@");
    let err = receive(&ir, &other).unwrap_err();
    assert!(matches!(err, ConsumeError::UnknownType(_)) && err.rejection().is_none());
}

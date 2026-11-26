//! `@version`, `@idempotent` and the example lines `ifMatch` and `key` in the IR
//! (`spec/rules.md` sections 11.7 to 11.9).

use super::*;

fn a_field() -> Field {
    Field {
        name: "revision".into(),
        wire: "rev".into(),
        ty: Type::Scalar(Scalar::Long),
        optional: false,
        default: None,
        readonly: true,
        version: false,
        derived: None,
        derived_id: None,
        inherited_from: None,
        constraints: Constraints::default(),
        example: None,
        docs: None,
        deprecated: None,
        origin: origin(1),
    }
}

#[test]
fn nothing_is_written_for_an_endpoint_without_the_annotations() {
    let plain = endpoint_with(EndpointRules::default());
    let v = serde_json::to_value(&plain).unwrap();
    assert!(v.get("idempotency").is_none());
    assert!(v.get("rules").is_none());
}

#[test]
fn the_version_guard_is_in_rules_and_idempotency_beside_them() {
    let guarded = endpoint_with(sample_rules());
    let v = serde_json::to_value(&guarded).unwrap();
    assert_eq!(v["rules"]["version"]["fact"], "room");
    assert_eq!(v["rules"]["version"]["wire"], "rev");
    let mut idem = endpoint_with(EndpointRules::default());
    idem.idempotency = Some(Idempotency {
        header: "Idempotency-Key".into(),
        required: false,
        caller: true,
        origin: origin(3),
    });
    let v = serde_json::to_value(&idem).unwrap();
    assert_eq!(v["idempotency"]["header"], "Idempotency-Key");
    assert_eq!(v["idempotency"]["caller"], true);
    // An idempotent endpoint with no block has no `rules` key.
    assert!(v.get("rules").is_none());
    let back: Endpoint = serde_json::from_value(v).unwrap();
    assert_eq!(back, idem);
}

#[test]
fn a_guard_is_not_a_statement() {
    let guard_only = EndpointRules {
        version: sample_rules().version,
        ..EndpointRules::default()
    };
    assert!(!guard_only.is_empty());
    assert!(!guard_only.has_statements());
    assert!(sample_rules().has_statements());
}

#[test]
fn a_field_names_its_version_only_when_it_is_one() {
    let mut f = a_field();
    let v = serde_json::to_value(&f).unwrap();
    assert!(v.get("version").is_none());
    f.version = true;
    let v = serde_json::to_value(&f).unwrap();
    assert_eq!(v["version"], true);
    let back: Field = serde_json::from_value(v).unwrap();
    assert!(back.version);
    // A document written before the key existed reads back as `false`.
    let mut old = serde_json::to_value(a_field()).unwrap();
    old.as_object_mut().unwrap().remove("version");
    let back: Field = serde_json::from_value(old).unwrap();
    assert!(!back.version);
}

#[test]
fn example_ifmatch_and_key_have_the_documented_json() {
    let any = serde_json::to_string(&ExampleIfMatch::Any).unwrap();
    assert_eq!(any, r#""any""#);
    let version = serde_json::to_string(&ExampleIfMatch::Version("3".into())).unwrap();
    assert_eq!(version, r#"{"version":"3"}"#);
    let key = ExampleKey {
        value: "k".into(),
        store: KeyStore::New,
    };
    assert_eq!(serde_json::to_string(&key).unwrap(), r#"{"value":"k"}"#);
    let other = ExampleKey {
        value: "k".into(),
        store: KeyStore::Other,
    };
    assert_eq!(
        serde_json::to_string(&other).unwrap(),
        r#"{"value":"k","store":"other"}"#
    );
    let stored = ExampleKey {
        value: "k".into(),
        store: KeyStore::Stored(ExampleOutcome::Ok(None)),
    };
    let text = serde_json::to_string(&stored).unwrap();
    assert_eq!(text, r#"{"value":"k","store":{"stored":{"ok":null}}}"#);
    let back: ExampleKey = serde_json::from_str(&text).unwrap();
    assert_eq!(back, stored);
}

#[test]
fn the_new_features_are_opt_in_and_named() {
    assert_eq!(Feature::RulesVersion.name(), "rules_version");
    assert_eq!(Feature::Idempotency.name(), "idempotency");
    for f in [Feature::RulesVersion, Feature::Idempotency] {
        assert!(Feature::OPT_IN.contains(&f));
        assert!(Feature::ALL.contains(&f));
        assert_eq!(Feature::from_name(f.name()), Some(f));
    }
}

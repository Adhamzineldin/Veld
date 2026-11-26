//! `@pii` and `@secret` (`spec/rules.md` section 19.3): where they may stand, what they lower to,
//! and what a contract without them keeps.

use crate::testing::{codes, ir as build, one};

const GUEST: &str = "model Guest {\n  id: uuid\n  name: string @pii\n  phone?: string @pii\n  password: string @secret\n  both: string @pii @secret\n  plain: string\n}\n";

fn flags(src: &str, decl: &str) -> Vec<(String, bool, bool)> {
    let ir = build(src);
    ir.decls
        .iter()
        .find(|d| d.name == decl)
        .map(|d| {
            d.fields()
                .iter()
                .map(|f| (f.name.clone(), f.pii, f.secret))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn both_annotations_lower_to_the_field() {
    let got = flags(GUEST, "Guest");
    let want = [
        ("id", false, false),
        ("name", true, false),
        ("phone", true, false),
        ("password", false, true),
        ("both", true, true),
        ("plain", false, false),
    ];
    let want: Vec<(String, bool, bool)> = want
        .iter()
        .map(|(n, p, s)| ((*n).to_string(), *p, *s))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn the_json_has_the_keys_only_where_they_are_true() {
    let ir = build(GUEST);
    let json = serde_json::to_value(&ir).unwrap();
    let fields = json["decls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "Guest")
        .unwrap()["fields"]
        .as_array()
        .unwrap()
        .clone();
    let seen = |name: &str, key: &str| {
        fields
            .iter()
            .find(|f| f["name"] == name)
            .is_some_and(|f| f.get(key) == Some(&serde_json::Value::Bool(true)))
    };
    assert!(seen("name", "pii") && !seen("name", "secret"));
    assert!(seen("password", "secret") && !seen("password", "pii"));
    assert!(seen("both", "pii") && seen("both", "secret"));
    let plain = fields.iter().find(|f| f["name"] == "plain").unwrap();
    assert!(plain.get("pii").is_none() && plain.get("secret").is_none());
}

#[test]
fn a_contract_without_them_has_neither_key() {
    let ir = build("model A { x: int }\n");
    let text = serde_json::to_string(&ir).unwrap();
    assert!(!text.contains("\"pii\"") && !text.contains("\"secret\""));
}

#[test]
fn they_stand_on_a_field_of_an_error_and_of_an_event() {
    let src = "error Leak 400 {\n token: string @secret\n who: string @pii\n}\n\
               event Registered { email: string @pii }\n\
               channel Signups { carries Registered }\n";
    assert_eq!(codes(src), Vec::<&str>::new());
    let got = flags(src, "Leak");
    assert!(got.contains(&("token".to_string(), false, true)));
    assert!(got.contains(&("who".to_string(), true, false)));
}

#[test]
fn an_inherited_field_keeps_its_flags() {
    let src = format!("{GUEST}model Vip extends Guest {{ tier: int }}\n");
    let got = flags(&src, "Vip");
    assert!(got.contains(&("name".to_string(), true, false)));
    assert!(got.contains(&("password".to_string(), false, true)));
}

#[test]
fn they_change_no_signature() {
    let with = build("model A { x: string @pii\n y: string @secret }\n");
    let without = build("model A { x: string\n y: string }\n");
    assert_eq!(with.decls[1].sig, without.decls[1].sig);
}

#[test]
fn elsewhere_or_with_an_argument_is_e0602() {
    assert_eq!(codes("@pii\nmodel A { x: int }\n"), ["E0602"]);
    assert_eq!(codes("@secret\nmodel A { x: int }\n"), ["E0602"]);
    assert_eq!(
        codes("model A { x: int }\nservice S at \"/s\" {\n  @pii\n  get \"/\" f() -> A\n}\n"),
        ["E0602"]
    );
    assert_eq!(codes("model A { x: string @pii(1) }\n"), ["E0602"]);
    assert_eq!(codes("model A { x: string @secret(\"a\") }\n"), ["E0602"]);
    assert_eq!(codes("model A { x: string @pii @pii }\n"), ["E0602"]);
}

#[test]
fn the_words_stay_ordinary_names() {
    let src = "model A { pii: string\n secret: string\n retain: string }\n";
    assert_eq!(codes(src), Vec::<&str>::new());
    assert!(one(src).ir.is_some());
}

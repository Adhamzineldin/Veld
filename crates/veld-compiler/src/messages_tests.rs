//! Message catalogs (`spec/platform.md` section 22): reading, `E1301`, `W1301`, `W1302`, and what
//! the IR keeps.

use std::collections::BTreeMap;

use crate::messages::{
    CatalogFile, Catalogs, MemberCatalogs, compile_with_catalogs, is_language_tag, parse_catalog,
};
use crate::{CompileInput, CompileOutput, PackageInput, SourceFile, compile_prefixed};
use veld_ir::{Feature, PackageKind};

const SRC: &str = r#"
model Booking {
  id: uuid
  start: datetime
  end: datetime
  rule endAfterStart: end > start
    message "A booking must end after it starts"
  rule noMessage: id != id
}

model Payment { amount: decimal }

@auth(none)
service Bookings at "/bookings" {
  post "/{id}/pay" pay(id: uuid, body: Payment) -> Booking {
    fact booking: Booking = id
    require notZero: body.amount > 0
      message "The amount must be positive"
  }
}
"#;

fn input() -> CompileInput {
    CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "app.veld".into(),
                text: SRC.into(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    }
}

fn catalogs(files: &[(&str, &str)]) -> Catalogs {
    let files = files
        .iter()
        .map(|(name, text)| CatalogFile {
            name: (*name).into(),
            text: (*text).into(),
        })
        .collect();
    let member = MemberCatalogs {
        dir: "messages".into(),
        files,
    };
    [("bookings".to_string(), member)].into()
}

fn run(files: &[(&str, &str)]) -> CompileOutput {
    compile_with_catalogs(&input(), None, &BTreeMap::new(), &catalogs(files))
}

fn codes(out: &CompileOutput) -> Vec<&'static str> {
    out.diagnostics.iter().map(|d| d.code).collect()
}

const AR: &str = "\"bookings/Booking.endAfterStart\" = \"يجب أن تنتهي الإقامة بعد بدايتها\"\n\
\"bookings/Bookings.pay.notZero\" = \"يجب أن يكون المبلغ موجبا\"\n\
\"veld/not-permitted\" = \"غير مسموح\"\n";

#[test]
fn language_tags_are_lower_case_subtags_of_a_primary_language() {
    for ok in ["ar", "fr", "pt-br", "zh-hant-tw", "ar-eg", "haw"] {
        assert!(is_language_tag(ok), "{ok}");
    }
    for bad in [
        "",
        "a",
        "AR",
        "pt-BR",
        "pt_br",
        "arabic",
        "ar-",
        "-ar",
        "ar--eg",
        "ar-abcdefghi",
        "ar eg",
    ] {
        assert!(!is_language_tag(bad), "{bad:?}");
    }
}

#[test]
fn a_catalog_is_a_flat_table_of_strings() {
    let ok = parse_catalog("\"a/B.c\" = \"x\"\n# comment\n'a/B.d' = '''y'''\n").unwrap();
    assert_eq!(ok.get("a/B.c").map(String::as_str), Some("x"));
    assert_eq!(ok.get("a/B.d").map(String::as_str), Some("y"));
    assert!(parse_catalog("").unwrap().is_empty());
    for bad in [
        "\"a/B.c\" = 3\n",
        "\"a/B.c\" = [\"x\"]\n",
        "[table]\n\"a/B.c\" = \"x\"\n",
        "\"a/B.c\" = \"x\"\n\"a/B.c\" = \"y\"\n",
        "a/B.c = \"x\"\n",
        "not toml at all",
    ] {
        assert!(parse_catalog(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn no_catalog_is_the_same_output_as_before() {
    let plain = compile_prefixed(&input(), None, &BTreeMap::new());
    let with = compile_with_catalogs(&input(), None, &BTreeMap::new(), &Catalogs::new());
    assert_eq!(
        serde_json::to_string(&plain.ir).unwrap(),
        serde_json::to_string(&with.ir).unwrap()
    );
    let text = serde_json::to_string(&with.ir).unwrap();
    assert!(!text.contains("messages"), "{text}");
}

#[test]
fn the_ir_keeps_the_catalogs_sorted_with_their_directory() {
    let out = run(&[
        ("pt-br.toml", "\"veld/version\" = \"versao\"\n"),
        ("ar.toml", AR),
    ]);
    let ir = out.ir.unwrap();
    let p = &ir.packages.iter().find(|p| p.name == "bookings").unwrap();
    let tags: Vec<&str> = p.messages.keys().map(String::as_str).collect();
    assert_eq!(tags, ["ar", "pt-br"]);
    assert_eq!(p.messages_dir.as_deref(), Some("messages"));
    assert_eq!(p.messages["ar"].len(), 3);
    let json = serde_json::to_value(&ir).unwrap();
    assert!(json["packages"][0]["messages"]["ar"]["veld/not-permitted"].is_string());
    assert_eq!(
        p.first_catalog_origin().map(|o| o.file),
        Some("messages/ar.toml".to_string())
    );
}

#[test]
fn a_member_with_catalogs_uses_the_messages_feature() {
    let ir = run(&[("ar.toml", AR)]).ir.unwrap();
    let all: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    assert!(ir.features_used(&all).contains(&Feature::Messages));
    let bare = compile_prefixed(&input(), None, &BTreeMap::new())
        .ir
        .unwrap();
    assert!(!bare.features_used(&all).contains(&Feature::Messages));
}

#[test]
fn a_complete_catalog_has_no_warning() {
    let full = format!("{AR}\"bookings/Booking.noMessage\" = \"x\"\n");
    // `noMessage` has no message in the contract, so it needs no translation, and a key for it is
    // still a key of the member.
    assert!(codes(&run(&[("ar.toml", &full)])).is_empty());
    assert!(codes(&run(&[("ar.toml", AR)])).is_empty());
}

#[test]
fn a_name_that_is_not_a_language_tag_is_e1301() {
    for name in ["AR.toml", "pt_br.toml", "arabic.toml", "pt-BR.toml"] {
        let out = run(&[(name, AR)]);
        assert_eq!(codes(&out), ["E1301"], "{name}");
        assert!(out.ir.is_none(), "{name}");
    }
}

#[test]
fn a_file_that_is_not_toml_is_not_a_catalog() {
    let out = run(&[("README.md", "hello"), ("ar.toml", AR)]);
    assert!(codes(&out).is_empty());
    assert_eq!(out.ir.unwrap().packages[0].messages.len(), 1);
}

#[test]
fn a_value_that_is_not_a_string_is_e1301_at_its_key() {
    let out = run(&[("ar.toml", "\"bookings/Booking.endAfterStart\" = 3\n")]);
    assert_eq!(codes(&out), ["E1301"]);
    let text = out.render();
    assert!(text.contains("ar.toml"), "{text}");
    assert!(out.ir.is_none());
    assert_eq!(
        codes(&run(&[("ar.toml", "[section]\nk = \"v\"\n")])),
        ["E1301"]
    );
    assert_eq!(codes(&run(&[("ar.toml", "x = = 1")])), ["E1301"]);
}

#[test]
fn a_key_no_rule_has_is_w1301_and_suggests_the_nearest_key() {
    let typo = "\"bookings/Booking.endAfterStrat\" = \"x\"\n";
    let out = run(&[("ar.toml", &format!("{AR}{typo}"))]);
    assert_eq!(codes(&out), ["W1301"]);
    let d = &out.diagnostics[0];
    assert!(d.message.contains("endAfterStrat"), "{}", d.message);
    assert_eq!(
        d.help.as_deref(),
        Some("did you mean `bookings/Booking.endAfterStart`?")
    );
    assert!(out.ir.is_some());
}

#[test]
fn the_fixed_keys_are_valid_keys() {
    let all = "\"veld/transition\" = \"a\"\n\"veld/transition-not-due\" = \"b\"\n\
               \"veld/version\" = \"c\"\n\"veld/not-permitted\" = \"d\"\n";
    let out = run(&[("ar.toml", &format!("{AR}{all}"))]);
    assert!(codes(&out).is_empty(), "{}", out.render());
}

#[test]
fn a_catalog_that_lacks_messages_is_w1302_once_with_the_count_and_the_first_keys() {
    let out = run(&[("ar.toml", "\"veld/not-permitted\" = \"x\"\n")]);
    assert_eq!(codes(&out), ["W1302"]);
    let m = &out.diagnostics[0].message;
    assert!(m.contains("`ar`") && m.contains("lacks 2"), "{m}");
    assert!(m.contains("bookings/Booking.endAfterStart"), "{m}");
    assert!(m.contains("bookings/Bookings.pay.notZero"), "{m}");
    let two = run(&[
        ("ar.toml", "\"veld/not-permitted\" = \"x\"\n"),
        ("fr.toml", AR),
    ]);
    assert_eq!(codes(&two), ["W1302"]);
}

#[test]
fn more_than_three_missing_keys_are_counted_not_listed() {
    let mut src = String::from("model M {\n  id: uuid\n");
    for n in 0..5 {
        src.push_str(&format!("  rule r{n}: id == id\n    message \"m{n}\"\n"));
    }
    src.push_str("}\n@auth(none)\nservice S at \"/s\" { get \"/\" f() -> M }\n");
    let mut input = input();
    input.packages[0].files[0].text = src;
    let out = compile_with_catalogs(
        &input,
        None,
        &BTreeMap::new(),
        &catalogs(&[("ar.toml", "\"veld/version\" = \"x\"\n")]),
    );
    let m = &out.diagnostics[0].message;
    assert!(m.contains("lacks 5") && m.contains("and 2 more"), "{m}");
}

#[test]
fn a_catalog_of_a_package_that_is_not_a_service_is_ignored() {
    let mut c = catalogs(&[("ar.toml", AR)]);
    let member = c.remove("bookings").unwrap();
    c.insert("nobody".into(), member);
    let out = compile_with_catalogs(&input(), None, &BTreeMap::new(), &c);
    assert!(codes(&out).is_empty());
    assert!(out.ir.unwrap().packages[0].messages.is_empty());
}

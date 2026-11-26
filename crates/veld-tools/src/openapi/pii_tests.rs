//! `x-veld-pii` and `x-veld-secret` (`spec/rules.md` section 19.3).

use serde_json::{Value, json};
use veld_ir::ServiceId;

use super::openapi;
use crate::testutil::ir;

fn doc(src: &str) -> Value {
    let ir = ir(src);
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    openapi(&ir, &ids)
}

const SRC: &str = r#"
model Address { street: string @pii }
model Guest {
  name: string @pii
  password: string @secret
  both: string @pii @secret
  home: Address @pii
  plain: string
}
@auth(bearer)
service Guests at "/guests" {
  get "/" list() -> Guest
}
"#;

#[test]
fn the_extensions_mark_exactly_the_annotated_properties() {
    let d = doc(SRC);
    let p = &d["components"]["schemas"]["app.Guest"]["properties"];
    assert_eq!(p["name"]["x-veld-pii"], json!(true));
    assert!(p["name"].get("x-veld-secret").is_none());
    assert_eq!(p["password"]["x-veld-secret"], json!(true));
    assert!(p["password"].get("x-veld-pii").is_none());
    assert_eq!(p["both"]["x-veld-pii"], json!(true));
    assert_eq!(p["both"]["x-veld-secret"], json!(true));
    assert_eq!(p["home"]["x-veld-pii"], json!(true));
    assert!(p["plain"].get("x-veld-pii").is_none() && p["plain"].get("x-veld-secret").is_none());
}

#[test]
fn a_contract_without_them_exports_no_extension() {
    let text =
        doc("model A { x: string }\nservice S at \"/s\" { get \"/\" f() -> A }\n").to_string();
    assert!(!text.contains("x-veld-pii") && !text.contains("x-veld-secret"));
}

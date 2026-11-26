//! The OpenAPI export and platform phase P1 (`spec/platform.md` sections 2 to 5): the extensions
//! stand only where the feature is, and a contract without them exports the same bytes.

use serde_json::{Value, json};
use veld_ir::ServiceId;

use super::openapi;
use crate::testutil::ir;

fn doc(src: &str) -> Value {
    let ir = ir(src);
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    openapi(&ir, &ids)
}

const VISIBLE: &str = "model User { id: uuid\n roles: string[] }\nprincipal User\n\
model Booking { guestId: uuid\n phone?: string @visible(when: user.id == guestId)\n note?: string }\n\
@auth(bearer)\nservice S at \"/s\" { get \"/\" f() -> Booking }\n";

#[test]
fn a_visible_field_carries_x_veld_visible_and_no_other_property_does() {
    let d = doc(VISIBLE);
    let props = &d["components"]["schemas"]["app.Booking"]["properties"];
    assert_eq!(props["phone"]["x-veld-visible"], json!(true));
    assert!(props["note"].get("x-veld-visible").is_none(), "{props}");
    assert!(props["guestId"].get("x-veld-visible").is_none(), "{props}");
}

#[test]
fn the_field_stays_optional_for_a_client() {
    let d = doc(VISIBLE);
    let required = d["components"]["schemas"]["app.Booking"]["required"].to_string();
    assert!(!required.contains("phone"), "{required}");
}

#[test]
fn a_contract_without_visibility_exports_no_extension() {
    let plain = VISIBLE.replace(" @visible(when: user.id == guestId)", "");
    assert!(!doc(&plain).to_string().contains("x-veld-visible"));
}

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

const TENANT: &str = "model User { id: uuid\n tenantId?: uuid @tenant }\nprincipal User\n\
model Booking { id: uuid\n tenantId: uuid @tenant\n title: string }\n\
@auth(bearer)\nservice S at \"/s\" { get \"/\" f() -> Booking[] }\n";

#[test]
fn a_tenant_field_is_read_only_and_carries_x_veld_tenant() {
    let d = doc(TENANT);
    let props = &d["components"]["schemas"]["app.Booking"]["properties"];
    assert_eq!(props["tenantId"]["x-veld-tenant"], json!(true));
    assert_eq!(props["tenantId"]["readOnly"], json!(true));
    assert!(props["title"].get("x-veld-tenant").is_none(), "{props}");
    assert!(props["title"].get("readOnly").is_none(), "{props}");
}

#[test]
fn a_contract_without_a_tenant_exports_no_extension() {
    let plain = TENANT.replace(" @tenant", "");
    assert!(!doc(&plain).to_string().contains("x-veld-tenant"));
}

const LIMIT: &str = "model User { id: uuid\n tenantId?: uuid @tenant }\nprincipal User\nmodel Room { id: uuid }\n\
@auth(bearer)\nservice S at \"/s\" {\n  @limit(10, per: minute, by: user)\n  get \"/\" f() -> Room[]\n  get \"/all\" g() -> Room[]\n}\n";

#[test]
fn a_limit_carries_x_veld_limit_and_a_429_with_retry_after() {
    let d = doc(LIMIT);
    let op = &d["paths"]["/s"]["get"];
    assert_eq!(
        op["x-veld-limit"],
        json!({ "count": 10, "per": "minute", "by": "user" })
    );
    let r429 = &op["responses"]["429"];
    assert!(r429.is_object(), "{op}");
    assert_eq!(
        r429["headers"]["Retry-After"]["schema"]["type"],
        json!("integer")
    );
}

#[test]
fn an_endpoint_without_a_limit_has_neither() {
    let d = doc(LIMIT);
    let other = &d["paths"]["/s/all"]["get"];
    assert!(other.get("x-veld-limit").is_none(), "{other}");
    assert!(other["responses"].get("429").is_none(), "{other}");
}

#[test]
fn a_contract_without_a_limit_exports_no_extension() {
    let plain = LIMIT.replace("  @limit(10, per: minute, by: user)\n", "");
    let text = doc(&plain).to_string();
    assert!(!text.contains("x-veld-limit") && !text.contains("Retry-After"));
}

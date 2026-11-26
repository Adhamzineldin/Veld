//! `@tenant` (`spec/platform.md` section 3): `E1110`, what it lowers to, and what a contract
//! without it keeps.

use crate::testing::{codes, ir as build, one};
use veld_ir::{Feature, Ir};

const USER: &str = "model User { id: uuid\n tenantId?: uuid @tenant }\nprincipal User\n";

fn booking(extra: &str) -> String {
    format!(
        "{USER}model Booking {{\n  id: uuid\n  tenantId: uuid @tenant\n{extra}}}\n\
         @auth(bearer)\nservice Bookings at \"/b\" {{ get \"/{{id}}\" one(id: uuid) -> Booking }}\n"
    )
}

fn field<'a>(ir: &'a Ir, decl: &str, name: &str) -> &'a veld_ir::Field {
    ir.decls
        .iter()
        .find(|d| d.name == decl)
        .and_then(|d| d.fields().iter().find(|f| f.name == name))
        .unwrap()
}

#[test]
fn the_field_lowers_with_its_id_and_the_feature_is_used() {
    let ir = build(&booking(""));
    let f = field(&ir, "Booking", "tenantId");
    assert!(f.tenant && !f.readonly && f.server_set());
    let id = f.tenant_id.as_deref().unwrap();
    let prefix = "veld:app/Booking.tenant@";
    assert!(
        id.starts_with(prefix) && id.len() == prefix.len() + 16,
        "{id}"
    );
    assert!(!field(&ir, "Booking", "id").tenant);
    let used = ir.features_used(&[ir.services[0].id]);
    assert!(used.contains(&Feature::Tenancy), "{used:?}");
}

#[test]
fn every_type_of_a_tenant_is_accepted() {
    for ty in ["uuid", "string", "int", "long"] {
        let src = format!(
            "model User {{ id: uuid\n tenant?: {ty} @tenant }}\nprincipal User\nmodel B {{ id: uuid\n tenant: {ty} @tenant }}\n\
             @auth(bearer)\nservice S at \"/s\" {{ get \"/\" f() -> B[] }}\n"
        );
        assert_eq!(codes(&src), Vec::<&str>::new(), "{ty}");
    }
}

#[test]
fn another_type_is_e1110() {
    for ty in ["bool", "decimal", "datetime", "float", "string[]"] {
        let src = format!("model B {{ t: {ty} @tenant }}\n");
        assert_eq!(codes(&src), vec!["E1110"], "{ty}");
    }
}

#[test]
fn two_tenants_in_one_model_or_inherited_are_e1110_once() {
    assert_eq!(
        codes("model B { a: uuid @tenant\n b: uuid @tenant }\n"),
        vec!["E1110"]
    );
    assert_eq!(
        codes("model P { a: uuid @tenant }\nmodel C extends P { b: uuid @tenant }\n"),
        vec!["E1110"]
    );
    assert_eq!(
        codes("model P { a: uuid @tenant\n b: uuid @tenant }\nmodel C extends P { n: int }\n"),
        vec!["E1110"]
    );
}

#[test]
fn an_optional_tenant_is_for_the_principal_model_alone() {
    assert_eq!(codes("model B { a?: uuid @tenant }\n"), vec!["E1110"]);
    assert_eq!(
        codes(&format!("{USER}model B {{ a?: uuid @tenant }}\n")),
        vec!["E1110"]
    );
    assert_eq!(codes(USER), Vec::<&str>::new());
}

#[test]
fn a_tenant_on_an_error_or_a_derived_field_is_e1110() {
    assert_eq!(codes("error E 409 { t: uuid @tenant }\n"), vec!["E1110"]);
    assert!(codes("model B { a: int\n t: int = a @tenant }\n").contains(&"E1110"));
}

#[test]
fn a_tenant_on_an_event_is_allowed_and_needs_no_principal() {
    let src = "event Paid { tenantId: uuid @tenant\n id: uuid }\nchannel C { carries Paid }\n";
    assert!(
        !codes(src).iter().any(|c| c.starts_with('E')),
        "{:?}",
        codes(src)
    );
}

#[test]
fn arguments_and_other_places_are_e0602() {
    assert!(codes("model B { t: uuid @tenant(1) }\n").contains(&"E0602"));
    assert!(codes("@tenant\nmodel B { t: uuid }\n").contains(&"E0602"));
}

#[test]
fn an_endpoint_that_returns_a_tenant_scoped_model_needs_a_principal_with_a_tenant() {
    let none = "model B { id: uuid\n t: uuid @tenant }\n@auth(bearer)\nservice S at \"/s\" { get \"/\" f() -> B[] }\n";
    assert_eq!(codes(none), vec!["E1110"]);
    let no_field = "model User { id: uuid }\nprincipal User\nmodel B { id: uuid\n t: uuid @tenant }\n@auth(bearer)\nservice S at \"/s\" { get \"/\" f() -> B[] }\n";
    assert_eq!(codes(no_field), vec!["E1110"]);
    let wrong_type = "model User { id: uuid\n t?: int @tenant }\nprincipal User\nmodel B { id: uuid\n t: uuid @tenant }\n@auth(bearer)\nservice S at \"/s\" { get \"/\" f() -> B[] }\n";
    assert_eq!(codes(wrong_type), vec!["E1110"]);
}

#[test]
fn an_endpoint_that_loads_one_in_a_fact_needs_them_too() {
    let src = "model B { id: uuid\n t: uuid @tenant }\n@auth(bearer)\nservice S at \"/s\" { post \"/{id}\" f(id: uuid) -> 204 { fact b: B = id } }\n";
    assert_eq!(codes(src), vec!["E1110"]);
}

#[test]
fn a_tenant_scoped_endpoint_needs_auth() {
    let src = booking("").replace("@auth(bearer)", "@auth(none)");
    assert_eq!(codes(&src), vec!["E1110"]);
}

#[test]
fn a_service_that_reaches_no_tenant_scoped_model_needs_nothing() {
    let src = "model B { id: uuid }\nservice S at \"/s\" { get \"/\" f() -> B[] }\n";
    assert_eq!(codes(src), Vec::<&str>::new());
}

#[test]
fn a_rule_that_reads_the_tenant_is_skipped_in_a_request_like_one_that_reads_a_readonly() {
    let ir =
        build("model B { t: uuid @tenant\n n: int\n rule r: n > 0 || t == t\n rule q: n > 1 }\n");
    let b = ir.decls.iter().find(|d| d.name == "B").unwrap();
    let flags: Vec<(String, bool)> = b
        .rules()
        .iter()
        .map(|r| (r.name.clone(), r.reads_readonly))
        .collect();
    assert_eq!(flags, [("r".to_string(), true), ("q".to_string(), false)]);
}

#[test]
fn it_is_part_of_no_signature_and_a_contract_without_it_keeps_its_json() {
    let with = build("model B { t: uuid @tenant\n n: int }\n");
    let without = build("model B { t: uuid\n n: int }\n");
    let sig = |ir: &Ir| ir.decls.iter().find(|d| d.name == "B").unwrap().sig.clone();
    assert_eq!(sig(&with), sig(&without));
    let text = serde_json::to_string(&without).unwrap();
    assert!(!text.contains("\"tenant\"") && !text.contains("tenant_id"));
    assert!(
        serde_json::to_string(&with)
            .unwrap()
            .contains("\"tenant\":true")
    );
}

#[test]
fn tenant_stays_an_ordinary_name() {
    assert!(one("model M { tenant: string }\n").ir.is_some());
}

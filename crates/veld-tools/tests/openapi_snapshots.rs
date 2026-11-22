//! OpenAPI snapshots for real conformance cases.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::case_ir;
use veld_ir::ServiceId;
use veld_tools::openapi;

fn export(case: &str) -> String {
    let ir = case_ir(case).unwrap_or_else(|| panic!("{case} must compile"));
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    serde_json::to_string_pretty(&openapi(&ir, &ids)).unwrap_or_default()
}

#[test]
fn spec_example() {
    insta::assert_snapshot!(export("ok_spec_example"));
}

#[test]
fn binding_kinds() {
    insta::assert_snapshot!(export("ok_binding_kinds"));
}

#[test]
fn union_and_constraints() {
    insta::assert_snapshot!(export("ok_union"));
}

#[test]
fn scalars_and_constraints() {
    insta::assert_snapshot!(export("ok_scalars_and_constraints"));
}

#[test]
fn streaming_and_auth() {
    insta::assert_snapshot!(export("ok_streaming_and_auth"));
}

#[test]
fn same_services_in_any_order_give_the_same_document() {
    let ir = case_ir("ok_spec_example").unwrap_or_else(|| panic!("must compile"));
    let mut ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    let a = openapi(&ir, &ids);
    ids.reverse();
    assert_eq!(a, openapi(&ir, &ids));
}

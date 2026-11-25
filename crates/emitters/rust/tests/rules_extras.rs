//! Rules beyond bodies in generated Rust (`spec/rules.md` sections 5.2 and 5.3): `W0904` for a
//! target with request validation off, declared errors, stream items and WebSocket messages, and
//! the golden `422` problem document.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{DynGenerator, GenRequest, Role};
use veld_emit_rust::Rust;

fn request(role: Role, ir: &veld_ir::Ir) -> GenRequest<'_> {
    GenRequest {
        ir,
        role,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: None,
    }
}

#[test]
fn w0904_is_reported_for_a_target_with_validation_off_that_reaches_rules() {
    let ir = rules_ir();
    let off = json!({ "validate": false });
    for role in [Role::Server, Role::Client] {
        let req = request(role, &ir);
        let warnings = Rust.warnings_dyn(&req, &off);
        assert_eq!(warnings.len(), 1, "{}: {warnings:?}", role.name());
        assert_eq!(warnings[0].code, "W0904");
        assert!(warnings[0].message.contains("bookings:bookings.Booking"));
        assert!(
            warnings[0].message.contains("and 2 more"),
            "{:?}",
            warnings[0]
        );
        assert!(warnings[0].origin.is_some(), "points at the first rule");
        // Validation on (the default, or asked for): nothing to say.
        assert!(Rust.warnings_dyn(&req, &json!({})).is_empty());
        assert!(
            Rust.warnings_dyn(&req, &json!({ "validate": true }))
                .is_empty()
        );
    }
}

#[test]
fn w0904_is_not_reported_when_the_contract_has_no_rules() {
    let ir = standard_ir();
    let off = json!({ "validate": false });
    let server = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![service_id(&ir, "Orders")],
        consumes: vec![service_id(&ir, "Iam")],
        framework: None,
    };
    assert!(Rust.warnings_dyn(&server, &off).is_empty());
    assert!(
        Rust.warnings_dyn(&request(Role::Client, &ir), &off)
            .is_empty()
    );
}

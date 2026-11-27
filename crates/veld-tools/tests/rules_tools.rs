//! A model with rules goes through the OpenAPI export, lint and the breaking-change diff
//! without trouble (rules are ignored by these tools for now).
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind, ServiceId};
use veld_tools::lint::{LintConfig, lint};
use veld_tools::{diff, has_breaking, openapi};

fn ir(src: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "app".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "app.veld".into(),
                text: src.into(),
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    let rendered = out.render();
    out.ir
        .unwrap_or_else(|| panic!("does not compile:\n{rendered}"))
}

const PLAIN: &str = "model Booking { start: datetime\n end: datetime }\nservice Bookings at \"/b\" {\n post \"/\" create(body: Booking) -> 201 Booking\n}\n";
const WITH_RULES: &str = "model Booking {\n start: datetime\n end: datetime\n rule endAfterStart: end > start message \"later\"\n}\nservice Bookings at \"/b\" {\n post \"/\" create(body: Booking) -> 201 Booking\n}\n";

#[test]
fn openapi_documents_rules_and_otherwise_describes_the_model_as_before() {
    let ir = ir(WITH_RULES);
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    let mut with_rules = openapi(&ir, &ids);
    let doc = serde_json::to_string(&with_rules).unwrap();
    assert!(doc.contains("endAfterStart"), "{doc}");
    // Without the `x-veld-rules` extension the document is the one of the plain contract.
    let rules = with_rules["components"]["schemas"]["app.Booking"]
        .as_object_mut()
        .and_then(|m| m.remove("x-veld-rules"));
    assert!(rules.is_some());
    let plain = ir_of_plain();
    let plain_ids: Vec<ServiceId> = plain.services.iter().map(|s| s.id).collect();
    assert_eq!(
        with_rules["components"],
        openapi(&plain, &plain_ids)["components"]
    );
}

fn ir_of_plain() -> Ir {
    ir(PLAIN)
}

#[test]
fn adding_a_rule_to_a_request_type_is_breaking_but_does_not_change_the_signature() {
    let (old, new) = (ir_of_plain(), ir(WITH_RULES));
    // `Booking` is a request body, so requests accepted before may now be rejected. The rule
    // never touches the signature, so typed error matching of deployed clients is unaffected.
    assert!(has_breaking(&diff(&old, &new)), "{:?}", diff(&old, &new));
    let sig = |ir: &Ir| {
        ir.decls
            .iter()
            .find(|d| d.name == "Booking")
            .map(|d| d.sig.clone())
    };
    assert_eq!(sig(&old), sig(&new));
}

#[test]
fn lint_accepts_a_model_with_rules() {
    let _ = lint(&ir(WITH_RULES), &LintConfig::default());
}

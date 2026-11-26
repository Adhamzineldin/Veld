//! `veld diff` and `@tenant` (`spec/platform.md` section 3).

use super::ChangeKind::{self, *};
use super::Severity::{self, PossiblyBreaking as P};
use super::diff;
use crate::testutil::ir;

fn model(on_tenant: &str, on_owner: &str) -> String {
    format!(
        "model User {{ id: uuid\n tenantId?: uuid @tenant }}\nprincipal User\n\
         model Booking {{\n  tenantId: uuid {on_tenant}\n  owner: uuid {on_owner}\n}}\n\
         @auth(bearer)\nservice S at \"/s\" {{ get \"/\" f() -> Booking[] }}\n"
    )
}

fn kinds(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| (c.kind, c.severity))
        .collect();
    v.sort();
    v
}

#[test]
fn adding_a_tenant_is_possibly_breaking() {
    assert_eq!(
        kinds(&model("", ""), &model("@tenant", "")),
        [(TenantAdded, P)]
    );
}

#[test]
fn removing_a_tenant_is_possibly_breaking() {
    assert_eq!(
        kinds(&model("@tenant", ""), &model("", "")),
        [(TenantRemoved, P)]
    );
}

#[test]
fn moving_a_tenant_is_possibly_breaking() {
    assert_eq!(
        kinds(&model("@tenant", ""), &model("", "@tenant")),
        [(TenantChanged, P)]
    );
}

#[test]
fn a_contract_that_does_not_move_reports_nothing() {
    assert!(kinds(&model("@tenant", ""), &model("@tenant", "")).is_empty());
    assert!(kinds(&model("", ""), &model("", "")).is_empty());
}

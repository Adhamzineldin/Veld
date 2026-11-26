//! `veld diff` and `@audited` (`spec/platform.md` section 4).

use super::ChangeKind::{self, *};
use super::Severity::{self, NonBreaking as N, PossiblyBreaking as P};
use super::diff;
use crate::testutil::ir;

fn contract(on: &str) -> String {
    format!(
        "model Room {{ id: uuid }}\nservice S at \"/s\" {{\n  {on}\n  get \"/\" f() -> Room[]\n}}\n"
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
fn an_endpoint_that_becomes_audited_is_compatible() {
    assert_eq!(
        kinds(&contract(""), &contract("@audited")),
        [(AuditAdded, N)]
    );
}

#[test]
fn an_endpoint_that_stops_being_audited_is_possibly_breaking() {
    assert_eq!(
        kinds(&contract("@audited"), &contract("")),
        [(AuditRemoved, P)]
    );
}

#[test]
fn a_contract_that_does_not_move_reports_nothing() {
    assert!(kinds(&contract("@audited"), &contract("@audited")).is_empty());
    assert!(kinds(&contract(""), &contract("")).is_empty());
}

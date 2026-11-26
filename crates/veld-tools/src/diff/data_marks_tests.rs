//! `veld diff` and `@pii`, `@secret` (`spec/rules.md` section 19.3): every change is
//! non-breaking, and a contract that does not move reports nothing.

use super::ChangeKind::{self, *};
use super::Severity::{self, NonBreaking as N};
use super::{diff, has_breaking};
use crate::testutil::ir;

fn model(marks: &str) -> String {
    format!(
        "model Guest {{\n  id: uuid\n  name: string {marks}\n}}\nservice S at \"/s\" {{ get \"/\" f() -> Guest }}\n"
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
fn adding_or_removing_pii_is_non_breaking() {
    assert_eq!(kinds(&model(""), &model("@pii")), [(PiiAdded, N)]);
    assert_eq!(kinds(&model("@pii"), &model("")), [(PiiRemoved, N)]);
}

#[test]
fn adding_or_removing_secret_is_non_breaking() {
    assert_eq!(kinds(&model(""), &model("@secret")), [(SecretAdded, N)]);
    assert_eq!(kinds(&model("@secret"), &model("")), [(SecretRemoved, N)]);
}

#[test]
fn moving_from_pii_to_secret_reports_both_and_breaks_nothing() {
    let got = kinds(&model("@pii"), &model("@secret"));
    assert_eq!(got, [(PiiRemoved, N), (SecretAdded, N)]);
    assert!(!has_breaking(&diff(
        &ir(&model("@pii")),
        &ir(&model("@secret"))
    )));
}

#[test]
fn the_same_marks_report_nothing() {
    assert!(kinds(&model("@pii @secret"), &model("@pii @secret")).is_empty());
}

#[test]
fn an_event_field_is_compared_too() {
    let ev = |marks: &str| {
        format!("event Signed {{ who: string {marks} }}\nchannel Signs {{ carries Signed }}\n")
    };
    assert_eq!(kinds(&ev(""), &ev("@pii")), [(PiiAdded, N)]);
}

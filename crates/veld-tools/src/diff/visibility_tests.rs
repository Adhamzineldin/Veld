//! `veld diff` and `@visible` (`spec/platform.md` section 2).

use super::ChangeKind::{self, *};
use super::Severity::{self, NonBreaking as N, PossiblyBreaking as P};
use super::diff;
use crate::testutil::ir;

fn model(when: Option<&str>) -> String {
    let ann = when
        .map(|w| format!(" @visible(when: {w})"))
        .unwrap_or_default();
    format!(
        "model User {{ id: uuid\n roles: string[] }}\nprincipal User\n\
         model Booking {{\n  guestId: uuid\n  phone?: string{ann}\n}}\n\
         @auth(bearer)\nservice S at \"/s\" {{ get \"/\" f() -> Booking }}\n"
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

const OWN: &str = "user.id == guestId";
const STAFF: &str = "\"staff\" in user.roles";

#[test]
fn adding_a_condition_is_possibly_breaking() {
    assert_eq!(kinds(&model(None), &model(Some(OWN))), [(VisibleAdded, P)]);
}

#[test]
fn changing_the_expression_is_possibly_breaking() {
    assert_eq!(
        kinds(&model(Some(OWN)), &model(Some(STAFF))),
        [(VisibleChanged, P)]
    );
}

#[test]
fn removing_the_condition_is_compatible() {
    assert_eq!(
        kinds(&model(Some(OWN)), &model(None)),
        [(VisibleRemoved, N)]
    );
}

#[test]
fn a_contract_that_does_not_move_reports_nothing() {
    assert!(kinds(&model(Some(OWN)), &model(Some(OWN))).is_empty());
    assert!(kinds(&model(None), &model(None)).is_empty());
}

#[test]
fn spelling_the_same_expression_another_way_is_not_a_change() {
    assert!(kinds(&model(Some(OWN)), &model(Some("user.id  ==  guestId"))).is_empty());
}

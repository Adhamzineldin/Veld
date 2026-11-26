//! `veld diff` and `@timeout`, `@slo` (`spec/platform.md` sections 12 and 13): every row of the
//! table in `service_levels.rs`, and the `504` that is not reported twice.

use super::ChangeKind::{self, *};
use super::Severity::{self, NonBreaking as N, PossiblyBreaking as P};
use super::diff;
use crate::testutil::ir;

fn kinds(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| (c.kind, c.severity))
        .collect();
    v.sort();
    v
}

fn with(annotations: &str) -> String {
    format!(
        "model Room {{ id: uuid }}\nservice S at \"/s\" {{\n  {annotations}\n  get \"/{{id}}\" one(id: uuid) -> Room | NotFound\n}}\n"
    )
}

#[test]
fn adding_or_shortening_a_deadline_is_possibly_breaking_and_the_504_is_not_reported_again() {
    assert_eq!(
        kinds(&with(""), &with("@timeout(seconds(2))")),
        [(TimeoutChanged, P)]
    );
    assert_eq!(
        kinds(&with("@timeout(seconds(2))"), &with("@timeout(seconds(1))")),
        [(TimeoutChanged, P)]
    );
}

#[test]
fn removing_or_lengthening_a_deadline_is_compatible() {
    assert_eq!(
        kinds(&with("@timeout(seconds(2))"), &with("")),
        [(TimeoutChanged, N)]
    );
    assert_eq!(
        kinds(&with("@timeout(seconds(2))"), &with("@timeout(seconds(3))")),
        [(TimeoutChanged, N)]
    );
}

#[test]
fn the_same_deadline_written_another_way_is_no_change() {
    assert!(kinds(&with("@timeout(seconds(2))"), &with("@timeout(ms(2000))")).is_empty());
}

#[test]
fn stating_or_tightening_an_objective_is_compatible() {
    assert_eq!(
        kinds(&with(""), &with("@slo(p99: ms(300))")),
        [(SloChanged, N)]
    );
    assert_eq!(
        kinds(&with("@slo(p99: ms(300))"), &with("@slo(p99: ms(200))")),
        [(SloChanged, N)]
    );
    assert_eq!(
        kinds(&with("@slo(errors: 1)"), &with("@slo(errors: 0.5)")),
        [(SloChanged, N)]
    );
}

#[test]
fn removing_or_loosening_an_objective_is_possibly_breaking() {
    assert_eq!(
        kinds(&with("@slo(p99: ms(300))"), &with("")),
        [(SloChanged, P)]
    );
    assert_eq!(
        kinds(&with("@slo(p99: ms(300))"), &with("@slo(p99: ms(900))")),
        [(SloChanged, P)]
    );
    assert_eq!(
        kinds(&with("@slo(errors: 0.1)"), &with("@slo(errors: 1)")),
        [(SloChanged, P)]
    );
}

#[test]
fn a_contract_without_them_is_judged_as_before() {
    assert!(kinds(&with(""), &with("")).is_empty());
}

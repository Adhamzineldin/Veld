//! `veld diff` and `@limit` (`spec/platform.md` section 5).

use super::ChangeKind::{self, *};
use super::Severity::{self, NonBreaking as N, PossiblyBreaking as P};
use super::diff;
use crate::testutil::ir;

fn contract(on: &str) -> String {
    format!(
        "model User {{ id: uuid\n tenantId?: uuid @tenant }}\nprincipal User\nmodel Room {{ id: uuid }}\n\
         @auth(bearer)\nservice S at \"/s\" {{\n  {on}\n  get \"/\" f() -> Room[]\n}}\n"
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

const TEN: &str = "@limit(10, per: minute, by: user)";

#[test]
fn a_new_limit_is_possibly_breaking_and_its_429_is_not_reported_twice() {
    assert_eq!(kinds(&contract(""), &contract(TEN)), [(LimitChanged, P)]);
}

#[test]
fn a_removed_limit_is_compatible() {
    assert_eq!(kinds(&contract(TEN), &contract("")), [(LimitChanged, N)]);
}

#[test]
fn a_lower_rate_is_possibly_breaking_and_a_higher_one_compatible() {
    let lower = "@limit(5, per: minute, by: user)";
    let higher = "@limit(11, per: minute, by: user)";
    assert_eq!(kinds(&contract(TEN), &contract(lower)), [(LimitChanged, P)]);
    assert_eq!(
        kinds(&contract(TEN), &contract(higher)),
        [(LimitChanged, N)]
    );
}

#[test]
fn the_rate_is_compared_across_windows() {
    // 10 per minute is 0.1666 per second: one per second is higher, one per hour is lower.
    assert_eq!(
        kinds(
            &contract(TEN),
            &contract("@limit(1, per: second, by: user)")
        ),
        [(LimitChanged, N)]
    );
    assert_eq!(
        kinds(&contract(TEN), &contract("@limit(1, per: hour, by: user)")),
        [(LimitChanged, P)]
    );
}

#[test]
fn the_same_rate_in_another_window_is_possibly_breaking() {
    assert_eq!(
        kinds(
            &contract("@limit(60, per: minute, by: user)"),
            &contract("@limit(1, per: second, by: user)")
        ),
        [(LimitChanged, P)]
    );
}

#[test]
fn another_subject_is_possibly_breaking_even_when_the_rate_is_higher() {
    assert_eq!(
        kinds(
            &contract(TEN),
            &contract("@limit(100, per: minute, by: ip)")
        ),
        [(LimitChanged, P)]
    );
}

#[test]
fn a_contract_that_does_not_move_reports_nothing() {
    assert!(kinds(&contract(TEN), &contract(TEN)).is_empty());
    assert!(kinds(&contract(""), &contract("")).is_empty());
}

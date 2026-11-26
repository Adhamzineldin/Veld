//! `veld diff` and `@retain` (`spec/platform.md` section 23): every row of the table in
//! `retention.rs`.

use super::ChangeKind::{self, *};
use super::Severity::{self, NonBreaking as N, PossiblyBreaking as P};
use super::{diff, has_breaking};
use crate::testutil::ir;

fn guest(retain: &str) -> String {
    format!(
        "{retain}\nmodel Guest {{\n  id: uuid\n  name: string @pii\n  at?: datetime\n  seen?: datetime\n}}\nservice S at \"/s\" {{ get \"/\" f() -> Guest }}\n"
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

const D30: &str = "@retain(days(30), since: at, then: anonymize)";

#[test]
fn adding_a_retention_is_possibly_breaking_and_removing_it_is_not() {
    assert_eq!(kinds(&guest(""), &guest(D30)), [(RetentionAdded, P)]);
    assert_eq!(kinds(&guest(D30), &guest("")), [(RetentionRemoved, N)]);
}

#[test]
fn a_shorter_duration_is_possibly_breaking_and_a_longer_one_is_not() {
    let d7 = "@retain(days(7), since: at, then: anonymize)";
    assert_eq!(kinds(&guest(D30), &guest(d7)), [(RetentionShortened, P)]);
    assert_eq!(kinds(&guest(d7), &guest(D30)), [(RetentionLengthened, N)]);
}

#[test]
fn another_since_field_is_possibly_breaking() {
    let other = "@retain(days(30), since: seen, then: anonymize)";
    assert_eq!(kinds(&guest(D30), &guest(other)), [(RetentionSinceChanged, P)]);
}

#[test]
fn anonymize_to_delete_is_possibly_breaking_and_delete_to_anonymize_is_not() {
    let delete = "@retain(days(30), since: at, then: delete)";
    assert_eq!(kinds(&guest(D30), &guest(delete)), [(RetentionThenChanged, P)]);
    assert_eq!(kinds(&guest(delete), &guest(D30)), [(RetentionThenChanged, N)]);
}

#[test]
fn several_changes_are_reported_each() {
    let both = "@retain(days(7), since: seen, then: delete)";
    assert_eq!(
        kinds(&guest(D30), &guest(both)),
        [
            (RetentionShortened, P),
            (RetentionSinceChanged, P),
            (RetentionThenChanged, P)
        ]
    );
}

#[test]
fn the_same_retention_reports_nothing_and_a_possible_break_fails_a_check() {
    assert!(kinds(&guest(D30), &guest(D30)).is_empty());
    assert!(has_breaking(&diff(&ir(&guest("")), &ir(&guest(D30)))));
    assert!(!has_breaking(&diff(&ir(&guest(D30)), &ir(&guest("")))));
}

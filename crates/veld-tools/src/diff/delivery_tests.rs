//! `@retry`, `@deadLetter` and `@ordered` in `veld diff` (`spec/events.md` section 16): each change
//! is judged by what it does to a consumer generated from the old contract and one generated from
//! the new.

use super::ChangeKind::{self, *};
use super::Severity::{self, NonBreaking as N, PossiblyBreaking as P};
use super::{diff, has_breaking};
use crate::testutil::ir;

fn run(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| (c.kind, c.severity))
        .collect();
    v.sort();
    v
}

/// A contract whose channel has the annotations `anns`.
fn with(anns: &str) -> String {
    format!(
        "event Paid {{ id: uuid }}\n{anns}\nchannel C {{ carries Paid }}\nservice P {{ publishes C }}\nservice S {{ subscribes C as \"g\" {{ on Paid -> h }} }}\n"
    )
}

const RETRY3: &str = "@retry(max: 3)";
const DEAD: &str = "@deadLetter";

#[test]
fn a_contract_without_the_annotations_has_no_such_change() {
    assert!(run(&with(""), &with("")).is_empty());
    assert!(run(&with("@topic(\"t\")"), &with("@topic(\"t\")")).is_empty());
}

#[test]
fn identical_annotations_are_no_change() {
    let src = with("@retry(max: 3, backoff: fixed)\n@deadLetter\n@ordered");
    assert!(run(&src, &src).is_empty());
}

#[test]
fn retry_added_is_compatible_only_with_a_dead_letter() {
    assert_eq!(
        run(&with(""), &with(&format!("{RETRY3}\n{DEAD}"))),
        [(ChannelRetryAdded, N), (ChannelDeadLetterAdded, N)]
    );
    assert_eq!(
        run(&with(DEAD), &with(&format!("{RETRY3}\n{DEAD}"))),
        [(ChannelRetryAdded, N)]
    );
    assert_eq!(run(&with(""), &with(RETRY3)), [(ChannelRetryAdded, P)]);
    assert!(has_breaking(&diff(&ir(&with("")), &ir(&with(RETRY3)))));
}

#[test]
fn retry_removed_is_possibly_breaking_even_with_a_dead_letter() {
    assert_eq!(run(&with(RETRY3), &with("")), [(ChannelRetryRemoved, P)]);
    assert_eq!(
        run(&with(&format!("{RETRY3}\n{DEAD}")), &with(DEAD)),
        [(ChannelRetryRemoved, P)]
    );
}

#[test]
fn a_lower_max_without_a_dead_letter_is_possibly_breaking() {
    assert_eq!(
        run(&with("@retry(max: 5)"), &with("@retry(max: 2)")),
        [(ChannelRetryChanged, P)]
    );
}

#[test]
fn a_higher_max_or_a_lower_one_with_a_dead_letter_is_compatible() {
    assert_eq!(
        run(&with("@retry(max: 2)"), &with("@retry(max: 5)")),
        [(ChannelRetryChanged, N)]
    );
    assert_eq!(
        run(
            &with(&format!("@retry(max: 5)\n{DEAD}")),
            &with(&format!("@retry(max: 2)\n{DEAD}"))
        ),
        [(ChannelRetryChanged, N)]
    );
    // The new channel decides: a dead letter added in the same change keeps what is given up.
    assert_eq!(
        run(
            &with("@retry(max: 5)"),
            &with(&format!("@retry(max: 2)\n{DEAD}"))
        ),
        [(ChannelRetryChanged, N), (ChannelDeadLetterAdded, N)]
    );
}

#[test]
fn a_changed_backoff_is_compatible_and_so_is_an_unchanged_retry_with_the_default_backoff() {
    assert_eq!(
        run(
            &with("@retry(max: 3, backoff: fixed)"),
            &with("@retry(max: 3, backoff: exponential)")
        ),
        [(ChannelRetryChanged, N)]
    );
    // `backoff` defaults to `exponential`: writing it out is no change.
    assert!(
        run(
            &with("@retry(max: 3)"),
            &with("@retry(max: 3, backoff: exponential)")
        )
        .is_empty()
    );
    assert!(!has_breaking(&diff(
        &ir(&with("@retry(max: 3, backoff: fixed)")),
        &ir(&with("@retry(max: 3, backoff: exponential)"))
    )));
}

#[test]
fn a_dead_letter_added_is_compatible_and_removed_is_possibly_breaking() {
    assert_eq!(run(&with(""), &with(DEAD)), [(ChannelDeadLetterAdded, N)]);
    assert_eq!(run(&with(DEAD), &with("")), [(ChannelDeadLetterRemoved, P)]);
}

#[test]
fn ordered_added_or_removed_is_possibly_breaking() {
    assert_eq!(
        run(&with(""), &with("@ordered")),
        [(ChannelOrderedAdded, P)]
    );
    assert_eq!(
        run(&with("@ordered"), &with("")),
        [(ChannelOrderedRemoved, P)]
    );
}

#[test]
fn several_changes_are_several_entries_and_a_channel_that_is_new_has_none() {
    assert_eq!(
        run(&with(&format!("{RETRY3}\n@ordered")), &with(DEAD)),
        [
            (ChannelRetryRemoved, P),
            (ChannelDeadLetterAdded, N),
            (ChannelOrderedRemoved, P)
        ]
    );
    let old = "event Paid { id: uuid }\n";
    assert_eq!(
        run(old, &with("@retry(max: 1)\n@deadLetter\n@ordered")),
        [
            (ChannelAdded, N),
            (PublishesAdded, N),
            (SubscriptionAdded, N)
        ]
    );
}

#[test]
fn the_message_says_which_channel_and_why() {
    let changes = diff(&ir(&with("")), &ir(&with("@ordered")));
    assert_eq!(changes.len(), 1);
    assert!(changes[0].message.contains("`C`") && changes[0].message.contains("`@ordered`"));
    assert!(changes[0].subject.contains('C'));
}

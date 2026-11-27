//! `veld diff` and timed moves (`spec/platform.md` section 10): an edge that becomes timed, or
//! waits longer, refuses calls that were accepted (breaking); the opposite only accepts more
//! (compatible). A changed `since` field is possibly breaking.

use super::ChangeKind::{self, *};
use super::Severity::{self, Breaking as B, NonBreaking as N, PossiblyBreaking as P};
use super::diff;
use crate::testutil::ir;

fn contract(flow: &str, transition: &str, model: &str) -> String {
    format!(
        "enum St {{\n  pending, paid, cancelled\n  flow pending -> paid\n  {flow}\n  final paid, cancelled\n}}\n\
         model Booking {{ id: uuid, status: St, {model} }}\n\
         service S at \"/s\" {{\n  post \"/{{id}}/x\" act(id: uuid) -> Booking {{\n    fact booking: Booking = id\n    {transition}\n  }}\n}}\n"
    )
}

const SINCE: &str = "transition booking.status: pending -> cancelled since booking.pendingSince";
const MODEL: &str = "pendingSince: datetime, updatedAt: datetime";

fn timed(duration: &str) -> String {
    contract(
        &format!("flow pending -> cancelled after {duration}"),
        SINCE,
        MODEL,
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
fn a_longer_wait_is_breaking_and_a_shorter_one_is_compatible() {
    assert_eq!(
        kinds(&timed("hours(1)"), &timed("hours(2)")),
        [(FlowTimingChanged, B)]
    );
    assert_eq!(
        kinds(&timed("hours(2)"), &timed("hours(1)")),
        [(FlowTimingChanged, N)]
    );
}

#[test]
fn the_same_wait_written_another_way_is_no_change() {
    assert!(kinds(&timed("hours(1)"), &timed("minutes(60)")).is_empty());
}

#[test]
fn an_edge_that_becomes_timed_is_breaking_and_one_that_stops_is_compatible() {
    let plain = contract(
        "flow pending -> cancelled",
        "transition booking.status: pending -> cancelled",
        MODEL,
    );
    let new = kinds(&plain, &timed("hours(1)"));
    assert!(new.contains(&(FlowTimingChanged, B)), "{new:?}");
    let back = kinds(&timed("hours(1)"), &plain);
    assert!(back.contains(&(FlowTimingChanged, N)), "{back:?}");
}

#[test]
fn another_since_field_is_possibly_breaking() {
    let other = contract(
        "flow pending -> cancelled after hours(1)",
        "transition booking.status: pending -> cancelled since booking.updatedAt",
        MODEL,
    );
    let got = kinds(&timed("hours(1)"), &other);
    assert!(got.contains(&(TransitionChanged, P)), "{got:?}");
}

#[test]
fn a_flow_that_is_only_timed_is_not_reported_as_removed_and_added() {
    let got = kinds(&timed("hours(1)"), &timed("hours(3)"));
    assert!(
        !got.iter()
            .any(|(k, _)| matches!(k, FlowRemoved | FlowAdded)),
        "{got:?}"
    );
}

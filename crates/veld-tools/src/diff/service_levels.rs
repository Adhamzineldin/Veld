//! `@timeout` and `@slo` in `veld diff` (`spec/platform.md` sections 12 and 13).
//!
//! What a caller built against the old contract sees:
//!
//! | Change | Judged |
//! |---|---|
//! | a deadline is added, or made shorter | possibly breaking: a slow call may now answer `504` |
//! | a deadline is removed, or made longer | compatible |
//! | an objective is stated, or tightened (a lower latency, a lower error share) | compatible: the server promises more |
//! | an objective is removed, or loosened | possibly breaking: a consumer may rely on the old promise |
//!
//! The `504 DeadlineExceeded` that comes with a deadline is not reported again as an added error.

use veld_ir::{Endpoint, Slo};

use super::{Change, ChangeKind};

pub(super) fn diff_service_levels(s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    diff_timeout(s, o, n, out);
    diff_slo(s, o.slo.as_ref(), n.slo.as_ref(), out);
}

fn ms(t: &Option<String>) -> Option<u64> {
    t.as_deref().and_then(|t| t.parse().ok())
}

fn diff_timeout(s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    let kind = ChangeKind::TimeoutChanged;
    match (ms(&o.timeout_ms), ms(&n.timeout_ms)) {
        (None, Some(new)) => out.push(Change::possibly_breaking(
            kind,
            s,
            format!("the endpoint now has a deadline of {new} ms: a slow call may answer 504"),
        )),
        (Some(old), None) => out.push(Change::non_breaking(
            kind,
            s,
            format!("the deadline of {old} ms was removed"),
        )),
        (Some(old), Some(new)) if new < old => out.push(Change::possibly_breaking(
            kind,
            s,
            format!("the deadline is shorter: {new} ms instead of {old} ms"),
        )),
        (Some(old), Some(new)) if new > old => out.push(Change::non_breaking(
            kind,
            s,
            format!("the deadline is longer: {new} ms instead of {old} ms"),
        )),
        _ => {}
    }
}

fn members(slo: &Slo) -> Vec<(&'static str, Option<f64>)> {
    let num = |t: &Option<String>| t.as_deref().and_then(|t| t.parse::<f64>().ok());
    vec![
        ("p50", num(&slo.p50_ms)),
        ("p90", num(&slo.p90_ms)),
        ("p95", num(&slo.p95_ms)),
        ("p99", num(&slo.p99_ms)),
        ("errors", num(&slo.errors)),
    ]
}

fn diff_slo(s: &str, old: Option<&Slo>, new: Option<&Slo>, out: &mut Vec<Change>) {
    let kind = ChangeKind::SloChanged;
    let empty = Slo::default();
    let (a, b) = (
        members(old.unwrap_or(&empty)),
        members(new.unwrap_or(&empty)),
    );
    for ((name, was), (_, now)) in a.into_iter().zip(b) {
        match (was, now) {
            (None, Some(v)) => out.push(Change::non_breaking(
                kind,
                s,
                format!("the objective `{name}: {v}` was stated"),
            )),
            (Some(v), None) => out.push(Change::possibly_breaking(
                kind,
                s,
                format!("the objective `{name}: {v}` was removed"),
            )),
            (Some(x), Some(y)) if y < x => out.push(Change::non_breaking(
                kind,
                s,
                format!("the objective `{name}` is tighter: {y} instead of {x}"),
            )),
            (Some(x), Some(y)) if y > x => out.push(Change::possibly_breaking(
                kind,
                s,
                format!("the objective `{name}` is looser: {y} instead of {x}"),
            )),
            _ => {}
        }
    }
}

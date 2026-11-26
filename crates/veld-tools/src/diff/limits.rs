//! `@limit` in `veld diff` (`spec/platform.md` section 5).
//!
//! What a caller built against the old contract sees is whether a call it makes may now be
//! refused with `429`:
//!
//! | Change | Judged |
//! |---|---|
//! | a limit is added | possibly breaking: a caller may now be refused |
//! | the rate is lower (`count / window`) | possibly breaking: a caller that fit may now be refused |
//! | the rate is higher | compatible |
//! | the same rate in another count and window (`60 per minute`, `1 per second`) | possibly breaking: the bursts that fit are not the same |
//! | `by` changes (`user`, `tenant`, `ip`) | possibly breaking: the calls are counted together in another way |
//! | the limit is removed | compatible |
//!
//! The `429 TooManyRequests` that comes with a limit is not reported again as an added error.

use std::cmp::Ordering;

use veld_ir::{Endpoint, Limit};

use super::{Change, ChangeKind};

fn text(l: &Limit) -> String {
    format!("{} per {} by {}", l.count, l.per.word(), l.by.word())
}

pub(super) fn diff_limit(s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    let kind = ChangeKind::LimitChanged;
    match (&o.limit, &n.limit) {
        (None, Some(new)) => out.push(Change::possibly_breaking(
            kind,
            s,
            format!(
                "the endpoint now has a limit of {}: a caller may be refused with 429",
                text(new)
            ),
        )),
        (Some(old), None) => out.push(Change::non_breaking(
            kind,
            s,
            format!("the limit of {} was removed", text(old)),
        )),
        (Some(old), Some(new)) if old != new => out.push(both(s, old, new)),
        (Some(_) | None, Some(_) | None) => {}
    }
}

/// The rate of a limit compared with another by cross-multiplying, which is exact:
/// `a.count / a.seconds` against `b.count / b.seconds`.
fn rate(a: &Limit, b: &Limit) -> Ordering {
    let left = u64::from(a.count) * u64::from(b.per.seconds());
    let right = u64::from(b.count) * u64::from(a.per.seconds());
    left.cmp(&right)
}

fn both(s: &str, old: &Limit, new: &Limit) -> Change {
    let kind = ChangeKind::LimitChanged;
    let was = text(old);
    let is = text(new);
    if old.by != new.by {
        return Change::possibly_breaking(
            kind,
            s,
            format!(
                "the limit is now `{is}` instead of `{was}`: calls are counted together in another way"
            ),
        );
    }
    match rate(new, old) {
        Ordering::Less => Change::possibly_breaking(
            kind,
            s,
            format!("the limit is tighter: `{is}` instead of `{was}`"),
        ),
        Ordering::Greater => Change::non_breaking(
            kind,
            s,
            format!("the limit is looser: `{is}` instead of `{was}`"),
        ),
        Ordering::Equal => Change::possibly_breaking(
            kind,
            s,
            format!(
                "the limit is the same rate in another window: `{is}` instead of `{was}`; the bursts that fit are not the same"
            ),
        ),
    }
}

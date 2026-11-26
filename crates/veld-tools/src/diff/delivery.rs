//! `@retry`, `@deadLetter` and `@ordered` on a channel (`spec/events.md` sections 10.2 and 16).
//!
//! None of the three changes a byte on the wire: a consumer generated from the old contract and
//! one generated from the new read the same messages. What differs is what each does with a
//! message it cannot handle, and the broker objects the adapter declares. Each change is judged
//! by what it does to a consumer generated from the old contract and one from the new:
//!
//! | Change | Result |
//! |---|---|
//! | `@retry` added, the new channel has `@deadLetter` | compatible |
//! | `@retry` added, no `@deadLetter` | possibly breaking: a message that was redelivered is gone after its retries |
//! | `@retry` removed | possibly breaking: the transport redelivers without bound, and one failing message can block the stream |
//! | `max` lowered, no `@deadLetter` in the new channel | possibly breaking: the message is given up earlier and is gone |
//! | `max` raised, or lowered with `@deadLetter` | compatible |
//! | `backoff` changed | compatible: only the waits differ |
//! | `@deadLetter` added | compatible: nothing is read from the dead-letter topic yet (on Kafka it must exist first) |
//! | `@deadLetter` removed | possibly breaking: rejected messages and exhausted retries are gone |
//! | `@ordered` added or removed | possibly breaking: consumers may depend on the order, and the adapter declares the queue or the partition differently |

use std::collections::BTreeMap;

use veld_ir::{Channel, Retry};

use super::{Change, ChangeKind, Ctx};

pub(super) fn diff_delivery(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let new: BTreeMap<&str, &Channel> = ctx
        .new
        .channels
        .iter()
        .map(|c| (c.qualified.as_str(), c))
        .collect();
    for o in &ctx.old.channels {
        if let Some(n) = new.get(o.qualified.as_str()) {
            diff_channel(&o.qualified, o, n, out);
        }
    }
}

fn diff_channel(q: &str, o: &Channel, n: &Channel, out: &mut Vec<Change>) {
    diff_retry(q, o, n, out);
    diff_dead_letter(q, o, n, out);
    if o.ordered != n.ordered {
        let (kind, what) = if n.ordered {
            (ChangeKind::ChannelOrderedAdded, "now has `@ordered`")
        } else {
            (
                ChangeKind::ChannelOrderedRemoved,
                "no longer has `@ordered`",
            )
        };
        out.push(Change::possibly_breaking(
            kind,
            q,
            format!(
                "channel `{}` {what}: consumers may depend on the order, and the adapter declares the queue or the partition differently",
                n.name
            ),
        ));
    }
}

fn diff_dead_letter(q: &str, o: &Channel, n: &Channel, out: &mut Vec<Change>) {
    match (o.dead_letter, n.dead_letter) {
        (false, true) => out.push(Change::non_breaking(
            ChangeKind::ChannelDeadLetterAdded,
            q,
            format!(
                "channel `{}` now has `@deadLetter`: rejected messages and exhausted retries are copied to `{}.dlq`",
                n.name, n.topic
            ),
        )),
        (true, false) => out.push(Change::possibly_breaking(
            ChangeKind::ChannelDeadLetterRemoved,
            q,
            format!(
                "channel `{}` no longer has `@deadLetter`: rejected messages and exhausted retries are gone",
                n.name
            ),
        )),
        (false, false) | (true, true) => {}
    }
}

fn diff_retry(q: &str, o: &Channel, n: &Channel, out: &mut Vec<Change>) {
    match (o.retry, n.retry) {
        (None, Some(r)) => out.push(retry_added(q, n, r)),
        (Some(_), None) => out.push(Change::possibly_breaking(
            ChangeKind::ChannelRetryRemoved,
            q,
            format!(
                "channel `{}` no longer has `@retry`: the transport redelivers a failing message without bound",
                n.name
            ),
        )),
        (Some(was), Some(is)) => out.extend(retry_changed(q, n, was, is)),
        (None, None) => {}
    }
}

fn retry_added(q: &str, n: &Channel, r: Retry) -> Change {
    if n.dead_letter {
        return Change::non_breaking(
            ChangeKind::ChannelRetryAdded,
            q,
            format!(
                "channel `{}` now has `@retry(max: {})`, with a dead letter for what is given up",
                n.name, r.max
            ),
        );
    }
    Change::possibly_breaking(
        ChangeKind::ChannelRetryAdded,
        q,
        format!(
            "channel `{}` now has `@retry(max: {})` and no `@deadLetter`: a message whose retries are used up is gone",
            n.name, r.max
        ),
    )
}

fn retry_changed(q: &str, n: &Channel, was: Retry, is: Retry) -> Option<Change> {
    if was == is {
        return None;
    }
    let text = format!(
        "`@retry` of channel `{}` changed from max {} {} to max {} {}",
        n.name,
        was.max,
        was.backoff.name(),
        is.max,
        is.backoff.name()
    );
    if is.max < was.max && !n.dead_letter {
        return Some(Change::possibly_breaking(
            ChangeKind::ChannelRetryChanged,
            q,
            format!("{text}: a message is given up earlier and, with no `@deadLetter`, is gone"),
        ));
    }
    Some(Change::non_breaking(
        ChangeKind::ChannelRetryChanged,
        q,
        text,
    ))
}

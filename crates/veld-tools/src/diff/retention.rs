//! `@retain` in `veld diff` (`spec/platform.md` section 23).
//!
//! Retention changes nothing on the wire. What it changes is how long the data is kept, so each
//! change is judged by what it does to whoever relies on the data:
//!
//! | Change | Verdict | Why |
//! |---|---|---|
//! | `@retain` added to a model | possibly breaking | what was kept without limit is now due |
//! | `@retain` removed | non-breaking | more is kept, not less |
//! | a shorter duration | possibly breaking | data that was kept is now due earlier |
//! | a longer duration | non-breaking | |
//! | `since:` another field | possibly breaking | the clock starts at another moment, and the tool cannot tell whether it is earlier |
//! | `then: anonymize` becomes `delete` | possibly breaking | the values that were kept are now gone |
//! | `then: delete` becomes `anonymize` | non-breaking | less is lost |

use veld_ir::retention::{Retain, RetainThen};
use veld_ir::{DeclKind, Model};

use super::{Change, ChangeKind, Ctx, decls};

fn retain_of(d: &veld_ir::Decl) -> Option<&Retain> {
    match &d.kind {
        DeclKind::Model(Model { retain, .. }) => retain.as_ref(),
        DeclKind::Error(_)
        | DeclKind::Enum(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_)
        | DeclKind::Event(_) => None,
    }
}

pub(super) fn diff_retention(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (decls::by_name(ctx.old), decls::by_name(ctx.new));
    for (q, o) in &old {
        let Some(n) = new.get(q) else { continue };
        match (retain_of(o), retain_of(n)) {
            (None, Some(r)) => out.push(Change::possibly_breaking(
                ChangeKind::RetentionAdded,
                q,
                format!(
                    "`{}` is now kept for {} after `{}`, then {}: what was kept without limit is due",
                    n.name,
                    days(r.after_ms),
                    r.since,
                    r.then.word()
                ),
            )),
            (Some(_), None) => out.push(Change::non_breaking(
                ChangeKind::RetentionRemoved,
                q,
                format!("`{}` is kept without limit again", n.name),
            )),
            (Some(a), Some(b)) => both(q, &n.name, a, b, out),
            (None, None) => {}
        }
    }
}

fn both(q: &str, name: &str, a: &Retain, b: &Retain, out: &mut Vec<Change>) {
    if b.after_ms < a.after_ms {
        out.push(Change::possibly_breaking(
            ChangeKind::RetentionShortened,
            q,
            format!(
                "`{name}` is kept for {} instead of {}: data that was kept is due earlier",
                days(b.after_ms),
                days(a.after_ms)
            ),
        ));
    } else if b.after_ms > a.after_ms {
        out.push(Change::non_breaking(
            ChangeKind::RetentionLengthened,
            q,
            format!(
                "`{name}` is kept for {} instead of {}",
                days(b.after_ms),
                days(a.after_ms)
            ),
        ));
    }
    if a.since != b.since {
        out.push(Change::possibly_breaking(
            ChangeKind::RetentionSinceChanged,
            q,
            format!(
                "the retention of `{name}` now counts from `{}` instead of `{}`",
                b.since, a.since
            ),
        ));
    }
    if a.then != b.then {
        let worse = b.then == RetainThen::Delete;
        out.push(Change::when(
            worse,
            ChangeKind::RetentionThenChanged,
            q,
            format!(
                "`{name}` is now {} instead of {} when its time is up",
                b.then.word(),
                a.then.word()
            ),
        ));
    }
}

/// `30 days`, `1 day`.
fn days(ms: i64) -> String {
    let n = ms / 86_400_000;
    format!("{n} day{}", if n == 1 { "" } else { "s" })
}

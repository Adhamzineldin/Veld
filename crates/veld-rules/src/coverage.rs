//! Which enforceable units an example evaluated, and how each ended (`spec/rules.md` section
//! 19.1, `veld test --coverage`).
//!
//! The reference pipeline reports every unit it evaluates to a tape, if one is running on the
//! current thread. Nothing is recorded, and nothing changes, when no tape runs, so no existing
//! behaviour or output depends on this module.
//!
//! A unit is named by a key: the rule id of an invariant, a `require`, an `ensure` or a
//! `transition`, and [`allow_key`] for an `allow` line, which has no id.

use std::cell::RefCell;

use veld_ir::Origin;

/// One evaluation of one unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitEvent {
    /// The rule id, or [`allow_key`] of an `allow` line.
    pub key: String,
    /// True when the unit held (an `allow` that permitted, a `transition` that was a legal
    /// move). An evaluation error is not held.
    pub held: bool,
}

#[derive(Default)]
struct Tape {
    /// True while the reference pipeline runs; the checks that only validate a filled-in
    /// example are not recorded.
    armed: u32,
    events: Vec<UnitEvent>,
}

thread_local! {
    static TAPE: RefCell<Option<Tape>> = const { RefCell::new(None) };
}

/// The key of an `allow` line: the line has no id, so its position names it.
#[must_use]
pub fn allow_key(origin: &Origin) -> String {
    format!("allow@{}:{}:{}", origin.file, origin.line, origin.col)
}

/// Runs `f` with a tape and returns what it recorded, in the order of evaluation. A tape that
/// was already running is put back afterwards.
pub fn collect<R>(f: impl FnOnce() -> R) -> (R, Vec<UnitEvent>) {
    let outer = TAPE.with(|t| {
        t.try_borrow_mut()
            .ok()
            .and_then(|mut t| t.replace(Tape::default()))
    });
    let out = f();
    let tape = TAPE.with(|t| match t.try_borrow_mut() {
        Ok(mut t) => std::mem::replace(&mut *t, outer),
        Err(_) => None,
    });
    (out, tape.map(|t| t.events).unwrap_or_default())
}

/// Runs `f` as the reference pipeline: what it evaluates is recorded.
pub(crate) fn armed<R>(f: impl FnOnce() -> R) -> R {
    let bump = |by: i64| {
        TAPE.with(|t| {
            if let Ok(mut t) = t.try_borrow_mut()
                && let Some(tape) = t.as_mut()
            {
                tape.armed = u32::try_from(i64::from(tape.armed) + by).unwrap_or(0);
            }
        });
    };
    bump(1);
    let out = f();
    bump(-1);
    out
}

/// Runs `f` with recording off: it only fills in what a unit is evaluated on (the defaults of a
/// fact), it is not an evaluation of the units of that value.
pub(crate) fn muted<R>(f: impl FnOnce() -> R) -> R {
    let saved = TAPE.with(|t| {
        t.try_borrow_mut()
            .ok()
            .and_then(|mut t| t.as_mut().map(|tape| std::mem::take(&mut tape.armed)))
    });
    let out = f();
    TAPE.with(|t| {
        if let (Ok(mut t), Some(armed)) = (t.try_borrow_mut(), saved)
            && let Some(tape) = t.as_mut()
        {
            tape.armed = armed;
        }
    });
    out
}

/// Records that the unit `key` was evaluated. `key` is only built when a tape is running.
pub(crate) fn record(key: impl FnOnce() -> String, held: bool) {
    TAPE.with(|t| {
        if let Ok(mut t) = t.try_borrow_mut()
            && let Some(tape) = t.as_mut().filter(|tape| tape.armed > 0)
        {
            tape.events.push(UnitEvent { key: key(), held });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_recorded_without_a_collector() {
        armed(|| record(|| "a".to_owned(), true));
        let ((), events) = collect(|| ());
        assert!(events.is_empty());
    }

    #[test]
    fn only_what_runs_armed_is_recorded_and_muted_code_is_left_out() {
        let ((), events) = collect(|| {
            record(|| "outside".to_owned(), true);
            armed(|| {
                record(|| "held".to_owned(), true);
                muted(|| record(|| "filling".to_owned(), true));
                record(|| "violated".to_owned(), false);
            });
        });
        let keys: Vec<(&str, bool)> = events.iter().map(|e| (e.key.as_str(), e.held)).collect();
        assert_eq!(keys, [("held", true), ("violated", false)]);
    }

    #[test]
    fn a_collector_inside_another_leaves_the_outer_one_intact() {
        let (inner, outer) = collect(|| {
            armed(|| record(|| "before".to_owned(), true));
            let ((), inner) = collect(|| armed(|| record(|| "inner".to_owned(), false)));
            armed(|| record(|| "after".to_owned(), true));
            inner
        });
        assert_eq!(inner.len(), 1);
        let keys: Vec<&str> = outer.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, ["before", "after"]);
    }

    #[test]
    fn an_allow_line_is_named_by_its_position() {
        let origin = Origin {
            file: "a.veld".to_owned(),
            line: 3,
            col: 5,
        };
        assert_eq!(allow_key(&origin), "allow@a.veld:3:5");
    }
}

//! The endpoints with a timed transition, for `veld check` (`spec/platform.md` section 10): the
//! endpoints a due-moves runner has to call, from the reference's `veld_rules::timed_endpoints`.

use serde::Serialize;
use veld_ir::Ir;

/// One endpoint that takes a timed edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimedMove {
    pub service: String,
    pub endpoint: String,
    /// The state the move leaves and the state it enters, by wire value.
    pub from: String,
    pub to: String,
    /// How long the fact waits in `from` before the move is due, in milliseconds.
    pub after_ms: i64,
}

/// Every endpoint with a timed transition, sorted by service, endpoint, then the move.
#[must_use]
pub fn timed_moves(ir: &Ir) -> Vec<TimedMove> {
    let mut moves: Vec<TimedMove> = veld_rules::timed_endpoints(ir)
        .into_iter()
        .map(|t| TimedMove {
            service: t.service_name,
            endpoint: t.endpoint,
            from: t.from,
            to: t.to,
            after_ms: t.after_ms,
        })
        .collect();
    moves.sort_by(|a, b| {
        (&a.service, &a.endpoint, &a.from, &a.to).cmp(&(&b.service, &b.endpoint, &b.from, &b.to))
    });
    moves
}

/// The one summary line of `veld check`: `timed: Bookings.expire (pending -> expired after
/// 3600000 ms), ...`; `None` when nothing is timed.
#[must_use]
pub fn timed_line(ir: &Ir) -> Option<String> {
    let moves = timed_moves(ir);
    if moves.is_empty() {
        return None;
    }
    let items: Vec<String> = moves
        .iter()
        .map(|m| {
            format!(
                "{}.{} ({} -> {} after {} ms)",
                m.service, m.endpoint, m.from, m.to, m.after_ms
            )
        })
        .collect();
    Some(format!("timed: {}", items.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::ir;

    const TIMED: &str = "enum S { a, b, flow a -> b after hours(1)\n  final b }\n\
model D { id: uuid, state: S = .a, entered: datetime }\n\
service Z at \"/z\" {\n  @auth(none)\n  post \"/{id}/zb\" zb(id: uuid) -> 200 D {\n    fact d: D = id\n    transition d.state: a -> b since d.entered\n  }\n}\n\
service A at \"/a\" {\n  @auth(none)\n  post \"/{id}/ab\" ab(id: uuid) -> 200 D {\n    fact d: D = id\n    transition d.state: a -> b since d.entered\n  }\n}\n";

    #[test]
    fn the_moves_are_sorted_with_their_wait_in_milliseconds() {
        let moves = timed_moves(&ir(TIMED));
        let names: Vec<String> = moves
            .iter()
            .map(|m| format!("{}.{}", m.service, m.endpoint))
            .collect();
        assert_eq!(names, ["A.ab", "Z.zb"]);
        assert_eq!((moves[0].from.as_str(), moves[0].to.as_str()), ("a", "b"));
        assert_eq!(moves[0].after_ms, 3_600_000);
    }

    #[test]
    fn the_line_names_each_move_and_is_absent_without_timed_edges() {
        let line = timed_line(&ir(TIMED)).unwrap();
        assert_eq!(
            line,
            "timed: A.ab (a -> b after 3600000 ms), Z.zb (a -> b after 3600000 ms)"
        );
        let plain =
            "model M { id: uuid }\nservice S at \"/s\" {\n  @auth(none)\n  get \"/\" f() -> M\n}\n";
        assert_eq!(timed_line(&ir(plain)), None);
    }
}

//! Pure renderers: they take a [`Theme`] and data, and return text. Tests render with
//! [`Theme::PLAIN`] and assert on structure; the CLI renders with the real theme.

use std::time::Duration;

use super::theme::{Style, Theme, width};

/// `veld 2.0 · generate  project-name`
#[must_use]
pub fn header(t: &Theme, command: &str, detail: &str) -> String {
    let mut s = format!(
        "{} {} {} {}",
        t.paint(Style::Brand, "veld"),
        t.paint(Style::Dim, env!("CARGO_PKG_VERSION")),
        t.paint(Style::Dim, if t.unicode { "·" } else { "-" }),
        t.paint(Style::Bold, command)
    );
    if !detail.is_empty() {
        s.push_str("  ");
        s.push_str(&t.paint(Style::Dim, detail));
    }
    s
}

/// `✓ message`
#[must_use]
pub fn ok(t: &Theme, msg: &str) -> String {
    format!("{} {msg}", t.paint(Style::Green, t.sym().ok))
}

/// `✗ message`
#[must_use]
pub fn fail(t: &Theme, msg: &str) -> String {
    format!("{} {msg}", t.paint(Style::Red, t.sym().fail))
}

/// `→ message`, dim: a hint at what to do next.
#[must_use]
pub fn hint(t: &Theme, msg: &str) -> String {
    format!(
        "{} {}",
        t.paint(Style::Cyan, t.sym().arrow),
        t.paint(Style::Dim, msg)
    )
}

/// Aligned `label  value` rows, indented by `indent` spaces.
#[must_use]
pub fn rows(t: &Theme, indent: usize, rows: &[(&str, String)]) -> Vec<String> {
    let w = rows.iter().map(|(k, _)| width(k)).max().unwrap_or(0);
    rows.iter()
        .map(|(k, v)| {
            format!(
                "{}{}{}  {v}",
                " ".repeat(indent),
                t.paint(Style::Dim, k),
                " ".repeat(w - width(k))
            )
        })
        .collect()
}

/// A titled box in the guide-rail style:
///
/// ```text
/// ◇  Summary ─────────╮
/// │                   │
/// ```
/// Plain rails only (no right edge), so long lines never break the frame.
#[must_use]
pub fn note_box(t: &Theme, title: &str, body: &[String]) -> Vec<String> {
    let s = t.sym();
    let inner = body
        .iter()
        .map(|l| width(l))
        .max()
        .unwrap_or(0)
        .max(width(title) + 2);
    let mut out = vec![format!(
        "{}  {} {}",
        t.paint(Style::Green, s.done),
        t.paint(Style::Bold, title),
        t.paint(
            Style::Dim,
            &s.rule.repeat(inner.saturating_sub(width(title)))
        )
    )];
    out.push(t.paint(Style::Dim, s.rail));
    for l in body {
        out.push(format!("{}  {l}", t.paint(Style::Dim, s.rail)));
    }
    out.push(t.paint(Style::Dim, s.rail));
    out.push(t.paint(
        Style::Dim,
        &format!("{}{}", s.bottom, s.rule.repeat(inner + 3)),
    ));
    out
}

/// A numbered list of next steps: `command  what it does`.
#[must_use]
pub fn next_steps(t: &Theme, steps: &[(&str, &str)]) -> Vec<String> {
    let w = steps.iter().map(|(c, _)| width(c)).max().unwrap_or(0);
    steps
        .iter()
        .map(|(c, d)| {
            format!(
                "{}{}  {}",
                t.paint(Style::Bold, c),
                " ".repeat(w - width(c)),
                t.paint(Style::Dim, d)
            )
        })
        .collect()
}

/// `[12:03:44] ✓ message`: a timestamped status line of `veld watch`. `kind` is one of
/// `ok`, `ready`, `fail` and `changed`.
#[must_use]
pub fn stamped(t: &Theme, time: &str, kind: &str, msg: &str) -> String {
    let (mark, style) = match kind {
        "fail" => (t.sym().fail, Style::Red),
        "changed" => (if t.unicode { "↻" } else { "~" }, Style::Yellow),
        _ => (t.sym().ok, Style::Green),
    };
    let msg = if kind == "changed" {
        format!("changed {msg}")
    } else {
        msg.to_string()
    };
    format!(
        "{} {} {msg}",
        t.paint(Style::Dim, &format!("[{time}]")),
        t.paint(style, mark)
    )
}

/// One target's outcome in a `generate` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSummary {
    pub name: String,
    pub out: String,
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub unchanged: usize,
}

/// `1.2s` or `340ms`.
#[must_use]
pub fn elapsed(d: Duration) -> String {
    if d.as_millis() >= 1000 {
        format!("{:.1}s", d.as_secs_f64())
    } else {
        format!("{}ms", d.as_millis())
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

/// `✓ name -> out: 1 created, 0 updated, ...` (or `~ ... would create ...` for a dry run).
#[must_use]
pub fn target_line(t: &Theme, s: &TargetSummary, dry_run: bool) -> String {
    let counts = if dry_run {
        format!(
            "would create {}, update {}, delete {} ({} unchanged)",
            s.created, s.updated, s.deleted, s.unchanged
        )
    } else {
        format!(
            "{} created, {} updated, {} deleted, {} unchanged",
            s.created, s.updated, s.deleted, s.unchanged
        )
    };
    let mark = if dry_run {
        t.paint(Style::Yellow, "~")
    } else {
        t.paint(Style::Green, t.sym().ok)
    };
    format!(
        "{mark} {} {} {}: {counts}",
        t.paint(Style::Bold, &s.name),
        t.paint(Style::Dim, "->"),
        t.paint(Style::Dim, &s.out)
    )
}

/// The closing line: `2 targets, 5 files changed in 340ms`. `time` is `None` in tests.
#[must_use]
pub fn generate_tail(
    t: &Theme,
    targets: &[TargetSummary],
    dry_run: bool,
    time: Option<Duration>,
) -> String {
    let changed: usize = targets
        .iter()
        .map(|s| s.created + s.updated + s.deleted)
        .sum();
    let mut tail = format!(
        "{}, {} {}",
        plural(targets.len(), "target"),
        plural(changed, "file"),
        if dry_run { "would change" } else { "changed" }
    );
    if let Some(d) = time {
        tail.push_str(&format!(" in {}", elapsed(d)));
    }
    t.paint(Style::Dim, &tail)
}

/// The per-target lines, a blank line and the closing line of a generate run.
#[cfg(test)]
#[must_use]
pub fn generate_summary(
    t: &Theme,
    targets: &[TargetSummary],
    dry_run: bool,
    time: Option<Duration>,
) -> Vec<String> {
    let mut lines: Vec<String> = targets.iter().map(|s| target_line(t, s, dry_run)).collect();
    lines.push(String::new());
    lines.push(generate_tail(t, targets, dry_run, time));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_align_on_the_longest_label() {
        let r = rows(&Theme::PLAIN, 2, &[("a", "1".into()), ("long", "2".into())]);
        assert_eq!(r, ["  a     1", "  long  2"]);
    }

    #[test]
    fn note_box_has_title_rails_and_a_closing_corner() {
        let b = note_box(&Theme::PLAIN, "Summary", &["x".to_string()]);
        assert!(b[0].starts_with("◇  Summary"));
        assert_eq!(b[2], "│  x");
        assert!(b.last().is_some_and(|l| l.starts_with('└')));
        let a = note_box(&Theme::ASCII, "Summary", &["x".to_string()]);
        assert!(a.iter().all(|l| l.is_ascii()));
    }

    fn sample() -> Vec<TargetSummary> {
        vec![
            TargetSummary {
                name: "api".into(),
                out: "gen/server".into(),
                created: 6,
                updated: 1,
                deleted: 0,
                unchanged: 2,
            },
            TargetSummary {
                name: "sdk".into(),
                out: "gen/client".into(),
                created: 0,
                updated: 0,
                deleted: 0,
                unchanged: 9,
            },
        ]
    }

    #[test]
    fn generate_summary_snapshot() {
        let time = Some(Duration::from_millis(340));
        let lines = generate_summary(&Theme::PLAIN, &sample(), false, time);
        insta::assert_snapshot!(lines.join("\n"));
    }

    #[test]
    fn dry_run_summary_snapshot_and_ascii_has_no_unicode() {
        let lines = generate_summary(&Theme::PLAIN, &sample(), true, None);
        insta::assert_snapshot!(lines.join("\n"));
        let ascii = generate_summary(&Theme::ASCII, &sample(), false, None);
        assert!(ascii.iter().all(|l| l.is_ascii()), "{ascii:?}");
    }

    #[test]
    fn stamped_lines_carry_time_and_mark() {
        assert_eq!(
            stamped(&Theme::PLAIN, "09:05:01", "ok", "regenerated in 12ms"),
            "[09:05:01] ✓ regenerated in 12ms"
        );
        assert_eq!(
            stamped(&Theme::PLAIN, "09:05:01", "fail", "x"),
            "[09:05:01] ✗ x"
        );
        assert_eq!(
            stamped(&Theme::ASCII, "09:05:01", "changed", "a.veld"),
            "[09:05:01] ~ changed a.veld"
        );
    }

    #[test]
    fn elapsed_picks_a_unit() {
        assert_eq!(elapsed(Duration::from_millis(340)), "340ms");
        assert_eq!(elapsed(Duration::from_millis(1250)), "1.2s");
    }
}

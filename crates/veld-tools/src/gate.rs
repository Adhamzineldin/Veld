//! The breaking-change gate of `veld generate`: compare the contract about to be generated with
//! the baseline (`veld.lock.ir.json`) before anything is written, and decide whether generation
//! may continue.
//!
//! v1 compared against its lock, then prompted, failed with `--strict`, or continued with
//! `--force`. v2 keeps the behaviour with clearer flags: `--strict` blocks, `--accept-breaking`
//! continues, and a terminal is asked. `--force` stays reserved for overwriting files.

use veld_ir::Ir;

use crate::diff::{Change, Severity, diff};

/// The outcome of comparing two contracts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    /// Every change, in `diff` order.
    pub changes: Vec<Change>,
}

/// How the caller wants breaking changes handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GatePolicy {
    /// `--strict`: breaking and possibly breaking changes always block.
    pub strict: bool,
    /// `--accept-breaking`: breaking and possibly breaking changes never block.
    pub accept_breaking: bool,
    /// Whether stdin and stderr are a terminal that can be asked.
    pub interactive: bool,
}

/// What `generate` does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateDecision {
    /// No breaking change, or the user accepted them up front.
    Proceed,
    /// Ask "Generate anyway? [y/N]" (only offered on a terminal).
    Prompt,
    /// Stop with a non-zero exit code and the list of breaking changes.
    Block,
}

impl GateResult {
    /// The breaking changes only.
    #[must_use]
    pub fn breaking(&self) -> Vec<&Change> {
        self.changes
            .iter()
            .filter(|c| c.severity == Severity::Breaking)
            .collect()
    }

    /// The non-breaking changes only.
    #[must_use]
    pub fn non_breaking(&self) -> Vec<&Change> {
        self.changes
            .iter()
            .filter(|c| c.severity == Severity::NonBreaking)
            .collect()
    }

    /// The changes the tool cannot judge: a rule whose expression changed, or a non-additive
    /// change to an event, a channel or a subscription.
    #[must_use]
    pub fn possibly_breaking(&self) -> Vec<&Change> {
        self.changes
            .iter()
            .filter(|c| c.severity == Severity::PossiblyBreaking)
            .collect()
    }

    /// Whether a change stops `generate`: a definite or a possible break (`Severity::fails_check`,
    /// the same test as `veld diff --strict`).
    #[must_use]
    pub fn has_breaking(&self) -> bool {
        self.changes.iter().any(|c| c.severity.fails_check())
    }

    /// The decision for `policy`. `strict` wins over everything, then `accept_breaking`; an
    /// unattended run (no terminal) blocks rather than guessing.
    #[must_use]
    pub fn decide(&self, policy: GatePolicy) -> GateDecision {
        if !self.has_breaking() {
            GateDecision::Proceed
        } else if policy.strict {
            GateDecision::Block
        } else if policy.accept_breaking {
            GateDecision::Proceed
        } else if policy.interactive {
            GateDecision::Prompt
        } else {
            GateDecision::Block
        }
    }

    /// The text `generate` prints: the breaking changes, the possibly breaking ones (told apart,
    /// because the tool cannot say which way they go), then a count of the other changes. Empty
    /// when nothing changed.
    #[must_use]
    pub fn report(&self) -> String {
        let mut out = String::new();
        let breaking = self.breaking();
        if !breaking.is_empty() {
            out.push_str(&format!("breaking changes ({}):\n", breaking.len()));
            for c in &breaking {
                out.push_str(&format!("  - {}: {}\n", c.subject, c.message));
            }
        }
        let possible = self.possibly_breaking();
        if !possible.is_empty() {
            out.push_str(&format!(
                "possibly breaking changes ({}); the tool cannot tell whether they break existing code (a rule that changed may be stricter or looser):\n",
                possible.len()
            ));
            for c in &possible {
                out.push_str(&format!("  - {}: {}\n", c.subject, c.message));
            }
        }
        let other = self.non_breaking().len();
        if other > 0 {
            out.push_str(&format!("{other} non-breaking change(s)\n"));
        }
        out
    }
}

/// Compares the baseline `old` with the contract `new` that is about to be generated.
#[must_use]
pub fn breaking_gate(old: &Ir, new: &Ir) -> GateResult {
    GateResult {
        changes: diff(old, new),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::ir;

    const BASE: &str = "model A { id: int }\nservice S at \"/s\" { get \"/\" f() -> A }\n";

    fn gate(new_src: &str) -> GateResult {
        breaking_gate(&ir(BASE), &ir(new_src))
    }

    #[test]
    fn identical_contracts_proceed_silently() {
        let g = gate(BASE);
        assert!(g.changes.is_empty());
        assert_eq!(g.decide(GatePolicy::default()), GateDecision::Proceed);
        assert_eq!(g.report(), "");
    }

    #[test]
    fn additions_do_not_block() {
        let g = gate(
            "model A { id: int\n extra?: string }\nservice S at \"/s\" { get \"/\" f() -> A }\n",
        );
        assert!(!g.has_breaking());
        assert_eq!(
            g.decide(GatePolicy {
                strict: true,
                ..GatePolicy::default()
            }),
            GateDecision::Proceed
        );
        assert!(g.report().contains("non-breaking"));
    }

    #[test]
    fn breaking_change_follows_the_policy() {
        let g = gate("model A { id: string }\nservice S at \"/s\" { get \"/\" f() -> A }\n");
        assert!(g.has_breaking());
        let p = GatePolicy::default();
        assert_eq!(g.decide(p), GateDecision::Block, "unattended runs block");
        assert_eq!(
            g.decide(GatePolicy {
                interactive: true,
                ..p
            }),
            GateDecision::Prompt
        );
        assert_eq!(
            g.decide(GatePolicy {
                accept_breaking: true,
                ..p
            }),
            GateDecision::Proceed
        );
        let strict = GatePolicy {
            strict: true,
            accept_breaking: true,
            interactive: true,
        };
        assert_eq!(g.decide(strict), GateDecision::Block, "strict wins");
        assert!(g.report().starts_with("breaking changes (1):"));
    }

    const RULED: &str = "model A { id: int\n rule positive: id > 0 }\nservice S at \"/s\" { post \"/\" f(body: A) -> 204 }\n";

    #[test]
    fn a_changed_rule_stops_generate_like_a_breaking_change() {
        let g = breaking_gate(&ir(RULED), &ir(&RULED.replace("id > 0", "id > 5")));
        assert!(g.breaking().is_empty(), "{:?}", g.changes);
        assert_eq!(g.possibly_breaking().len(), 1, "{:?}", g.changes);
        assert!(g.has_breaking());
        let p = GatePolicy::default();
        assert_eq!(g.decide(p), GateDecision::Block, "unattended runs block");
        let ask = GatePolicy {
            interactive: true,
            ..p
        };
        assert_eq!(g.decide(ask), GateDecision::Prompt);
        let accept = GatePolicy {
            accept_breaking: true,
            ..p
        };
        assert_eq!(g.decide(accept), GateDecision::Proceed);
        let strict = GatePolicy {
            strict: true,
            ..accept
        };
        assert_eq!(g.decide(strict), GateDecision::Block, "strict wins");
    }

    #[test]
    fn the_report_tells_a_possible_break_from_a_definite_one() {
        let g = breaking_gate(&ir(RULED), &ir(&RULED.replace("id > 0", "id > 5")));
        let text = g.report();
        assert!(text.starts_with("possibly breaking changes (1);"), "{text}");
        assert!(text.contains("cannot tell"), "{text}");
        assert!(!text.contains("\nbreaking changes"), "{text}");
        // Both kinds at once: the definite ones come first, each under its own heading.
        let both = breaking_gate(
            &ir(RULED),
            &ir(&RULED
                .replace("id > 0", "id > 5")
                .replace("at \"/s\"", "at \"/t\"")),
        );
        let text = both.report();
        assert!(text.starts_with("breaking changes ("), "{text}");
        assert!(text.contains("\npossibly breaking changes ("), "{text}");
    }

    #[test]
    fn an_added_event_channel_publisher_or_subscriber_does_not_stop_generate() {
        let old = "event Paid { id: uuid }\nchannel C { carries Paid }\nservice P at \"/p\" {\n  publishes C\n  get \"/\" ping() -> 204\n}\n";
        let new = "event Paid { id: uuid }\nevent New { id: uuid }\n\
             channel C { carries Paid }\nchannel D { carries New }\n\
             service P at \"/p\" {\n  publishes C, D\n  get \"/\" ping() -> 204\n}\n\
             service S {\n  subscribes C as \"g\" {\n    on Paid -> a\n  }\n}\n";
        let g = breaking_gate(&ir(old), &ir(new));
        assert!(!g.changes.is_empty(), "the additions are reported");
        assert!(!g.has_breaking(), "{:?}", g.changes);
        assert_eq!(g.decide(GatePolicy::default()), GateDecision::Proceed);
        let strict = GatePolicy {
            strict: true,
            ..GatePolicy::default()
        };
        assert_eq!(g.decide(strict), GateDecision::Proceed);
    }

    #[test]
    fn a_changed_event_stops_generate() {
        let old = "event Paid { id: uuid }\nchannel C { carries Paid }\n";
        let new = "event Paid { id: uuid\n note?: string }\nchannel C { carries Paid }\n";
        let g = breaking_gate(&ir(old), &ir(new));
        assert_eq!(g.possibly_breaking().len(), 1, "{:?}", g.changes);
        assert_eq!(g.decide(GatePolicy::default()), GateDecision::Block);
    }
}

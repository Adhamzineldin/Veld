//! `--prove` of `veld check` and `veld diff` (`spec/rules.md` section 18): the glue between the
//! command line, `veld-prove` and the output.
//!
//! The report says what the rules of the contract allow, over the fragment of section 18.3. It
//! never calls an application verified or correct.

use std::collections::BTreeMap;
use std::time::Duration;

use veld_ir::{Ir, Origin};
use veld_prove::{Changed, ChangedRule, Options, Outcome, Report, Solver, find_in_environment};
use veld_tools::{Change, RuleSite, Severity};

use crate::cli::ProveArgs;
use crate::pipeline::{self, Scope};
use crate::ui::Ui;

/// The default time limit of a question, in seconds.
const DEFAULT_SECONDS: u64 = 5;

fn options(args: &ProveArgs) -> Options {
    Options {
        timeout: Duration::from_secs(args.prove_timeout.unwrap_or(DEFAULT_SECONDS).max(1)),
    }
}

/// The solver, or one line that says there is none.
fn solver(ui: &Ui, args: &ProveArgs) -> Option<Solver> {
    match find_in_environment(args.solver.as_deref()) {
        Ok(s) => Some(s),
        Err(e) => {
            ui.warn(&format!("--prove: {e}; the questions were not asked"));
            None
        }
    }
}

fn shown_file(packages: &BTreeMap<String, String>, origin: &Origin) -> String {
    match packages.get(&origin.file) {
        Some(pkg) => format!("{pkg}/{}", origin.file),
        None => origin.file.clone(),
    }
}

/// The warning about undecided questions, and the lines that close the report.
pub fn print_report(ui: &Ui, report: &Report) {
    if let Some(text) = report.undecided_text() {
        ui.diagnostic(&text);
    }
    report.closing_lines().iter().for_each(|l| ui.info(l));
}

/// `veld check --prove`: the three questions about every contract. Prints the findings and
/// returns how many `E0970` there are.
pub fn check(
    ui: &Ui,
    project: &veld_project::config::Project,
    ir: &Ir,
    scope: Option<&Scope>,
    args: &ProveArgs,
) -> usize {
    let Some(solver) = solver(ui, args) else {
        return 0;
    };
    let report = veld_prove::check(ir, &solver, &options(args));
    let packages = pipeline::file_packages(project);
    let mut errors = 0;
    for item in report.findings() {
        if scope.is_some_and(|s| !s.has_relative(&item.origin.file)) {
            continue;
        }
        ui.diagnostic(&item.finding_text(&|o| shown_file(&packages, o)));
        errors += 1;
    }
    print_report(ui, &report);
    errors
}

fn site_of<'a>(rule: &veld_tools::ChangedRule<'a>) -> Changed<'a> {
    match rule.site {
        RuleSite::Invariant { old, new } => Changed::Invariant { old, new },
        RuleSite::Require { old, new } => Changed::Require { old, new },
    }
}

/// The position of a change in `changes`: the "possibly breaking" entry of this rule.
fn position(changes: &[Change], rule: &veld_tools::ChangedRule<'_>) -> Option<usize> {
    let start = format!("possibly breaking: {}", rule.marker());
    changes.iter().position(|c| {
        c.kind == rule.kind
            && c.subject == rule.subject
            && c.severity == Severity::PossiblyBreaking
            && c.message.starts_with(&start)
    })
}

/// The words of a change up to what the tool could not tell.
fn head(message: &str) -> &str {
    let body = message
        .strip_prefix("possibly breaking: ")
        .unwrap_or(message);
    let cut = body
        .find("; it is checked on")
        .or_else(|| body.find("; the tool cannot tell"))
        .unwrap_or(body.len());
    &body[..cut]
}

/// What `diff` says once the solver has answered for a rule, given where the rule is checked.
/// A rule that got tighter is breaking where requests meet it; one that is proved not tighter
/// is only safe where nothing but requests meet it, because on responses a looser rule loses a
/// guarantee.
fn upgrade(change: &mut Change, request: bool, response: bool, outcome: &Outcome) {
    let head = head(&change.message).to_owned();
    match outcome {
        Outcome::Counterexample { value } => {
            let said = format!(
                "{head}; tightened: the old rule accepted {value} and the new rule rejects it \
                 (the reference evaluator confirmed this)"
            );
            if request {
                change.severity = Severity::Breaking;
                change.message = said;
            } else {
                change.severity = Severity::NonBreaking;
                change.message = format!("note: {said}; on responses this is a new guarantee");
            }
        }
        Outcome::Proved { .. } if !response => {
            change.severity = Severity::NonBreaking;
            change.message = format!(
                "note: {head}; not tightened: every value the old rule accepted is still \
                 accepted (proved over the fragment)"
            );
        }
        Outcome::Proved { .. } => {
            change.message = format!(
                "possibly breaking: {head}; not tightened (proved), but the new rule may accept \
                 more, which loses a guarantee on responses"
            );
        }
        Outcome::Unknown { reason } => {
            change
                .message
                .push_str(&format!("; --prove: unknown ({reason})"));
        }
        Outcome::Unsupported { reason } => {
            change
                .message
                .push_str(&format!("; --prove: unsupported ({reason})"));
        }
    }
}

/// `veld diff --prove`: asks whether each changed rule got tighter and rewrites its change.
pub fn diff(
    ui: &Ui,
    old: &Ir,
    new: &Ir,
    changes: &mut [Change],
    args: &ProveArgs,
) -> Option<Report> {
    let changed = veld_tools::changed_rules(old, new);
    if changed.is_empty() {
        return None;
    }
    let solver = solver(ui, args)?;
    let rules: Vec<ChangedRule<'_>> = changed
        .iter()
        .map(|c| ChangedRule {
            subject: c.subject.to_owned(),
            detail: c.name(),
            changed: site_of(c),
        })
        .collect();
    let report = veld_prove::prove_diff(&rules, &solver, &options(args));
    for (rule, item) in changed.iter().zip(&report.items) {
        if let Some(i) = position(changes, rule) {
            upgrade(&mut changes[i], rule.request, rule.response, &item.outcome);
        }
    }
    Some(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use veld_tools::ChangeKind;

    fn change() -> Change {
        Change {
            kind: ChangeKind::InvariantChanged,
            severity: Severity::PossiblyBreaking,
            subject: "demo:hello.M".into(),
            message: "possibly breaking: rule `r` changed from `a > 0` to `a > 5`; it is checked \
                      on requests, and the tool cannot tell whether the new rule is tighter or \
                      looser"
                .into(),
        }
    }

    fn after(request: bool, response: bool, outcome: &Outcome) -> Change {
        let mut c = change();
        upgrade(&mut c, request, response, outcome);
        c
    }

    fn found() -> Outcome {
        Outcome::Counterexample {
            value: "{\"a\":3}".into(),
        }
    }

    #[test]
    fn a_tightened_rule_breaks_requests_and_is_a_new_guarantee_on_responses() {
        let c = after(true, false, &found());
        assert_eq!(c.severity, Severity::Breaking);
        assert!(c.message.contains("tightened") && c.message.contains("{\"a\":3}"));
        assert!(
            c.message
                .starts_with("rule `r` changed from `a > 0` to `a > 5`")
        );
        let c = after(false, true, &found());
        assert_eq!(c.severity, Severity::NonBreaking);
        assert!(c.message.starts_with("note: "));
    }

    #[test]
    fn a_rule_proved_not_tightened_is_safe_only_where_requests_meet_it() {
        let proved = Outcome::Proved { core: Vec::new() };
        assert_eq!(after(true, false, &proved).severity, Severity::NonBreaking);
        // On a response a looser rule loses a guarantee: it stays possibly breaking.
        let c = after(true, true, &proved);
        assert_eq!(c.severity, Severity::PossiblyBreaking);
        assert!(c.message.contains("not tightened (proved)"));
    }

    #[test]
    fn an_undecided_rule_stays_possibly_breaking_and_says_why() {
        let unknown = Outcome::Unknown {
            reason: "the solver gave up".into(),
        };
        let c = after(true, false, &unknown);
        assert_eq!(c.severity, Severity::PossiblyBreaking);
        assert!(
            c.message
                .ends_with("; --prove: unknown (the solver gave up)")
        );
        let c = after(
            true,
            false,
            &Outcome::Unsupported {
                reason: "float".into(),
            },
        );
        assert!(c.message.ends_with("; --prove: unsupported (float)"));
    }
}

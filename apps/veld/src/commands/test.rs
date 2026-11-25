//! `veld test`: runs the `example` blocks of the contracts (`spec/rules.md` section 12.3).
//!
//! Offline, every example runs against the reference evaluator. With `--live <base-url>` each
//! one is sent to a running test handler and its answer is compared with the expected outcome.

use std::path::Path;

use anyhow::Result;
use veld_ir::{Example, Ir};
use veld_rules::{ExampleReport, Verdict};
use veld_testing::{Live, LiveError};

use super::test_fuzz;
use crate::cli::TestArgs;
use crate::error::{Exit, user};
use crate::pipeline;
use crate::ui::render;
use crate::ui::{Style, Ui};

/// What running one example came to.
enum Done {
    Report(Box<ExampleReport>),
    /// The example could not be run: it does not fit the contract, or cannot be filled in.
    Broken(String),
    Skipped(String),
}

#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    passed: usize,
    skipped: usize,
    failed: usize,
}

pub fn run(ui: &Ui, config: Option<&Path>, args: &TestArgs) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    let detail = match &args.live {
        Some(url) => format!("{} (live {url})", project.name),
        None => project.name.clone(),
    };
    ui.header("test", &detail);
    let scope = match args.paths.is_empty() {
        true => None,
        false => Some(pipeline::scope_of(&project, &args.paths)?),
    };
    let Some(done) = pipeline::compile_or_report(ui, project)? else {
        return Ok(Exit::User);
    };
    let examples: Vec<&Example> = veld_testing::select(&done.ir, args.filter.as_deref())
        .into_iter()
        .filter(|e| {
            scope
                .as_ref()
                .is_none_or(|s| s.has_relative(&e.origin.file))
        })
        .collect();
    let no_examples =
        "no examples to run (write `example \"...\" { ... }` blocks in your contracts)";
    if examples.is_empty() && args.fuzz.is_none() {
        ui.info(no_examples);
        return Ok(Exit::Ok);
    }
    let live = match &args.live {
        Some(url) => Some(Live::connect(url, args.dry_run).map_err(|e| user(e.to_string()))?),
        None => None,
    };
    let mut tally = Tally::default();
    for ex in &examples {
        let result = run_one(&done.ir, live.as_ref(), ex)?;
        print_result(ui, &ex.name, &result);
        tally.record(&result);
    }
    let examples_exit = if examples.is_empty() {
        ui.info(no_examples);
        Exit::Ok
    } else {
        summarize(ui, tally)
    };
    // Fuzzing runs after the examples; `--fuzz` requires `--live`, so `live` is there.
    let (Some(count), Some(live)) = (args.fuzz, live.as_ref()) else {
        return Ok(examples_exit);
    };
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    let fuzz_exit = test_fuzz::run(ui, &done.ir, live, count, args.seed)?;
    let ok = matches!((examples_exit, fuzz_exit), (Exit::Ok, Exit::Ok));
    Ok(if ok { Exit::Ok } else { Exit::User })
}

fn run_one(ir: &Ir, live: Option<&Live>, ex: &Example) -> Result<Done> {
    let Some(live) = live else {
        return Ok(match veld_rules::run_example(ir, ex) {
            Ok(report) => Done::Report(Box::new(report)),
            Err(e) => Done::Broken(format!("[{}] {e}", e.code())),
        });
    };
    match live.run_example(ir, ex) {
        Ok(report) => Ok(Done::Report(Box::new(report))),
        Err(LiveError::Example(e)) => Ok(Done::Broken(format!("[{}] {e}", e.code()))),
        Err(LiveError::Unsupported { reason, .. }) => Ok(Done::Skipped(reason)),
        Err(e @ (LiveError::Connect { .. } | LiveError::NotTestHandler { .. })) => {
            Err(user(e.to_string()))
        }
    }
}

impl Tally {
    fn record(&mut self, done: &Done) {
        match done {
            Done::Report(r) => match r.verdict {
                Verdict::Passed => self.passed += 1,
                Verdict::Skipped(_) => self.skipped += 1,
                Verdict::Failed(_) => self.failed += 1,
            },
            Done::Skipped(_) => self.skipped += 1,
            Done::Broken(_) => self.failed += 1,
        }
    }
}

/// One line per example: `ok`, `skip` with the reason, or `FAIL` with what differs. A failure
/// is a result, so `--quiet` does not hide it.
fn print_result(ui: &Ui, name: &str, done: &Done) {
    print_verdict(ui, name, done);
    if let Done::Report(r) = done {
        audited_lines(ui, r);
    }
}

/// Under an example: the audited rules (`@rollout(audit)`) it would violate. They do not fail it
/// (`spec/rules.md` section 17.1), so they are information, not a result.
fn audited_lines(ui: &Ui, r: &ExampleReport) {
    for v in &r.audited {
        let text = format!(
            "     audited {}: {} (at {})",
            v.rule_name, v.message, v.path
        );
        ui.info(&ui.out_styled(Style::Dim, &text));
    }
}

fn print_verdict(ui: &Ui, name: &str, done: &Done) {
    let verdict = match done {
        Done::Report(r) => &r.verdict,
        Done::Skipped(why) => return skip_line(ui, name, why),
        Done::Broken(why) => return fail_lines(ui, name, std::slice::from_ref(why)),
    };
    match verdict {
        Verdict::Passed => ui.info(&format!("{} {name}", ui.out_styled(Style::Green, "ok  "))),
        Verdict::Skipped(why) => skip_line(ui, name, why),
        Verdict::Failed(m) => {
            let mut lines = vec![format!("expected: {}", m.expected)];
            lines.push(format!("actual:   {}", m.actual));
            lines.extend(m.differences.iter().map(ToString::to_string));
            fail_lines(ui, name, &lines);
        }
    }
}

fn skip_line(ui: &Ui, name: &str, why: &str) {
    let text = ui.out_styled(Style::Dim, &format!("skip {name} ({why})"));
    ui.info(&text);
}

fn fail_lines(ui: &Ui, name: &str, lines: &[String]) {
    ui.data(&format!("{} {name}", ui.out_styled(Style::Red, "FAIL")));
    for line in lines {
        ui.data(&format!("     {line}"));
    }
}

fn summarize(ui: &Ui, t: Tally) -> Exit {
    let mut parts = vec![format!("{} passed", t.passed)];
    if t.skipped > 0 {
        parts.push(format!("{} skipped", t.skipped));
    }
    if t.failed > 0 {
        parts.insert(0, format!("{} failed", t.failed));
        eprintln!("{}", render::fail(&ui.theme_err(), &parts.join(", ")));
        return Exit::User;
    }
    ui.info(&render::ok(&ui.theme_out(), &parts.join(", ")));
    Exit::Ok
}

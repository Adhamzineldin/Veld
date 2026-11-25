//! `veld check`: compile everything, lint it and report.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use veld_tools::lint::{self, LintConfig, LintIssue, LintLevel};
use veld_tools::{DepsFormat, deps};

use crate::cli::{CheckArgs, GraphFormat};
use crate::error::{Exit, user};
use crate::pipeline::{self, Counts, Scope};
use crate::ui::render;
use crate::ui::{Style, Ui};

pub fn run(ui: &Ui, config: Option<&Path>, args: &CheckArgs) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    if args.graph.is_none() {
        ui.header("check", &project.name);
    }
    super::registry::auto_install(ui, &project)?;
    let scope = match args.paths.is_empty() {
        true => None,
        false => Some(pipeline::scope_of(&project, &args.paths)?),
    };
    let lint_config = LintConfig::from_levels(&project.lint).map_err(|e| user(e.to_string()))?;
    let started = Instant::now();
    let out = pipeline::compile_project(&project)?;
    ui.debug(
        1,
        &format!("compiled in {}", render::elapsed(started.elapsed())),
    );
    let (counts, hidden) = pipeline::report_diagnostics_in(ui, &project, &out, scope.as_ref());
    let Some(ir) = out.ir.filter(|_| counts.errors + hidden == 0) else {
        eprintln!("{}", pipeline::failure_summary(ui, counts));
        if hidden > 0 {
            ui.warn(&format!(
                "{} in other files are not shown (run `veld check` for all)",
                pipeline::plural(hidden, "error")
            ));
        }
        return Ok(if counts.errors > 0 {
            Exit::User
        } else {
            Exit::Ok
        });
    };
    if let Some(format) = args.graph {
        print!("{}", deps(&ir, deps_format(format)));
        return Ok(Exit::Ok);
    }
    let issues = match args.no_lint {
        true => Vec::new(),
        false => {
            let cfg = if args.strict {
                lint_config.strict()
            } else {
                lint_config
            };
            run_lint(ui, &project, &ir, &cfg, scope.as_ref())
        }
    };
    let example_errors = run_examples(ui, &project, &ir, scope.as_ref());
    summarize(
        ui,
        &ir,
        counts,
        &issues,
        example_errors,
        args.strict,
        started,
    )
}

fn deps_format(f: GraphFormat) -> DepsFormat {
    match f {
        GraphFormat::Text => DepsFormat::Text,
        GraphFormat::Dot => DepsFormat::Dot,
        GraphFormat::Json => DepsFormat::Json,
    }
}

fn run_lint(
    ui: &Ui,
    project: &veld_project::config::Project,
    ir: &veld_ir::Ir,
    cfg: &LintConfig,
    scope: Option<&Scope>,
) -> Vec<LintIssue> {
    let packages = pipeline::file_packages(project);
    let mut issues = lint::lint(ir, cfg);
    issues.retain(|i| scope.is_none_or(|s| s.has_relative(&i.origin.file)));
    for issue in &issues {
        if issue.level == LintLevel::Warn && ui.quiet {
            continue;
        }
        ui.diagnostic(&render_issue(issue, &packages));
    }
    issues
}

/// A lint finding in the shape of a compiler diagnostic.
pub fn render_issue(issue: &LintIssue, packages: &BTreeMap<String, String>) -> String {
    let severity = match issue.level {
        LintLevel::Error => "error",
        LintLevel::Warn | LintLevel::Off => "warning",
    };
    let file = &issue.origin.file;
    let shown = match packages.get(file) {
        Some(pkg) => format!("{pkg}/{file}"),
        None => file.clone(),
    };
    format!(
        "{severity}[{}]: {}\n --> {shown}:{}:{}\n  = note: {} (lint rule `{}`)\n",
        issue.code, issue.message, issue.origin.line, issue.origin.col, issue.subject, issue.rule
    )
}

/// `E0953`: an example whose left-out values cannot be filled in, or whose filled-in value
/// breaks an invariant (`spec/rules.md` section 12.2). Prints each one and returns how many.
fn run_examples(
    ui: &Ui,
    project: &veld_project::config::Project,
    ir: &veld_ir::Ir,
    scope: Option<&Scope>,
) -> usize {
    let packages = pipeline::file_packages(project);
    let mut count = 0;
    for ex in &ir.examples {
        if scope.is_some_and(|s| !s.has_relative(&ex.origin.file)) {
            continue;
        }
        let filled = if ex.event.is_some() {
            veld_rules::fill_event_example(ir, ex).map(|_| ())
        } else {
            veld_rules::fill_example(ir, ex).map(|_| ())
        };
        let Err(err) = filled else {
            continue;
        };
        if err.code() != "E0953" {
            continue;
        }
        count += 1;
        let file = &ex.origin.file;
        let shown = match packages.get(file) {
            Some(pkg) => format!("{pkg}/{file}"),
            None => file.clone(),
        };
        ui.diagnostic(&format!(
            "error[E0953]: {err}\n --> {shown}:{}:{}\n",
            ex.origin.line, ex.origin.col
        ));
    }
    count
}

fn summarize(
    ui: &Ui,
    ir: &veld_ir::Ir,
    counts: Counts,
    issues: &[LintIssue],
    example_errors: usize,
    strict: bool,
    started: Instant,
) -> Result<Exit> {
    let lint_errors = issues
        .iter()
        .filter(|i| i.level == LintLevel::Error)
        .count();
    let lint_warns = issues.len() - lint_errors;
    let failed = lint_errors > 0 || example_errors > 0 || (strict && counts.warnings > 0);
    if failed {
        let mut parts = Vec::new();
        if example_errors > 0 {
            parts.push(pipeline::plural(example_errors, "example error"));
        }
        if lint_errors > 0 {
            parts.push(pipeline::plural(lint_errors, "lint error"));
        }
        if strict && counts.warnings > 0 {
            parts.push(pipeline::plural(counts.warnings, "warning"));
        }
        eprintln!(
            "{}",
            render::fail(
                &ui.theme_err(),
                &format!("{} (check failed)", parts.join(", "))
            )
        );
        return Ok(Exit::User);
    }
    let lint_note = match lint_warns {
        0 => String::new(),
        n => format!(", {}", pipeline::plural(n, "lint warning")),
    };
    ui.info(&format!(
        "{}{lint_note} {}",
        pipeline::success_summary(ui, ir),
        ui.out_styled(
            Style::Dim,
            &format!("in {}", render::elapsed(started.elapsed()))
        )
    ));
    Ok(Exit::Ok)
}

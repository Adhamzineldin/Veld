//! `veld generate`: the full pipeline, for every target or one.

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use veld_codegen::FileSet;
use veld_project::config::{EmitterRef, TargetSpec};
use veld_project::output::{self, PlanAction, WriteOptions};
use veld_tools::{GateDecision, GatePolicy};

use crate::cli::GenerateArgs;
use crate::error::{Exit, user};
use crate::pipeline::{self, Compiled, Written};
use crate::ui::Ui;
use crate::ui::prompt::{self, Clack, Prompter};
use crate::ui::render::{self, TargetSummary};

pub fn run(ui: &Ui, config: Option<&Path>, args: &GenerateArgs) -> Result<Exit> {
    if args.diff && !args.read_only() {
        return Err(user("`--diff` needs `--check` or `--dry-run`"));
    }
    let project = pipeline::load_project(config)?;
    ui.header("generate", &project.name);
    super::registry::auto_install(ui, &project)?;
    generate(ui, project, args)
}

/// One full run over an already loaded project. Nothing is written unless every target
/// compiled, passed its capability check and generated, and no target has a conflict.
pub fn generate(
    ui: &Ui,
    project: veld_project::config::Project,
    args: &GenerateArgs,
) -> Result<Exit> {
    let started = Instant::now();
    pipeline::select_targets(&project, args.target.as_deref())?;
    let Some(done) = pipeline::compile_or_report(ui, project)? else {
        return Ok(Exit::User);
    };
    if let Some(exit) = breaking_gate(ui, &done, args)? {
        return Ok(exit);
    }
    let targets = pipeline::select_targets(&done.project, args.target.as_deref())?;
    let outputs = match build_outputs(ui, &done, &targets) {
        Ok(o) => o,
        Err(messages) => {
            messages.iter().for_each(|m| ui.error(m));
            return Ok(Exit::User);
        }
    };
    if args.check {
        return check_stale(ui, &outputs, args.diff);
    }
    write_and_finish(ui, &done, &targets, &outputs, args, started)
}

type Outputs<'a> = Vec<(&'a TargetSpec, FileSet)>;

fn build_outputs<'a>(
    ui: &Ui,
    done: &Compiled,
    targets: &[&'a TargetSpec],
) -> Result<Outputs<'a>, Vec<String>> {
    let mut outputs = Vec::new();
    let mut failures = Vec::new();
    for &target in targets {
        let t0 = Instant::now();
        let spin = ui.spinner(&format!("Generating {}", target.name));
        let result = pipeline::build_job(&done.project, &done.ir, target)
            .and_then(|job| pipeline::run_job(&done.ir, &job));
        spin.finish();
        match result {
            Ok(files) => {
                ui.debug(
                    1,
                    &format!(
                        "{}: {} generated in {}",
                        target.name,
                        pipeline::plural(files.len(), "file"),
                        render::elapsed(t0.elapsed())
                    ),
                );
                outputs.push((target, files));
            }
            Err(messages) => failures.extend(messages),
        }
    }
    if failures.is_empty() {
        Ok(outputs)
    } else {
        Err(failures)
    }
}

// ---------------------------------------------------------------------------
// Breaking-change gate
// ---------------------------------------------------------------------------

/// Compares the contracts with the lock before anything is written. `Some(exit)` stops the run.
fn breaking_gate(ui: &Ui, done: &Compiled, args: &GenerateArgs) -> Result<Option<Exit>> {
    let Some(old) = pipeline::read_lock(&done.project) else {
        return Ok(None);
    };
    let gate = veld_tools::breaking_gate(&old, &done.ir);
    if !gate.has_breaking() {
        return Ok(None);
    }
    eprint!("{}", gate.report());
    if args.read_only() {
        return Ok(None);
    }
    let policy = GatePolicy {
        strict: args.strict,
        accept_breaking: args.accept_breaking,
        interactive: Ui::interactive(),
    };
    match gate.decide(policy) {
        GateDecision::Proceed => Ok(None),
        GateDecision::Prompt => match Clack::default().confirm("Generate anyway?", false) {
            Ok(true) => Ok(None),
            Ok(false) => Ok(Some(Exit::User)),
            Err(e) if prompt::is_cancelled(&e) => Ok(Some(Exit::Cancelled)),
            Err(e) => Err(e),
        },
        GateDecision::Block => {
            ui.error("breaking changes since the last generate; nothing was written");
            eprintln!(
                "{}",
                render::hint(
                    &ui.theme_err(),
                    "pass --accept-breaking to continue, or revert the change"
                )
            );
            Ok(Some(Exit::User))
        }
    }
}

// ---------------------------------------------------------------------------
// --check
// ---------------------------------------------------------------------------

fn action_word(a: PlanAction) -> &'static str {
    match a {
        PlanAction::Create => "create",
        PlanAction::Update => "update",
        PlanAction::Delete => "delete",
        PlanAction::Conflict => "conflict",
        PlanAction::Modified => "modified",
        PlanAction::Unchanged => "unchanged",
    }
}

/// `generate --check`: writes nothing; exit 1 when any target's output on disk differs.
fn check_stale(ui: &Ui, outputs: &Outputs<'_>, show_diff: bool) -> Result<Exit> {
    let mut stale = 0;
    for (target, files) in outputs {
        let plan = output::check(&target.out, files)
            .map_err(|e| user(format!("target `{}`: {e}", target.name)))?;
        if !plan.is_stale() {
            continue;
        }
        stale += 1;
        ui.data(&format!(
            "{} ({})",
            target.name,
            pipeline::display_path(&target.out)
        ));
        for e in plan.stale_entries() {
            ui.data(&format!("  {} {}", action_word(e.action), e.path));
            if show_diff {
                print_diff(target, files, &e.path, e.action);
            }
        }
    }
    if stale == 0 {
        ui.ok("generated output is up to date");
        return Ok(Exit::Ok);
    }
    eprintln!(
        "{}",
        render::fail(
            &ui.theme_err(),
            &format!(
                "{} out of date; run `veld generate`",
                pipeline::plural(stale, "target")
            )
        )
    );
    Ok(Exit::User)
}

/// The unified diff of one path of a plan, to stdout.
fn print_diff(target: &TargetSpec, files: &FileSet, path: &str, action: PlanAction) {
    if !matches!(
        action,
        PlanAction::Create | PlanAction::Update | PlanAction::Delete | PlanAction::Modified
    ) {
        return;
    }
    let old = std::fs::read(target.out.join(path)).unwrap_or_default();
    let new = files.get(path).unwrap_or_default();
    let (Ok(old), Ok(new)) = (std::str::from_utf8(&old), std::str::from_utf8(new)) else {
        println!("Binary files a/{path} and b/{path} differ");
        return;
    };
    print!("{}", unified_diff(path, old, new));
}

/// A unified diff with three lines of context.
fn unified_diff(path: &str, old: &str, new: &str) -> String {
    similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{path}"), &format!("b/{path}"))
        .to_string()
}

// ---------------------------------------------------------------------------
// Write and finish
// ---------------------------------------------------------------------------

fn write_and_finish(
    ui: &Ui,
    done: &Compiled,
    targets: &[&TargetSpec],
    outputs: &Outputs<'_>,
    args: &GenerateArgs,
    started: Instant,
) -> Result<Exit> {
    let opts = WriteOptions {
        dry_run: args.dry_run,
        force: args.force,
    };
    let spin = ui.spinner("Writing files");
    let t0 = Instant::now();
    let written = pipeline::write_all(outputs, opts);
    spin.finish();
    let written = written?;
    ui.debug(1, &format!("wrote in {}", render::elapsed(t0.elapsed())));
    for w in &written {
        for e in &w.plan.entries {
            ui.debug(
                2,
                &format!("{}: {} {}", w.name, action_word(e.action), e.path),
            );
        }
    }
    if args.diff {
        for ((target, files), w) in outputs.iter().zip(&written) {
            for e in w.plan.stale_entries() {
                print_diff(target, files, &e.path, e.action);
            }
        }
    }
    let clean = print_results(ui, &written, args.dry_run, started);
    if !clean {
        return Ok(Exit::User);
    }
    if args.dry_run {
        return Ok(Exit::Ok);
    }
    if args.target.is_none() {
        pipeline::write_lock(&done.project, &done.ir)?;
    }
    if args.setup {
        super::setup::apply_to(ui, &done.project, targets, false)?;
    } else {
        hint_setup(ui, &done.project, targets);
    }
    super::hooks::post_generate(ui, &done.project)?;
    import_hints(ui, &done.project, targets);
    lint_hint(ui, done);
    Ok(Exit::Ok)
}

/// One line when generated targets are not wired into their app yet.
fn hint_setup(ui: &Ui, project: &veld_project::config::Project, targets: &[&TargetSpec]) {
    let n = crate::setup::unwired(project, targets);
    if n > 0 {
        ui.hint(&format!(
            "{n} target(s) are not wired into your app yet. Setup: run `veld setup`, then `npm install`"
        ));
    }
}

/// One line per target that says how to import what was generated.
fn import_hints(ui: &Ui, project: &veld_project::config::Project, targets: &[&TargetSpec]) {
    let lines: Vec<(String, String)> = targets
        .iter()
        .filter_map(|t| import_line(project, t).map(|l| (t.name.clone(), l)))
        .collect();
    if lines.is_empty() {
        return;
    }
    ui.info("");
    ui.info("Use it:");
    let width = lines
        .iter()
        .map(|(n, _)| n.chars().count())
        .max()
        .unwrap_or(0);
    for (name, line) in lines {
        ui.info(&format!("  {name:<width$} {line}"));
    }
}

/// How to import a target's output: an npm package for the TypeScript-based emitters, the base
/// package for Java. `None` for plugins (they know their own conventions).
pub fn import_line(project: &veld_project::config::Project, target: &TargetSpec) -> Option<String> {
    let EmitterRef::Builtin(id) = &target.emitter else {
        return None;
    };
    let option = |key: &str| {
        target
            .options
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    };
    match id.as_str() {
        "typescript" | "node" => {
            let pkg = option("package-name")
                .unwrap_or_else(|| pipeline::default_package_name(&target.name));
            Some(format!("import {{ ... }} from '{pkg}'"))
        }
        "java" => Some(format!(
            "import {}.*;",
            java_package(project, target, option("package"))
        )),
        _ => None,
    }
}

/// The base Java package: the `package` option, else `dev.veld.generated.<service package>`,
/// the same default the Java emitter uses.
pub(super) fn java_package(
    project: &veld_project::config::Project,
    target: &TargetSpec,
    option: Option<String>,
) -> String {
    if let Some(p) = option {
        return p;
    }
    let home = target
        .service
        .clone()
        .or_else(|| target.services.first().cloned())
        .or_else(|| project.services().next().map(|s| s.name.clone()))
        .unwrap_or_default();
    let segment: String = home
        .trim_start_matches('@')
        .replace('/', "_")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("dev.veld.generated.{segment}")
}

/// After a generate, one line when lint findings exist.
fn lint_hint(ui: &Ui, done: &Compiled) {
    let Ok(cfg) = veld_tools::lint::LintConfig::from_levels(&done.project.lint) else {
        return;
    };
    let n = veld_tools::lint::lint(&done.ir, &cfg).len();
    if n > 0 {
        ui.hint(&format!(
            "{}, run `veld check`",
            pipeline::plural(n, "lint finding")
        ));
    }
}

fn summary_of(w: &Written) -> TargetSummary {
    let [created, updated, deleted, unchanged] = pipeline::tally(&w.plan);
    TargetSummary {
        name: w.name.clone(),
        out: pipeline::display_path(&w.out),
        created,
        updated,
        deleted,
        unchanged,
    }
}

/// Prints every target's result and the closing line. Returns false if any target conflicted.
fn print_results(ui: &Ui, written: &[Written], dry_run: bool, started: Instant) -> bool {
    let t = ui.theme_out();
    let mut shown = Vec::new();
    let mut clean = true;
    for w in written {
        if !w.plan.conflicts().is_empty() {
            report_conflicts(ui, w);
            clean = false;
            continue;
        }
        let s = summary_of(w);
        ui.info(&render::target_line(&t, &s, dry_run));
        if dry_run {
            list_changes(ui, w);
        }
        shown.push(s);
    }
    if clean {
        ui.info(&render::generate_tail(
            &t,
            &shown,
            dry_run,
            Some(started.elapsed()),
        ));
    }
    clean
}

fn list_changes(ui: &Ui, w: &Written) {
    for e in &w.plan.entries {
        let mark = match e.action {
            PlanAction::Create => "+",
            PlanAction::Update => "~",
            PlanAction::Delete => "-",
            PlanAction::Unchanged | PlanAction::Conflict | PlanAction::Modified => continue,
        };
        ui.info(&format!("    {mark} {}", e.path));
    }
}

fn report_conflicts(ui: &Ui, w: &Written) {
    let out = pipeline::display_path(&w.out);
    let foreign = w.plan.paths_with(PlanAction::Conflict);
    let edited = w.plan.paths_with(PlanAction::Modified);
    if !foreign.is_empty() {
        ui.error(&format!(
            "target `{}`: {} file(s) in `{out}` already exist and were not created by Veld; nothing was written",
            w.name,
            foreign.len()
        ));
        for path in &foreign {
            eprintln!("  conflict: {path}");
        }
        eprintln!("  move them away, or re-run with --force to overwrite them");
    }
    if !edited.is_empty() {
        ui.error(&format!(
            "target `{}`: {} generated file(s) in `{out}` were edited since Veld wrote them; nothing was written",
            w.name,
            edited.len()
        ));
        for path in &edited {
            eprintln!("  you edited generated file: {path}");
        }
        eprintln!("  keep your edits elsewhere, or re-run with --force to overwrite them");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use veld_project::config::load_str;

    #[test]
    fn unified_diff_has_headers_and_hunks() {
        let d = unified_diff("a.ts", "one\ntwo\n", "one\n2\n");
        assert!(d.contains("--- a/a.ts") && d.contains("+++ b/a.ts"), "{d}");
        assert!(d.contains("-two") && d.contains("+2"), "{d}");
    }

    #[test]
    fn import_lines_per_emitter() {
        let p = load_str(
            &std::env::temp_dir(),
            "[project]\nname = \"s\"\n[[targets]]\nname = \"web\"\nrole = \"client\"\nemitter = \"typescript\"\nout = \"web\"\n[[targets]]\nname = \"api\"\nrole = \"server\"\nemitter = \"java\"\nservice = \"s\"\nout = \"srv\"\n[targets.options]\npackage = \"com.acme\"\n",
        )
        .unwrap();
        assert_eq!(
            import_line(&p, &p.targets[0]).as_deref(),
            Some("import { ... } from '@veld/web'")
        );
        assert_eq!(
            import_line(&p, &p.targets[1]).as_deref(),
            Some("import com.acme.*;")
        );
    }
}

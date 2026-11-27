//! Retention (`spec/platform.md` section 23, `spec/targets/typescript.md` section 5l).
//!
//! Veld deletes nothing: a server whose services reach a model with `@retain` gets
//! `retention.ts` with `retentionPlan` (data, from `Ir::retention_plan`), the interface
//! `RetentionJobs` the application implements and `run<Target>Retention(jobs, options)`, which asks
//! for each entry what is due. The file imports nothing, so a server without `@retain` keeps its
//! files and a client generates none of it.

use veld_codegen::runtime_config::pascal_target;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic, Profile};
use veld_emit_typescript::tree::Tree;
use veld_emit_typescript::util::{HEADER, add_file, ts_str};
use veld_ir::retention::RetentionEntry;

/// The module of the retention file, root-relative, without extension.
pub const MODULE: &str = "retention";

/// The largest number of milliseconds a TypeScript `number` holds exactly.
const MAX_SAFE_MS: i64 = 9_007_199_254_740_991;

/// Writes `retention.ts` for the plan of the tree's services. Returns the module path to export,
/// or nothing when the plan is empty.
///
/// # Errors
/// `E0802` when the target's name is unknown (`run<Target>Retention` needs it), `E0801` for a
/// retention longer than a `number` holds in milliseconds, and `E0805` when the file cannot be added.
pub fn emit(
    files: &mut FileSet,
    tree: &Tree<'_>,
    profile: &Profile,
) -> Result<Option<String>, Vec<GenDiagnostic>> {
    let plan = tree.ir.retention_plan(&tree.services);
    if plan.is_empty() {
        return Ok(None);
    }
    let Some(target) = profile.target_name() else {
        return Err(vec![GenDiagnostic::new(
            "E0802",
            "the target has models with `@retain` but its name is unknown (the core passes it as the reserved option `target-name`)",
        )]);
    };
    for entry in &plan {
        if entry.after_ms > MAX_SAFE_MS {
            return Err(vec![GenDiagnostic::new(
                "E0801",
                format!(
                    "the retention of `{}` is longer than the milliseconds a TypeScript `number` holds exactly",
                    entry.model
                ),
            )]);
        }
    }
    let mut w = CodeWriter::new();
    w.line(HEADER);
    w.blank();
    write_types(&mut w);
    w.blank();
    write_plan(&mut w, &plan);
    w.blank();
    write_runner(&mut w, &pascal_target(target));
    add_file(files, &format!("{MODULE}.ts"), w.finish())?;
    Ok(Some(MODULE.to_string()))
}

fn write_types(w: &mut CodeWriter) {
    w.line("/** One model with `@retain` (`spec/platform.md` section 23). */");
    w.open("export interface RetentionEntry");
    w.line("/** The qualified name of the model. */");
    w.line("readonly model: string;");
    w.line("/** How long a value is kept after `since`, in milliseconds. */");
    w.line("readonly afterMs: number;");
    w.line("/** The wire name of the `datetime` or `date` field the time counts from: a value without it is kept. */");
    w.line("readonly since: string;");
    w.line("readonly then: 'delete' | 'anonymize';");
    w.line("/** For `anonymize`: the wire names of the `@pii` fields; empty for `delete`. */");
    w.line("readonly fields: readonly string[];");
    w.close();
    w.blank();
    w.line("/** What the application does when a value is due. Each method answers how many values it changed. */");
    w.open("export interface RetentionJobs");
    w.line("/** Deletes the values of `model` whose `since` is before `before`. */");
    w.line("delete(model: string, before: Date): number | Promise<number>;");
    w.line("/** Anonymizes `fields` of the values of `model` whose `since` is before `before`. */");
    w.line(
        "anonymize(model: string, before: Date, fields: readonly string[]): number | Promise<number>;",
    );
    w.close();
    w.blank();
    w.open("export interface RetentionOptions");
    w.line("/** The instant the job runs at: `before` of each entry is this minus its `afterMs`. Default: the system clock. */");
    w.line("now?: Date;");
    w.close();
    w.blank();
    w.line("/** What one entry did: how many values it changed, or why it failed (and then `changed` is 0). */");
    w.open("export interface RetentionResult");
    w.line("readonly model: string;");
    w.line("readonly then: 'delete' | 'anonymize';");
    w.line("readonly before: Date;");
    w.line("readonly changed: number;");
    w.line("readonly error?: unknown;");
    w.close();
}

fn write_plan(w: &mut CodeWriter, plan: &[RetentionEntry]) {
    w.line("/** The models to keep for a limited time, sorted by name. Veld deletes nothing: `run<Target>Retention` asks the application to. */");
    w.open_with(
        "export const retentionPlan: readonly RetentionEntry[] =",
        " [",
    );
    for e in plan {
        let fields: Vec<String> = e.fields.iter().map(|f| ts_str(f)).collect();
        w.open_with("", "{");
        w.line(format!("model: {},", ts_str(&e.model)));
        w.line(format!("afterMs: {},", e.after_ms));
        w.line(format!("since: {},", ts_str(&e.since)));
        w.line(format!("then: {},", ts_str(e.then.word())));
        w.line(format!("fields: [{}],", fields.join(", ")));
        w.close_with("},");
    }
    w.close_with("];");
}

fn write_runner(w: &mut CodeWriter, target: &str) {
    w.line("/**");
    w.line(" * Asks `jobs` for what is due, once per entry of `retentionPlan`, in order, with `before` = now");
    w.line(" * minus the entry's `afterMs`. A job that throws or rejects does not stop the others: it is in the");
    w.line(" * report of its entry. Run it from a job of your own, on a schedule.");
    w.line(" */");
    w.open(format!(
        "export async function run{target}Retention(jobs: RetentionJobs, options: RetentionOptions = {{}}): Promise<RetentionResult[]>"
    ));
    w.line("const now = (options.now ?? new Date()).getTime();");
    w.line("const report: RetentionResult[] = [];");
    w.open("for (const entry of retentionPlan)");
    w.line("const before = new Date(now - entry.afterMs);");
    w.open("try");
    w.line("const changed = await (entry.then === 'delete' ? jobs.delete(entry.model, before) : jobs.anonymize(entry.model, before, entry.fields));");
    w.line("report.push({ model: entry.model, then: entry.then, before, changed });");
    w.close_with("} catch (error) {");
    w.indent();
    w.line("report.push({ model: entry.model, then: entry.then, before, changed: 0, error });");
    w.close();
    w.close();
    w.line("return report;");
    w.close();
}

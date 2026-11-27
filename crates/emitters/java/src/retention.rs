//! Retention (`spec/platform.md` section 23, `spec/targets/java.md` section 18.4).
//!
//! Veld deletes nothing: a server whose services reach a model with `@retain` gets the package
//! `<base>.retention` with `Retention.retentionPlan()` (data, from `Ir::retention_plan`), the
//! interface `RetentionJobs` the application implements and `Retention.run<Target>Retention(jobs,
//! options)`, which asks for each entry what is due. The classes import nothing but the JDK, so a
//! server without `@retain` keeps its files and a client generates none of it.

use veld_codegen::runtime_config::pascal_target;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic, Profile};
use veld_ir::retention::RetentionEntry as Entry;

use crate::tree::{Tree, java_path};
use crate::util::{HEADER, add_file, java_str};

/// The package of the retention classes, relative to the base package.
const PACKAGE: &str = "retention";

/// `RetentionEntry`: one line of the plan.
const ENTRY: &str = r#"import java.util.List;

/** One model with `@retain` (`spec/platform.md` section 23). */
@SuppressWarnings("deprecation")
public record RetentionEntry(String model, long afterMs, String since, Then then, List<String> fields) {
  /** What is done to a value that is due. */
  public enum Then {
    /** The value is deleted. */
    DELETE("delete"),
    /** The `@pii` fields of the value are anonymized. */
    ANONYMIZE("anonymize");

    private final String word;

    Then(String word) {
      this.word = word;
    }

    /** The word the contract writes: {@code delete} or {@code anonymize}. */
    public String word() {
      return word;
    }
  }

  /**
   * One model with {@code @retain}.
   *
   * @param model the qualified name of the model
   * @param afterMs how long a value is kept after {@code since}, in milliseconds
   * @param since the wire name of the {@code datetime} or {@code date} field the time counts from: a
   *     value without it is kept
   * @param then what is done to a value that is due
   * @param fields for {@code anonymize}: the wire names of the {@code @pii} fields; empty for {@code delete}
   */
  public RetentionEntry {
    fields = List.copyOf(fields);
  }
}
"#;

/// `RetentionJobs`: what the application does.
const JOBS: &str = r#"import java.time.Instant;
import java.util.List;

/** What the application does when a value is due. Each method answers how many values it changed. */
@SuppressWarnings("deprecation")
public interface RetentionJobs {
  /** Deletes the values of {@code model} whose {@code since} is before {@code before}. */
  long delete(String model, Instant before) throws Exception;

  /** Anonymizes {@code fields} of the values of {@code model} whose {@code since} is before {@code before}. */
  long anonymize(String model, Instant before, List<String> fields) throws Exception;
}
"#;

/// `RetentionOptions`: the clock of a run.
const OPTIONS: &str = r#"import java.time.Instant;

/**
 * The options of a run of retention.
 *
 * @param now the instant the job runs at: {@code before} of each entry is this minus its {@code
 *     afterMs}; {@code null} is the system clock
 */
@SuppressWarnings("deprecation")
public record RetentionOptions(Instant now) {
  /** The options of a run at the system clock. */
  public RetentionOptions() {
    this(null);
  }
}
"#;

/// `RetentionResult`: what one entry did.
const RESULT: &str = r#"import java.time.Instant;

/**
 * What one entry did.
 *
 * @param model the qualified name of the model
 * @param then what the entry does
 * @param before the instant a value had to be older than
 * @param changed how many values the job changed; 0 when it failed
 * @param error why the entry failed, or {@code null}
 */
@SuppressWarnings("deprecation")
public record RetentionResult(
    String model, RetentionEntry.Then then, Instant before, long changed, Throwable error) {}
"#;

/// One file of the package: the header, the package, then `body`.
fn file(package: &str, body: &str) -> String {
    format!("{HEADER}\npackage {package};\n\n{body}")
}

fn write_plan(w: &mut CodeWriter, plan: &[Entry]) {
    w.line("/** The models to keep for a limited time, sorted by name. Veld deletes nothing: `run<Target>Retention` asks the application to. */");
    w.open_with(
        "private static final java.util.List<RetentionEntry> PLAN =",
        " java.util.List.of(",
    );
    for (n, e) in plan.iter().enumerate() {
        let fields: Vec<String> = e.fields.iter().map(|f| java_str(f)).collect();
        let then = match e.then {
            veld_ir::retention::RetainThen::Delete => "DELETE",
            veld_ir::retention::RetainThen::Anonymize => "ANONYMIZE",
        };
        let comma = if n + 1 == plan.len() { "" } else { "," };
        w.line(format!(
            "new RetentionEntry({}, {}L, {}, RetentionEntry.Then.{then}, java.util.List.of({})){comma}",
            java_str(&e.model),
            e.after_ms,
            java_str(&e.since),
            fields.join(", ")
        ));
    }
    w.close_with(");");
}

fn write_runner(w: &mut CodeWriter, target: &str) {
    w.open("public static java.util.List<RetentionEntry> retentionPlan()");
    w.line("return PLAN;");
    w.close();
    w.blank();
    w.line("/** Asks {@code jobs} for what is due, at the system clock. */");
    w.open(format!(
        "public static java.util.List<RetentionResult> run{target}Retention(RetentionJobs jobs)"
    ));
    w.line(format!(
        "return run{target}Retention(jobs, new RetentionOptions());"
    ));
    w.close();
    w.blank();
    w.line("/**");
    w.line(" * Asks {@code jobs} for what is due, once per entry of {@link #retentionPlan()}, in order, with");
    w.line(" * {@code before} = now minus the entry's {@code afterMs}. A job that throws does not stop the");
    w.line(" * others: it is in the report of its entry. Run it from a job of your own, on a schedule.");
    w.line(" */");
    w.open(format!(
        "public static java.util.List<RetentionResult> run{target}Retention(RetentionJobs jobs, RetentionOptions options)"
    ));
    w.line(
        "java.time.Instant now = options.now() == null ? java.time.Instant.now() : options.now();",
    );
    w.line("java.util.List<RetentionResult> report = new java.util.ArrayList<>();");
    w.open("for (RetentionEntry entry : PLAN)");
    w.line("report.add(ask(jobs, entry, now));");
    w.close();
    w.line("return report;");
    w.close();
    w.blank();
    w.open("private static RetentionResult ask(RetentionJobs jobs, RetentionEntry entry, java.time.Instant now)");
    w.line("java.time.Instant before;");
    w.open("try");
    w.line("before = now.minusMillis(entry.afterMs());");
    w.close_with("} catch (RuntimeException e) {");
    w.indent();
    w.line(
        "return new RetentionResult(entry.model(), entry.then(), java.time.Instant.MIN, 0L, e);",
    );
    w.close();
    w.open("try");
    w.line("long changed = entry.then() == RetentionEntry.Then.DELETE");
    w.line("    ? jobs.delete(entry.model(), before)");
    w.line("    : jobs.anonymize(entry.model(), before, entry.fields());");
    w.line("return new RetentionResult(entry.model(), entry.then(), before, changed, null);");
    w.close_with("} catch (InterruptedException e) {");
    w.indent();
    w.line("Thread.currentThread().interrupt();");
    w.line("return new RetentionResult(entry.model(), entry.then(), before, 0L, e);");
    w.close_with("} catch (Exception | StackOverflowError e) {");
    w.indent();
    w.line("return new RetentionResult(entry.model(), entry.then(), before, 0L, e);");
    w.close();
    w.close();
}

/// Writes the retention classes for the plan of the tree's services. Nothing when the plan is
/// empty.
///
/// # Errors
/// `E0802` when the target's name is unknown (`run<Target>Retention` needs it) and `E0805` when a
/// file cannot be added.
pub fn emit(
    files: &mut FileSet,
    tree: &Tree<'_>,
    profile: &Profile,
) -> Result<(), Vec<GenDiagnostic>> {
    let plan = tree.ir.retention_plan(&tree.services);
    if plan.is_empty() {
        return Ok(());
    }
    let Some(target) = profile.target_name() else {
        return Err(vec![GenDiagnostic::new(
            "E0802",
            "the target has models with `@retain` but its name is unknown (the core passes it as the reserved option `target-name`)",
        )]);
    };
    let package = format!("{}.{PACKAGE}", tree.base);
    for (class, body) in [
        ("RetentionEntry", ENTRY),
        ("RetentionJobs", JOBS),
        ("RetentionOptions", OPTIONS),
        ("RetentionResult", RESULT),
    ] {
        add_file(files, &java_path(&package, class), file(&package, body))?;
    }
    let mut w = CodeWriter::new();
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open("public final class Retention");
    w.line("private Retention() {}");
    w.blank();
    write_plan(&mut w, &plan);
    w.blank();
    write_runner(&mut w, &pascal_target(target));
    w.close();
    let body = format!(
        "/** Retention (`spec/platform.md` section 23): the plan, and the runner that asks the application what is due. Veld deletes nothing. */\n{}",
        w.finish()
    );
    add_file(
        files,
        &java_path(&package, "Retention"),
        file(&package, &body),
    )?;
    Ok(())
}

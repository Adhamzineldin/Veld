//! Invariants: the check function of a declaration with rules, its rule ids and its public
//! surface (`spec/rules.md` section 8, `spec/targets/java.md` section 8).
//!
//! A model or an error with rules gets, next to its schema:
//!
//! * `RULE_<NAME>` constants with the rule ids;
//! * `checkOwn(value, request, path, out)`: its own rules, in list order, each through
//!   `Rules.run`, which catches the evaluation errors of one rule;
//! * `checkWire(json, request, path, out)`: what its schema calls for every object it visits;
//! * `check(value)` and `checkRequest(value)`: the full check of a value, nested values included.
//!
//! Nesting is the runtime's job: `RuleWalk` visits every object of a value by its schema.

use veld_codegen::{CodeWriter, GenDiagnostic, naming, print_expr};
use veld_ir::{Decl, Rule};

use crate::rule_expr::JavaExpr;
use crate::tree::FileCtx;
use crate::util::{java_str, unique};

/// Whether a declaration has invariants.
#[must_use]
pub fn has_rules(d: &Decl) -> bool {
    !d.rules().is_empty()
}

/// The `.checked(...)` suffix of the schema of a declaration with rules.
#[must_use]
pub fn schema_suffix(d: &Decl, class: &str) -> String {
    let checked = if has_rules(d) {
        format!(".checked({class}::checkWire)")
    } else {
        String::new()
    };
    format!("{checked}{}", crate::derived::schema_suffix(d, class))
}

/// The constant names of the rule ids, in rule order.
fn constants(rules: &[Rule]) -> Vec<String> {
    unique(
        rules
            .iter()
            .map(|r| {
                format!(
                    "RULE_{}",
                    naming::screaming_snake(&r.name).trim_start_matches('_')
                )
            })
            .collect(),
    )
}

/// The wire names of the fields a rule reads.
fn read_wires(d: &Decl, rule: &Rule) -> String {
    let names: Vec<String> = rule
        .reads
        .iter()
        .map(|name| {
            let wire = d
                .fields()
                .iter()
                .find(|f| &f.name == name)
                .map_or(name.as_str(), |f| f.wire.as_str());
            java_str(wire)
        })
        .collect();
    names.join(", ")
}

/// Writes the rule members of `d` into the body of its class. `record` is the Java type of the
/// value the rules read: the model itself, or `Data` for an error.
///
/// # Errors
/// `E0801` for a rule that reads something Java cannot express (`now`, a parameter, a fact).
pub fn write_members(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    record: &str,
) -> Result<(), Vec<GenDiagnostic>> {
    let rules = d.rules();
    let names = constants(rules);
    let (list, violation, json) = (fc.name("java.util.List"), fc.rt("Violation"), fc.rt("Json"));
    let walk = fc.rt("RuleWalk");
    for (rule, name) in rules.iter().zip(&names) {
        w.line(format!("/** The id of the rule `{}`. */", rule.name));
        w.line(format!(
            "public static final String {name} = {};",
            java_str(&rule.id)
        ));
    }
    w.blank();
    w.line("/** Appends the violations of this declaration's own rules on one value. */");
    w.open(format!(
        "static void checkOwn({record} v, boolean request, String path, {list}<{violation}> out)"
    ));
    let mut problems = Vec::new();
    for (rule, name) in rules.iter().zip(&names) {
        write_rule(w, fc, d, rule, name, &mut problems);
    }
    w.close();
    if !problems.is_empty() {
        return Err(problems);
    }
    w.blank();
    w.open(format!(
        "private static void checkWire({json} json, boolean request, String path, {list}<{violation}> out)"
    ));
    w.line(format!(
        "checkOwn({record}.fromWire({walk}.forDecoding(SCHEMA, json, request)), request, path, out);"
    ));
    w.close();
    w.blank();
    w.line("/** The violations of a value and of every value nested in it (spec/rules.md section 5.2). */");
    w.open(format!(
        "public static {list}<{violation}> check({record} value)"
    ));
    w.line(format!(
        "return {walk}.checkWire(SCHEMA, value.toWire(), false);"
    ));
    w.close();
    w.blank();
    w.line("/** The violations of a value in request position: rules that read a read-only field are skipped. */");
    w.open(format!(
        "public static {list}<{violation}> checkRequest({record} value)"
    ));
    w.line(format!(
        "return {walk}.checkWire(SCHEMA, value.toWire(), true);"
    ));
    w.close();
    Ok(())
}

fn write_rule(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    rule: &Rule,
    constant: &str,
    problems: &mut Vec<GenDiagnostic>,
) {
    let mut printer = JavaExpr::new(fc, d.id);
    let body = print_expr(&mut printer, &rule.expr);
    for what in printer.unsupported() {
        problems.push(GenDiagnostic {
            code: "E0801",
            message: format!(
                "rule `{}` of `{}` uses {what}, which generator `java` cannot enforce",
                rule.name, d.name
            ),
            origin: Some(rule.origin.clone()),
        });
    }
    let rules = fc.rt("Rules");
    let list = fc.name("java.util.List");
    let message = rule
        .message
        .clone()
        .unwrap_or_else(|| format!("violates rule {}", rule.name));
    let skipped = rule.reads_readonly;
    if skipped {
        w.open("if (!request)");
    }
    // An audited rule reports to the sink of a server and is skipped when there is none, so a
    // client never evaluates it (`spec/rules.md` section 17.1).
    let (call, sink) = if rule.is_audited() {
        (format!("{}.run(", fc.rt("Audit")), "")
    } else {
        (format!("{rules}.run("), "out, ")
    };
    w.line(call);
    w.indent().indent();
    w.line(format!(
        "{sink}{constant}, {}, path, {list}.of({}), {},",
        java_str(&rule.name),
        read_wires(d, rule),
        java_str(&message)
    ));
    w.line(format!("() -> {});", body.code));
    w.dedent().dedent();
    if skipped {
        w.close();
    }
}

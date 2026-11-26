//! Check functions: for every model or error with invariants, one plain function that evaluates
//! its rules in list order on one value (`spec/rules.md` section 8).
//!
//! Nesting is not generated: the declaration's schema node refers to this function, and the
//! runtime's schema walk (`checkRules` in `runtime/veld.ts`) calls it for every object it
//! visits, the object's own rules before its fields.

use veld_codegen::CodeWriter;
use veld_ir::{Decl, Rule};

use crate::expr::{print, wire_name};
use crate::tree::FileCtx;
use crate::util::{prop_key, ts_str, type_name};

/// Whether `d` carries invariants.
#[must_use]
pub fn has_rules(d: &Decl) -> bool {
    !d.rules().is_empty()
}

/// The name of the check function of `d`.
#[must_use]
pub fn fn_name(d: &Decl) -> String {
    format!("check{}Rules", type_name(d))
}

/// `{ a: 'id', b: 'id' }`: the rule ids by rule name.
#[must_use]
pub fn ids_value(d: &Decl) -> String {
    let items: Vec<String> = d
        .rules()
        .iter()
        .map(|r| format!("{}: {}", prop_key(&r.name), ts_str(&r.id)))
        .collect();
    format!("{{ {} }}", items.join(", "))
}

/// `{ readonly a: string; readonly b: string }`: the type of [`ids_value`].
#[must_use]
pub fn ids_type(d: &Decl) -> String {
    let items: Vec<String> = d
        .rules()
        .iter()
        .map(|r| format!("readonly {}: string", prop_key(&r.name)))
        .collect();
    format!("{{ {} }}", items.join("; "))
}

fn fields_literal(d: &Decl, rule: &Rule) -> String {
    let wires: Vec<String> = rule
        .reads
        .iter()
        .map(|name| ts_str(&wire_name(d.fields(), name)))
        .collect();
    format!("[{}]", wires.join(", "))
}

fn message(rule: &Rule) -> String {
    match &rule.message {
        Some(m) => ts_str(m),
        None => ts_str(&format!("violates rule {}", rule.name)),
    }
}

fn write_rule(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, rule: &Rule) {
    let ns = fc.rules_ns();
    let cond = print(fc.tree.ir, &ns, &rule.expr);
    let (id, fields) = (ts_str(&rule.id), fields_literal(d, rule));
    let guarded = rule.reads_readonly;
    if guarded {
        w.open("if (mode !== 'request')");
    }
    // An audited rule reports to the sink of a server and is skipped when there is none, so a
    // client never evaluates it (`spec/rules.md` section 17.1).
    let sink = if rule.is_audited() {
        let rollout = fc.rollout_ns();
        w.open_with("", "{");
        w.line(format!("const audit = {rollout}.auditSink();"));
        w.open("if (audit !== null)");
        "audit"
    } else {
        "out"
    };
    w.open("try");
    w.open(format!("if (!({cond}))"));
    w.line(format!(
        "{sink}.push({{ rule: {id}, path, fields: {fields}, message: {} }});",
        message(rule)
    ));
    w.close();
    w.close_with("} catch (e) {");
    w.indent();
    // A rule of a value that has derived fields (or reaches one) meets an absent one whenever a
    // derived field could not be computed: the cause is reported already, so the rule is skipped
    // in every mode (`spec/rules.md` section 17.2).
    let mode = if fc.tree.ir.type_reaches_derived(&veld_ir::Type::Decl(d.id)) {
        format!("e instanceof {ns}.AbsentField ? 'partial' : mode")
    } else {
        "mode".to_string()
    };
    w.line(format!(
        "{ns}.failed(e, {mode}, {sink}, {id}, {}, path, {fields});",
        ts_str(&rule.name)
    ));
    w.close();
    if rule.is_audited() {
        w.close();
        w.close();
    }
    if guarded {
        w.close();
    }
}

/// Writes the check function of `d`. `self_ty` is the TypeScript type of the value it checks.
pub fn write_check_fn(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, self_ty: &str) {
    let violation = fc.rt("Violation");
    let mode = fc.rt("RuleMode");
    w.open(format!(
        "function {}(self: {self_ty}, path: string, mode: {mode}, out: {violation}[]): void",
        fn_name(d)
    ));
    for rule in d.rules() {
        write_rule(w, fc, d, rule);
    }
    w.close();
}

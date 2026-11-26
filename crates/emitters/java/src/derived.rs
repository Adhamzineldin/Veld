//! Derived fields (`spec/rules.md` section 17.2, `spec/targets/java.md` section 15): a field whose
//! `=` is followed by an expression is computed by the server, in dependency order, after the
//! defaults and before the invariants.
//!
//! A model, an error or an event with derived fields gets, next to its schema:
//!
//! * `DERIVED_<NAME>` constants with the field ids (embedded verbatim, never computed here);
//! * `deriveField<Name>(v, path, out)`, one private method per derived field: the expression
//!   printed by the same printer as an invariant, as the wire value it computes, run through
//!   `Derive.run` (an evaluation error is one violation, a read of an absent value is skipped);
//! * `deriveWire(json, request, path, out)`, registered on the schema with `.derived(...)`: the
//!   fields in the order of `Decl::derived_fields`, each result put into the object, and the value
//!   decoded again for the fields that follow;
//! * `derive(value)` and `derive(value, out)`, the public function: the code the server runs.
//!
//! Nesting, dropping what was supplied and the walk are the runtime's (`Derive`).

use veld_codegen::{CodeWriter, GenDiagnostic, naming};
use veld_ir::rules::{Expr, ExprKind, ExprType, PathRef, Root};
use veld_ir::{Decl, DeclId, Field, Ir, Type};

use crate::rule_expr::JavaExpr;
use crate::tree::FileCtx;
use crate::types::codec_expr;
use crate::util::{java_str, unique};

/// Whether a declaration has derived fields.
#[must_use]
pub fn has_derived(d: &Decl) -> bool {
    d.has_derived()
}

/// The `.derived(...)` suffix of the schema of a declaration with derived fields.
#[must_use]
pub fn schema_suffix(d: &Decl, class: &str) -> String {
    if !has_derived(d) {
        return String::new();
    }
    let wires: Vec<String> = d
        .fields()
        .iter()
        .filter(|f| f.derived.is_some())
        .map(|f| java_str(&f.wire))
        .collect();
    format!(".derived({class}::deriveWire, {})", wires.join(", "))
}

/// The names the members of one derived field get.
struct Names {
    constant: String,
    method: String,
}

fn names_of(fields: &[&Field]) -> Vec<Names> {
    let constants = unique(
        fields
            .iter()
            .map(|f| {
                format!(
                    "DERIVED_{}",
                    naming::screaming_snake(&f.name).trim_start_matches('_')
                )
            })
            .collect(),
    );
    let methods = unique(
        fields
            .iter()
            .map(|f| format!("deriveField{}", naming::pascal(&f.name)))
            .collect(),
    );
    constants
        .into_iter()
        .zip(methods)
        .map(|(constant, method)| Names { constant, method })
        .collect()
}

/// The wire names of the fields a derived expression reads, as a Java `List.of(...)`.
fn read_wires(d: &Decl, f: &Field) -> String {
    let names: Vec<String> = f
        .derived_reads()
        .iter()
        .map(|name| {
            let wire = d
                .fields()
                .iter()
                .find(|g| &g.name == name)
                .map_or(name.as_str(), |g| g.wire.as_str());
            java_str(wire)
        })
        .collect();
    names.join(", ")
}

/// Whether the expression reads a `@readonly` field, of its own declaration or at any depth
/// (`obj.field`, the variable of a lambda over a list): such a field is absent from a request body
/// (`spec/rules.md` 17.2), so the derived field is left absent in request position.
fn reads_readonly(ir: &Ir, d: &Decl, f: &Field) -> bool {
    f.derived
        .as_ref()
        .is_some_and(|e| expr_reads_readonly(ir, e, d.id, &mut Vec::new()))
}

/// The declaration a type leads to through lists, maps and pages (a lambda variable is an element).
fn element_decl(t: &Type) -> Option<DeclId> {
    match t {
        Type::Decl(id) => Some(*id),
        Type::List(x) | Type::Map(x) | Type::Page(x) => element_decl(x),
        Type::Scalar(_) => None,
    }
}

fn path_reads_readonly(ir: &Ir, p: &PathRef, start: Option<DeclId>) -> bool {
    let mut at = start;
    for step in &p.steps {
        let Some(id) = at else { return false };
        let Some(field) = ir.decl(id).fields().iter().find(|g| g.name == step.name) else {
            return false;
        };
        if field.server_set() {
            return true;
        }
        at = element_decl(&field.ty);
    }
    false
}

fn expr_reads_readonly(
    ir: &Ir,
    e: &Expr,
    own: DeclId,
    vars: &mut Vec<(String, Option<DeclId>)>,
) -> bool {
    match &e.e {
        ExprKind::Lit(_) | ExprKind::Now(_) => false,
        ExprKind::Path(p) | ExprKind::Has(p) => {
            let start = match &p.root {
                Root::SelfValue => Some(own),
                Root::Var(name) => vars
                    .iter()
                    .rev()
                    .find(|(n, _)| n == name)
                    .and_then(|(_, t)| *t),
                Root::Principal | Root::Result | Root::Param(_) | Root::Fact(_) => None,
            };
            path_reads_readonly(ir, p, start)
        }
        ExprKind::Not(x) | ExprKind::Neg(x) | ExprKind::Widen(x) | ExprKind::Old(x) => {
            expr_reads_readonly(ir, x, own, vars)
        }
        ExprKind::Bin(b) => {
            expr_reads_readonly(ir, &b.l, own, vars) || expr_reads_readonly(ir, &b.r, own, vars)
        }
        ExprKind::Call(c) => c.args.iter().any(|a| expr_reads_readonly(ir, a, own, vars)),
        ExprKind::List(items) => items.iter().any(|a| expr_reads_readonly(ir, a, own, vars)),
        ExprKind::Quant(q) => {
            if expr_reads_readonly(ir, &q.list, own, vars) {
                return true;
            }
            let element = match &q.list.t {
                ExprType::Type(t) => element_decl(t),
                ExprType::Duration => None,
            };
            vars.push((q.var.clone(), element));
            let found = expr_reads_readonly(ir, &q.body, own, vars);
            vars.pop();
            found
        }
    }
}

/// Writes the derived members of `d` into the body of its class. `record` is the Java type of the
/// value the expressions read: the model itself, or `Data` for an error.
///
/// # Errors
/// `E0801` for an expression that reads something Java cannot express, or a derived `duration`
/// (which has no Java type).
pub fn write_members(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    record: &str,
) -> Result<(), Vec<GenDiagnostic>> {
    let ordered = d.derived_fields();
    let names = names_of(&ordered);
    let mut problems = Vec::new();
    for (f, n) in ordered.iter().zip(&names) {
        w.line(format!("/** The id of the derived field `{}`. */", f.name));
        w.line(format!(
            "public static final String {} = {};",
            n.constant,
            java_str(f.derived_id.as_deref().unwrap_or_default())
        ));
    }
    w.blank();
    write_wire_function(w, fc, d, record, &ordered, &names);
    for (f, n) in ordered.iter().zip(&names) {
        w.blank();
        write_field_function(w, fc, d, record, (f, n), &mut problems);
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    w.blank();
    write_public(w, fc, record);
    Ok(())
}

/// `deriveWire`: every derived field in order, each put into the object.
fn write_wire_function(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    record: &str,
    ordered: &[&Field],
    names: &[Names],
) {
    let (json, walk) = (fc.rt("Json"), fc.rt("RuleWalk"));
    let (list, violation) = (fc.name("java.util.List"), fc.rt("Violation"));
    let (map, linked) = (fc.name("java.util.Map"), fc.name("java.util.LinkedHashMap"));
    w.line("/** Computes the derived fields of one object, in dependency order (`Derive` calls it). */");
    w.open(format!(
        "private static {json}.Obj deriveWire({json}.Obj json, boolean request, String path, {list}<{violation}> out)"
    ));
    w.line(format!(
        "{map}<String, {json}> members = new {linked}<>(json.fields());"
    ));
    w.line(format!(
        "{record} v = {record}.fromWire({walk}.forDecoding(SCHEMA, json, request));"
    ));
    w.line(format!("{json} x;"));
    for (i, (f, n)) in ordered.iter().zip(names).enumerate() {
        let call = format!("{}(v, path, out)", n.method);
        let call = if reads_readonly(fc.tree.ir, d, f) {
            format!("request ? null : {call}")
        } else {
            call
        };
        w.line(format!("x = {call};"));
        if i + 1 == ordered.len() {
            w.line(format!(
                "if (x != null) {{ members.put({}, x); }}",
                java_str(&f.wire)
            ));
        } else {
            w.open("if (x != null)");
            w.line(format!("members.put({}, x);", java_str(&f.wire)));
            w.line(format!(
                "v = {record}.fromWire({walk}.forDecoding(SCHEMA, new {json}.Obj(members), request));"
            ));
            w.close();
        }
    }
    w.line(format!("return new {json}.Obj(members);"));
    w.close();
}

/// `deriveField<Name>`: the expression of one derived field as the wire value it computes.
fn write_field_function(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    record: &str,
    (f, n): (&Field, &Names),
    problems: &mut Vec<GenDiagnostic>,
) {
    let Some(expr) = &f.derived else {
        return;
    };
    let (json, derive) = (fc.rt("Json"), fc.rt("Derive"));
    let (list, violation) = (fc.name("java.util.List"), fc.rt("Violation"));
    let codec = codec_expr(fc, &f.ty);
    let (value, unsupported) = {
        let mut printer = JavaExpr::new(fc, d.id);
        let value = printer.app_value(expr);
        (value, printer.unsupported().to_vec())
    };
    for what in unsupported {
        problems.push(GenDiagnostic {
            code: "E0801",
            message: format!(
                "derived field `{}` of `{}` uses {what}, which generator `java` cannot enforce",
                f.name, d.name
            ),
            origin: Some(d.origin.clone()),
        });
    }
    let value = value.unwrap_or_else(|| {
        problems.push(GenDiagnostic {
            code: "E0801",
            message: format!(
                "derived field `{}` of `{}` is a duration, which generator `java` cannot represent",
                f.name, d.name
            ),
            origin: Some(d.origin.clone()),
        });
        "null".to_string()
    });
    w.open(format!(
        "private static {json} {}({record} v, String path, {list}<{violation}> out)",
        n.method
    ));
    w.line(format!(
        "return {derive}.run(out, {}, {}, path, {list}.of({}),",
        n.constant,
        java_str(&f.name),
        read_wires(d, f)
    ));
    w.indent().indent();
    w.line(format!("() -> {codec}.encode({value}));"));
    w.dedent().dedent();
    w.close();
}

/// `derive(value)` and `derive(value, out)`: the public function that computes the derived fields
/// of a value, nested values included. It is the code the server runs, so a form can show `total`
/// while the user types.
fn write_public(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, record: &str) {
    let (derive, codec) = (fc.rt("Derive"), fc.rt("Codec"));
    let (list, violation) = (fc.name("java.util.List"), fc.rt("Violation"));
    w.line("/**");
    w.line(" * The value with its defaults applied and its derived fields computed (spec/rules.md section 17.2):");
    w.line(" * the code the server runs, so a form can show a derived field while the user types. A field");
    w.line(" * whose expression fails stays {@code null}; what the value held for a derived field is replaced.");
    w.line(" */");
    w.open(format!("public static {record} derive({record} value)"));
    w.line(format!(
        "return {derive}.value(SCHEMA, {codec}.<{record}>of({record}::fromWire, {record}::toWire), value);"
    ));
    w.close();
    w.blank();
    w.line("/** Like {@link #derive}, with the violation of every field that could not be computed in {@code out}. */");
    w.open(format!(
        "public static {record} derive({record} value, {list}<{violation}> out)"
    ));
    w.line(format!(
        "return {derive}.value(SCHEMA, {codec}.<{record}>of({record}::fromWire, {record}::toWire), value, out);"
    ));
    w.close();
}

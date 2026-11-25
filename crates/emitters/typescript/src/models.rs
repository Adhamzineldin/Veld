//! Models, enums, unions, consts, errors and the prelude.
//!
//! Output is identical in the client and server roles.

use std::collections::BTreeMap;

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{Const, Decl, DeclId, DeclKind, Enum, ErrorDecl, Model, Union};

use crate::checks;
use crate::tree::{FileCtx, Tree};
use crate::types::ts_type;
use crate::util::{add_file, doc_comment, literal, prop_key, ts_str, type_name};
use crate::validate::{schema_expr, write_object_schema};

/// Emits every declaration module of `tree` and returns the module paths written
/// (root-relative, without extension), for barrels.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let mut groups: BTreeMap<String, Vec<DeclId>> = BTreeMap::new();
    for &id in tree.decls() {
        groups
            .entry(tree.decl_module(tree.ir.decl(id)))
            .or_default()
            .push(id);
    }
    let mut modules = Vec::new();
    for (module, ids) in groups {
        let text = render_module(tree, &module, &ids);
        add_file(files, &format!("{module}.ts"), text)?;
        modules.push(module);
    }
    Ok(modules)
}

fn render_module(tree: &Tree<'_>, module: &str, ids: &[DeclId]) -> String {
    let mut fc = FileCtx::new(tree, module);
    let mut w = CodeWriter::new();
    let (errors, others): (Vec<DeclId>, Vec<DeclId>) =
        ids.iter().partition(|id| tree.ir.decl(**id).is_error());
    for id in others.into_iter().chain(errors) {
        write_decl(&mut w, &mut fc, tree.ir.decl(id));
        w.blank();
    }
    fc.finish(&w.finish())
}

fn write_decl(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl) {
    match &d.kind {
        DeclKind::Model(m) => write_model(w, fc, d, m),
        DeclKind::Enum(e) => write_enum(w, fc, d, e),
        DeclKind::Union(u) => write_union(w, fc, d, u),
        DeclKind::Const(c) => write_const(w, fc, d, c),
        DeclKind::Error(e) => write_error(w, fc, d, e),
    }
}

fn write_fields(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    fields: &[veld_ir::Field],
    own_only: bool,
) {
    for f in fields
        .iter()
        .filter(|f| !own_only || f.inherited_from.is_none())
    {
        doc_comment(w, f.docs.as_deref(), f.deprecated.as_deref());
        let ro = if f.readonly { "readonly " } else { "" };
        let opt = if f.optional { "?" } else { "" };
        let ty = ts_type(fc, &f.ty);
        w.line(format!("{ro}{}{opt}: {ty};", prop_key(&f.name)));
    }
}

fn write_model(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, m: &Model) {
    let name = type_name(d);
    doc_comment(w, d.docs.as_deref(), d.deprecated.as_deref());
    let ext = m
        .parent
        .map(|p| format!(" extends {}", fc.type_ref(p)))
        .unwrap_or_default();
    if m.fields.iter().all(|f| f.inherited_from.is_some()) {
        w.line(format!("export interface {name}{ext} {{}}"));
    } else {
        w.open(format!("export interface {name}{ext}"));
        write_fields(w, fc, &m.fields, true);
        w.close();
    }
    w.blank();
    if checks::has_rules(d) {
        write_ruled_companion(w, fc, d, m);
        return;
    }
    let companion = fc.rt("Companion");
    let make = fc.rt("companion");
    w.open_with(
        format!(
            "export const {name}: {companion}<{name}> = {make}<{name}>({},",
            ts_str(&d.sig)
        ),
        " {",
    );
    write_object_schema(w, fc, &m.fields);
    w.close_with("});");
}

/// The companion of a model with invariants: the plain companion plus `ruleIds`, and a schema
/// that carries the check function.
fn write_ruled_companion(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, m: &Model) {
    let name = type_name(d);
    checks::write_check_fn(w, fc, d, &name);
    w.blank();
    let companion = fc.rt("Companion");
    let make = fc.rt("companion");
    let ids = checks::ids_type(d);
    w.open_with(
        format!("export const {name}: {companion}<{name}> & {{ readonly ruleIds: {ids} }} ="),
        " {",
    );
    w.open_with(format!("...{make}<{name}>({},", ts_str(&d.sig)), " {");
    write_object_schema(w, fc, &m.fields);
    w.line(format!("rules: {},", checks::fn_name(d)));
    w.close_with("}),");
    w.line(format!("ruleIds: {},", checks::ids_value(d)));
    w.close_with("};");
}

fn write_enum(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, e: &Enum) {
    let name = type_name(d);
    doc_comment(w, d.docs.as_deref(), d.deprecated.as_deref());
    let union: Vec<String> = e.variants.iter().map(|v| ts_str(&v.wire)).collect();
    let union = if union.is_empty() {
        "never".to_string()
    } else {
        union.join(" | ")
    };
    w.line(format!("export type {name} = {union};"));
    w.blank();
    let make = fc.rt("companion");
    w.open_with(format!("export const {name} = {{"), "");
    let values: Vec<String> = e.variants.iter().map(|v| ts_str(&v.wire)).collect();
    w.line(format!(
        "...{make}<{name}>({}, {{ k: 'enum', values: [{}] }}),",
        ts_str(&d.sig),
        values.join(", ")
    ));
    for v in &e.variants {
        doc_comment(w, v.docs.as_deref(), v.deprecated.as_deref());
        w.line(format!("{}: {},", prop_key(&v.name), ts_str(&v.wire)));
    }
    w.close_with("} as const;");
}

fn write_union(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, u: &Union) {
    let name = type_name(d);
    doc_comment(w, d.docs.as_deref(), d.deprecated.as_deref());
    let disc = prop_key(&u.discriminator);
    let arms: Vec<String> = u
        .arms
        .iter()
        .map(|a| {
            format!(
                "({} & {{ {disc}: {} }})",
                fc.type_ref(a.decl),
                ts_str(&a.label)
            )
        })
        .collect();
    w.line(format!(
        "export type {name} = {};",
        if arms.is_empty() {
            "never".to_string()
        } else {
            arms.join(" | ")
        }
    ));
    w.blank();
    let companion = fc.rt("Companion");
    let make = fc.rt("companion");
    w.open_with(
        format!(
            "export const {name}: {companion}<{name}> = {make}<{name}>({},",
            ts_str(&d.sig)
        ),
        " {",
    );
    w.line("k: 'union',");
    w.line(format!("d: {},", ts_str(&u.discriminator)));
    w.open_with("arms:", " {");
    for a in &u.arms {
        let target = fc.value_ref(a.decl);
        w.line(format!(
            "{}: {{ k: 'ref', to: () => {target}.schema }},",
            prop_key(&a.label)
        ));
    }
    w.close_with("},");
    w.close_with("});");
}

fn write_const(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, c: &Const) {
    doc_comment(w, d.docs.as_deref(), d.deprecated.as_deref());
    let ty = ts_type(fc, &c.ty);
    w.line(format!(
        "export const {}: {ty} = {};",
        type_name(d),
        literal(&c.value)
    ));
}

fn write_error(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, e: &ErrorDecl) {
    let name = type_name(d);
    let data = format!("{name}Data");
    doc_comment(w, d.docs.as_deref(), d.deprecated.as_deref());
    if e.fields.fields.is_empty() {
        w.line(format!("export interface {data} {{}}"));
    } else {
        w.open(format!("export interface {data}"));
        write_fields(w, fc, &e.fields.fields, false);
        w.close();
    }
    w.blank();
    if checks::has_rules(d) {
        checks::write_check_fn(w, fc, d, &data);
        w.blank();
    }
    doc_comment(w, d.docs.as_deref(), d.deprecated.as_deref());
    let base = fc.rt("VeldError");
    let init = fc.rt("ErrorInit");
    let schema = fc.rt("Schema");
    let matches = fc.rt("matchesError");
    w.open(format!("export class {name} extends {base}<{data}>"));
    w.line(format!("static readonly sig = {};", ts_str(&d.sig)));
    if let Some(status) = e.default_status {
        w.line(format!("static readonly defaultStatus = {status};"));
    }
    w.open_with(format!("static readonly schema: {schema} ="), " {");
    write_object_schema(w, fc, &e.fields.fields);
    if checks::has_rules(d) {
        w.line(format!("rules: {},", checks::fn_name(d)));
    }
    w.close_with("};");
    w.blank();
    w.open(format!("constructor(data: {data}, init: {init} = {{}})"));
    let default = e
        .default_status
        .map(|s| format!(", {s}"))
        .unwrap_or_default();
    w.line(format!(
        "super({name}.sig, {}, data, init{default});",
        ts_str(&d.name)
    ));
    w.close();
    w.blank();
    w.open(format!(
        "static matches(e: unknown, service?: string): e is {name}"
    ));
    w.line(format!("return {matches}(e, {name}.sig, service);"));
    w.close();
    if checks::has_rules(d) {
        write_error_rules_members(w, fc, d, &name, &data);
    }
    w.close();
}

fn write_error_rules_members(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    name: &str,
    data: &str,
) {
    let violation = fc.rt("Violation");
    let check = fc.rt("checkValue");
    w.blank();
    w.line(format!("static readonly ruleIds: {} = {};", checks::ids_type(d), checks::ids_value(d)));
    w.blank();
    w.open(format!("static check(data: {data}): {violation}[]"));
    w.line(format!("return {check}({name}.schema, data);"));
    w.close();
}

/// A schema literal for a declaration-less type, used by services (re-exported for the client and server).
pub fn type_schema(fc: &mut FileCtx<'_, '_>, ty: &veld_ir::Type) -> String {
    schema_expr(fc, ty, &veld_ir::Constraints::default())
}

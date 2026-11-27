//! Models, enums, unions, errors and constants: one Rust module per contract module.

use std::collections::BTreeMap;

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{Decl, DeclId, DeclKind, Enum, ErrorDecl, Field, Model, Type, Union};

use crate::tree::{Tree, const_name, schema_name};
use crate::types::{Ctx, const_value, default_json, rust_type, scalar_type, schema_ref};
use crate::util::{HEADER, add_file, bare, doc, lit, snake, type_name, unsupported, variant};

const DERIVE_SERDE: &str = "::serde::Serialize, ::serde::Deserialize";

/// Emits every declaration of the tree: `src/prelude.rs` for the prelude, and one file per
/// contract module.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    let mut by_file: BTreeMap<String, Vec<DeclId>> = BTreeMap::new();
    for id in tree.decls() {
        if !tree.owns_prelude && crate::tree::is_prelude(tree.ir, tree.ir.decl(*id)) {
            continue;
        }
        by_file
            .entry(tree.module_file(tree.ir.decl(*id)))
            .or_default()
            .push(*id);
    }
    for (path, ids) in by_file {
        let here = tree.module_path(tree.ir.decl(ids[0]));
        let ctx = Ctx { tree, here: &here };
        let mut w = CodeWriter::with_unit("    ");
        for id in ids {
            emit_decl(&mut w, &ctx, tree.ir.decl(id))?;
        }
        add_file(files, &path, with_imports(&w.finish()))?;
    }
    Ok(())
}

/// Prepends the header and the imports a body uses.
pub fn with_imports(body: &str) -> String {
    let inner = body.lines().take_while(|l| l.starts_with("//!")).count();
    let (head, body) = match body
        .char_indices()
        .filter(|(_, c)| *c == '\n')
        .nth(inner.saturating_sub(1))
    {
        Some((at, _)) if inner > 0 => body.split_at(at + 1),
        Some(_) | None => ("", body),
    };
    let mut out = format!(
        "{HEADER}\n\n{head}{}",
        if head.is_empty() { "" } else { "\n" }
    );
    let mut any = false;
    if body.contains("veld::") {
        out.push_str("use crate::veld;\n");
        any = true;
    }
    if body.contains("sp::") {
        out.push_str("use crate::veld::spec as sp;\n");
        any = true;
    }
    if body.contains("vs::") {
        out.push_str("use crate::veld::schema as vs;\n");
        any = true;
    }
    if any {
        out.push('\n');
    }
    out.push_str(body.trim_start_matches('\n'));
    out
}

fn emit_decl(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, d: &Decl) -> Result<(), Vec<GenDiagnostic>> {
    match &d.kind {
        DeclKind::Enum(e) => emit_enum(w, d, e),
        DeclKind::Model(m) => emit_model(w, ctx, d, m, None)?,
        DeclKind::Error(e) => emit_model(w, ctx, d, &e.fields, Some(e))?,
        DeclKind::Union(u) => emit_union(w, ctx, d, u),
        DeclKind::Const(c) => return emit_const(w, d, &c.ty, &c.value),
        // An event is a record with event identity (`spec/events.md` section 3): the same struct,
        // schema and check functions as a model. Only a server that publishes or subscribes has
        // the declaration in its tree (`Tree::with_events`).
        DeclKind::Event(e) => emit_model(w, ctx, d, &event_model(e), None)?,
    }
    if ctx.tree.derived && d.has_derived() {
        crate::rules::emit_derived(w, ctx, d)?;
    }
    if ctx.tree.rules && !d.rules().is_empty() {
        match &d.kind {
            DeclKind::Model(m) => return crate::rules::emit(w, ctx, d, m),
            DeclKind::Error(e) => return crate::rules::emit(w, ctx, d, &e.fields),
            DeclKind::Event(e) => return crate::rules::emit(w, ctx, d, &event_model(e)),
            DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Const(_) => {}
        }
    }
    Ok(())
}

/// An event as the model that the struct, the schema and the rules are written from.
fn event_model(e: &veld_ir::Event) -> Model {
    Model {
        parent: None,
        fields: e.fields.clone(),
        rules: e.rules.clone(),
        retain: None,
    }
}

fn deprecated(w: &mut CodeWriter, d: Option<&String>) {
    if let Some(msg) = d {
        w.line("///").line(format!("/// **Deprecated:** {msg}"));
    }
}

fn emit_enum(w: &mut CodeWriter, d: &Decl, e: &Enum) {
    let name = type_name(&d.name);
    doc(w, d.docs.as_deref());
    deprecated(w, d.deprecated.as_ref());
    w.line(format!(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, {DERIVE_SERDE})]"
    ));
    w.open(format!("pub enum {name}"));
    for v in &e.variants {
        doc(w, v.docs.as_deref());
        w.line(format!("#[serde(rename = {})]", lit(&v.wire)));
        w.line(format!("{},", variant(&v.name)));
    }
    w.close().blank();
    w.open(format!("impl {name}"));
    w.line("/// The declaration signature.");
    w.line(format!(
        "pub const VELD_SIG: &'static str = {};",
        lit(&d.sig)
    ));
    w.blank();
    w.line("/// The value on the wire.");
    w.open("pub const fn wire(self) -> &'static str");
    w.open("match self");
    for v in &e.variants {
        w.line(format!("{name}::{} => {},", variant(&v.name), lit(&v.wire)));
    }
    w.close().close();
    if !e.flows.is_empty() {
        machine_helpers(w, &name, e);
    }
    w.close().blank();
    let wires: Vec<String> = e.variants.iter().map(|v| lit(&v.wire)).collect();
    w.line(format!(
        "pub static {}: vs::Schema = vs::Schema::Enum(&[{}]);",
        schema_name(&d.name),
        wires.join(", ")
    ));
    w.blank();
    decl_impl(w, d, &name);
}

/// The helpers of a state machine (`spec/rules.md` section 10.3): the states that can follow a
/// state, and whether a state is final. The server still decides.
fn machine_helpers(w: &mut CodeWriter, name: &str, e: &Enum) {
    w.blank();
    w.line("/// The states that can follow this one, in declaration order of the edges (`flow`). A final");
    w.line("/// state has none.");
    w.open(format!(
        "pub const fn next_states(self) -> &'static [{name}]"
    ));
    w.open("match self");
    for v in &e.variants {
        let next: Vec<String> = e
            .flows
            .iter()
            .filter(|f| f.from == v.name)
            .map(|f| format!("{name}::{}", variant(&f.to)))
            .collect();
        w.line(format!(
            "{name}::{} => &[{}],",
            variant(&v.name),
            next.join(", ")
        ));
    }
    w.close().close().blank();
    w.line("/// Whether this state is final: no state follows it.");
    w.open("pub const fn is_final(self) -> bool");
    let finals: Vec<String> = e
        .finals
        .iter()
        .map(|f| format!("{name}::{}", variant(f)))
        .collect();
    if finals.is_empty() {
        w.line("let _ = self;");
        w.line("false");
    } else {
        w.line(format!("matches!(self, {})", finals.join(" | ")));
    }
    w.close();
    timed_moves_fn(w, name, e);
}

/// `timed_moves()` of a machine with a timed edge (`spec/platform.md` section 10.1): the source,
/// the target and the milliseconds of each timed edge, in the order of the edges. Server and client
/// have it; the server decides whether a move is due.
fn timed_moves_fn(w: &mut CodeWriter, name: &str, e: &Enum) {
    let moves: Vec<String> = e
        .flows
        .iter()
        .filter_map(|f| {
            let ms = f.after_ms.as_deref()?.parse::<i64>().ok()?;
            Some(format!(
                "({name}::{}, {name}::{}, {ms})",
                variant(&f.from),
                variant(&f.to)
            ))
        })
        .collect();
    if moves.is_empty() {
        return;
    }
    w.blank();
    w.line("/// The timed edges: `(from, to, milliseconds)` in the order of the `flow` lines. A move is due");
    w.line("/// when the value has been in `from` for that long; the server decides (`spec/platform.md` 10.1).");
    w.open(format!(
        "pub const fn timed_moves() -> &'static [({name}, {name}, i64)]"
    ));
    w.line(format!("&[{}]", moves.join(", ")));
    w.close();
}

fn decl_impl(w: &mut CodeWriter, d: &Decl, name: &str) {
    w.open(format!("impl veld::VeldDecl for {name}"));
    w.line(format!("const VELD_SIG: &'static str = {name}::VELD_SIG;"));
    w.blank();
    w.open("fn schema() -> &'static vs::Schema");
    w.line(format!("&{}", schema_name(&d.name)));
    w.close().close().blank();
}

fn field_type(ctx: &Ctx<'_, '_>, f: &Field, owner: DeclId) -> String {
    let boxed = matches!(&f.ty, Type::Decl(t) if ctx.tree.reaches(*t, owner));
    let t = rust_type(ctx, &f.ty, boxed);
    if f.optional {
        format!("Option<{t}>")
    } else {
        t
    }
}

fn emit_model(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    d: &Decl,
    m: &Model,
    error: Option<&ErrorDecl>,
) -> Result<(), Vec<GenDiagnostic>> {
    let name = type_name(&d.name);
    doc(w, d.docs.as_deref());
    deprecated(w, d.deprecated.as_ref());
    w.line(format!(
        "#[derive(Debug, Clone, PartialEq, {DERIVE_SERDE})]"
    ));
    w.open(format!("pub struct {name}"));
    for f in &m.fields {
        doc(w, f.docs.as_deref());
        deprecated(w, f.deprecated.as_ref());
        if f.derived.is_some() {
            if f.docs.is_some() {
                w.line("///");
            }
            w.line("/// **Derived** (`spec/rules.md` section 17.2): the server computes it. What a caller sends,");
            w.line("/// and what a handler puts here, is replaced; write any value of the type as a stand-in.");
        }
        let ident = snake(&f.name);
        let mut attrs = Vec::new();
        if bare(&ident) != f.wire {
            attrs.push(format!("rename = {}", lit(&f.wire)));
        }
        if f.optional {
            attrs.push("default".to_string());
            attrs.push("skip_serializing_if = \"Option::is_none\"".to_string());
        }
        if !attrs.is_empty() {
            w.line(format!("#[serde({})]", attrs.join(", ")));
        }
        w.line(format!("pub {ident}: {},", field_type(ctx, f, d.id)));
    }
    w.close().blank();
    w.open(format!("impl {name}"));
    w.line("/// The declaration signature.");
    w.line(format!(
        "pub const VELD_SIG: &'static str = {};",
        lit(&d.sig)
    ));
    w.close().blank();
    model_schema(w, ctx, d, m);
    decl_impl(w, d, &name);
    if ctx.tree.p1.visibility && crate::platform::has_visible(m) {
        crate::platform::emit_visible(w, ctx, d, m)?;
    }
    if let Some(e) = error {
        error_impls(w, d, e, &name);
    }
    Ok(())
}

fn model_schema(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, d: &Decl, m: &Model) {
    w.open_with(
        format!("pub static {}: vs::Schema =", schema_name(&d.name)),
        " vs::Schema::Model(&vs::ModelSchema {",
    );
    w.line(format!("name: {},", lit(&d.name)));
    w.line(format!("sig: {},", lit(&d.sig)));
    w.open_with("fields:", " &[");
    for f in &m.fields {
        w.open("vs::Field");
        w.line(format!("wire: {},", lit(&f.wire)));
        w.line(format!(
            "schema: {},",
            schema_ref(ctx, &f.ty, &f.constraints)
        ));
        w.line(format!("optional: {},", f.optional));
        let default = f.default.as_ref().and_then(default_json);
        w.line(format!(
            "default: {},",
            default.map_or("None".to_string(), |j| format!("Some({})", lit(&j)))
        ));
        w.line(format!("readonly: {},", f.readonly));
        if ctx.tree.derived {
            w.line(format!("derived: {},", f.derived.is_some()));
        }
        if ctx.tree.p1.tenant_fields {
            w.line(format!("tenant: {},", f.tenant));
        }
        if ctx.tree.p1.pii {
            w.line(format!("pii: {},", f.pii || f.secret));
        }
        w.close_with("},");
    }
    w.close_with("],");
    if ctx.tree.rules {
        let check = if m.rules.is_empty() {
            "None".to_string()
        } else {
            format!("Some(veld::rules::run::<{}>)", type_name(&d.name))
        };
        w.line(format!("invariants: {check},"));
    }
    if ctx.tree.derived {
        let derive = if d.has_derived() {
            format!("Some(veld::rules::derive_object::<{}>)", type_name(&d.name))
        } else {
            "None".to_string()
        };
        w.line(format!("derive: {derive},"));
    }
    if ctx.tree.p1.tenancy {
        w.line(format!("tenant: {},", crate::platform::tenant_mark(m)));
    }
    if ctx.tree.p1.visibility {
        let show = if crate::platform::has_visible(m) {
            format!(
                "Some(veld::platform::show_record::<{}>)",
                type_name(&d.name)
            )
        } else {
            "None".to_string()
        };
        w.line(format!("show: {show},"));
    }
    w.close_with("});");
    w.blank();
}

fn error_impls(w: &mut CodeWriter, d: &Decl, e: &ErrorDecl, name: &str) {
    w.open(format!("impl veld::DeclaredError for {name}"));
    w.line(format!("const NAME: &'static str = {};", lit(&d.name)));
    let status = e
        .default_status
        .map_or("None".to_string(), |s| format!("Some({s})"));
    w.line(format!("const DEFAULT_STATUS: Option<u16> = {status};"));
    w.close().blank();
    w.open(format!("impl From<{name}> for veld::Error"));
    w.open(format!("fn from(e: {name}) -> Self"));
    w.line("veld::Error::declared(&e)");
    w.close().close().blank();
    w.open(format!("impl std::fmt::Display for {name}"));
    w.open("fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result");
    w.line(format!("f.write_str({})", lit(&d.name)));
    w.close().close().blank();
    w.line(format!("impl std::error::Error for {name} {{}}"));
    w.blank();
}

fn emit_union(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, d: &Decl, u: &Union) {
    let name = type_name(&d.name);
    doc(w, d.docs.as_deref());
    deprecated(w, d.deprecated.as_ref());
    w.line(format!(
        "#[derive(Debug, Clone, PartialEq, {DERIVE_SERDE})]"
    ));
    w.line(format!("#[serde(tag = {})]", lit(&u.discriminator)));
    w.open(format!("pub enum {name}"));
    for arm in &u.arms {
        w.line(format!("#[serde(rename = {})]", lit(&arm.label)));
        w.line(format!(
            "{}({}),",
            variant(&arm.label),
            ctx.tree.type_path(arm.decl, ctx.here)
        ));
    }
    w.close().blank();
    w.open(format!("impl {name}"));
    w.line("/// The declaration signature.");
    w.line(format!(
        "pub const VELD_SIG: &'static str = {};",
        lit(&d.sig)
    ));
    w.close().blank();
    w.open_with(
        format!("pub static {}: vs::Schema =", schema_name(&d.name)),
        " vs::Schema::Union(&vs::UnionSchema {",
    );
    w.line(format!("name: {},", lit(&d.name)));
    w.line(format!("sig: {},", lit(&d.sig)));
    w.line(format!("discriminator: {},", lit(&u.discriminator)));
    w.open_with("arms:", " &[");
    for arm in &u.arms {
        w.line(format!(
            "({}, &{}),",
            lit(&arm.label),
            ctx.tree.schema_path(arm.decl, ctx.here)
        ));
    }
    w.close_with("],");
    w.close_with("});");
    w.blank();
    decl_impl(w, d, &name);
}

fn emit_const(
    w: &mut CodeWriter,
    d: &Decl,
    ty: &Type,
    value: &veld_ir::Literal,
) -> Result<(), Vec<GenDiagnostic>> {
    let (Type::Scalar(s), Some(v)) = (ty, const_value(value)) else {
        return Err(unsupported(format!(
            "generator `rust` supports constants of scalar types only (`{}`)",
            d.qualified
        )));
    };
    let t = match scalar_type(*s) {
        "String" => "&str",
        other => other,
    };
    doc(w, d.docs.as_deref());
    w.line(format!("pub const {}: {t} = {v};", const_name(&d.name)));
    w.blank();
    Ok(())
}

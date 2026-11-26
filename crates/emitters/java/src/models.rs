//! Models, enums, unions, errors, constants and the prelude. One file per declaration.
//!
//! Output is identical in the client and server roles.

use std::collections::BTreeMap;

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{
    Const, Decl, DeclId, DeclKind, Enum, ErrorDecl, Field, Literal, Scalar, Type, Union,
};

use crate::rules;
use crate::tree::{FileCtx, Tree, java_path};
use crate::types::{codec_expr, java_type, json_literal, nullable, schema_expr};
use crate::util::{
    add_file, deprecated_annotation, doc_comment, end_call, java_str, member_ident, open_call,
    type_name, unique,
};

const SUPPRESS: &str = "@SuppressWarnings(\"deprecation\")";

/// Emits every declaration of `tree`.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    let mut consts: BTreeMap<String, Vec<DeclId>> = BTreeMap::new();
    for &id in tree.decls() {
        let d = tree.ir.decl(id);
        let package = tree.decl_package(d);
        if matches!(d.kind, DeclKind::Const(_)) {
            consts.entry(package).or_default().push(id);
            continue;
        }
        let name = type_name(d);
        add_file(
            files,
            &java_path(&package, &name),
            render(tree, d, &package, &name)?,
        )?;
    }
    for (package, ids) in consts {
        let text = render_constants(tree, &package, &ids)?;
        add_file(files, &java_path(&package, "Constants"), text)?;
    }
    crate::policy::emit(files, tree)
}

fn render(
    tree: &Tree<'_>,
    d: &Decl,
    package: &str,
    name: &str,
) -> Result<String, Vec<GenDiagnostic>> {
    let mut fc = FileCtx::new(tree, package, name);
    let mut w = CodeWriter::new();
    match &d.kind {
        DeclKind::Model(m) => write_record(&mut w, &mut fc, d, &m.fields)?,
        DeclKind::Event(e) => write_record(&mut w, &mut fc, d, &e.fields)?,
        DeclKind::Enum(e) => write_enum(&mut w, &mut fc, d, e),
        DeclKind::Union(u) => write_union(&mut w, &mut fc, d, u),
        DeclKind::Error(e) => write_error(&mut w, &mut fc, d, e)?,
        DeclKind::Const(_) => {}
    }
    Ok(fc.finish(&w.finish()))
}

fn head_docs(w: &mut CodeWriter, d: &Decl, params: &[(String, String)]) {
    doc_comment(w, d.docs.as_deref(), params, d.deprecated.as_deref());
    deprecated_annotation(w, d.deprecated.as_deref());
    w.line(SUPPRESS);
}

/// `@param` lines for the fields that carry docs or a deprecation.
fn field_params(fields: &[Field], idents: &[String]) -> Vec<(String, String)> {
    fields
        .iter()
        .zip(idents)
        .filter_map(|(f, id)| {
            let first = f
                .docs
                .as_deref()
                .and_then(|d| d.trim().lines().next())
                .unwrap_or("")
                .to_string();
            let text = match (&f.deprecated, first.is_empty()) {
                (Some(m), true) => format!("Deprecated: {m}"),
                (Some(m), false) => format!("{first} Deprecated: {m}"),
                (None, _) => first,
            };
            (!text.is_empty()).then(|| (id.clone(), text))
        })
        .collect()
}

pub(crate) fn field_idents(fields: &[Field]) -> Vec<String> {
    unique(fields.iter().map(|f| member_ident(&f.name)).collect())
}

/// `public record Name(` / components / `) implements X {`, leaving the writer inside the body.
fn open_record(
    w: &mut CodeWriter,
    modifiers: &str,
    name: &str,
    comps: &[String],
    implements: &str,
) {
    if comps.is_empty() {
        w.open(format!("{modifiers} record {name}(){implements}"));
        return;
    }
    w.line(format!("{modifiers} record {name}("));
    w.indent().indent();
    for (i, c) in comps.iter().enumerate() {
        if i + 1 == comps.len() {
            w.line(format!("{c}){implements} {{"));
        } else {
            w.line(format!("{c},"));
        }
    }
    w.dedent();
}

fn components(fc: &mut FileCtx<'_, '_>, fields: &[Field], idents: &[String]) -> Vec<String> {
    fields
        .iter()
        .zip(idents)
        .map(|(f, id)| format!("{} {id}", java_type(fc, &f.ty, nullable(f))))
        .collect()
}

/// `fromWire` and `toWire` of a record with `fields`.
fn write_wire_methods(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    name: &str,
    fields: &[Field],
    idents: &[String],
) {
    let json = fc.rt("Json");
    let codec = fc.rt("Codec");
    w.line("/** Reads the wire form of a value that was validated and prepared with `Wire`. */");
    w.open(format!("public static {name} fromWire({json} json)"));
    if fields.is_empty() {
        w.line(format!("return new {name}();"));
    } else {
        w.line(format!("{json}.Obj o = json.asObject();"));
        open_call(w, format!("return new {name}"));
        for (i, f) in fields.iter().enumerate() {
            let get = if nullable(f) { "opt" } else { "req" };
            let sep = if i + 1 == fields.len() { ");" } else { "," };
            let c = codec_expr(fc, &f.ty);
            w.line(format!("{codec}.{get}(o, {}, {c}){sep}", java_str(&f.wire)));
        }
        end_call(w);
    }
    w.close();
    w.blank();
    w.line("/** The wire form of this value. */");
    w.open(format!("public {json}.Obj toWire()"));
    w.line(format!("return {json}.object()"));
    w.indent().indent();
    for (f, id) in fields.iter().zip(idents) {
        let c = codec_expr(fc, &f.ty);
        w.line(format!(".put({}, {id}, {c})", java_str(&f.wire)));
    }
    w.line(".build();");
    w.dedent().dedent();
    w.close();
}

fn write_sig(w: &mut CodeWriter, d: &Decl, modifiers: &str) {
    w.line("/** The declaration signature (spec/ir.md section 5.1). */");
    w.line(format!(
        "{modifiers}String VELD_SIG = {};",
        java_str(&d.sig)
    ));
}

fn write_object_schema(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    fields: &[Field],
    modifiers: &str,
    checked: &str,
) {
    let schema = fc.rt("Schema");
    w.line("/** The wire description: validation, defaults and canonical bytes. */");
    if fields.is_empty() {
        w.line(format!(
            "{modifiers}{schema} SCHEMA = {schema}.object(){checked};"
        ));
        return;
    }
    open_call(w, format!("{modifiers}{schema} SCHEMA = {schema}.object"));
    for (i, f) in fields.iter().enumerate() {
        let ty = schema_expr(fc, &f.ty, &f.constraints, true);
        let default = f
            .default
            .as_ref()
            .map_or_else(|| "null".to_string(), |l| json_literal(fc, l));
        let sep = match (i + 1 == fields.len(), checked.is_empty()) {
            (false, _) => ",".to_string(),
            (true, true) => ");".to_string(),
            (true, false) => format!("){checked};"),
        };
        w.line(format!(
            "{schema}.field({}, {}, {ty}, {}, {default}, {}){sep}",
            java_str(&f.name),
            java_str(&f.wire),
            f.optional || f.derived.is_some(),
            f.readonly
        ));
    }
    end_call(w);
}

fn write_codec_and_canonical(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, name: &str) {
    let codec = fc.rt("Codec");
    let wire = fc.rt("Wire");
    w.line("/** A codec for use in composite types. */");
    w.open(format!("public static {codec}<{name}> codec()"));
    w.line(format!(
        "return {codec}.of({name}::fromWire, {name}::toWire);"
    ));
    w.close();
    w.blank();
    w.line("/** The canonical bytes of this value (spec/ir.md section 5.3). */");
    w.open("public String veldCanonical()");
    w.line(format!("return {wire}.canonicalOf(SCHEMA, toWire());"));
    w.close();
}

fn write_record(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    fields: &[Field],
) -> Result<(), Vec<GenDiagnostic>> {
    let name = type_name(d);
    let idents = field_idents(fields);
    head_docs(w, d, &field_params(fields, &idents));
    let unions: Vec<String> = fc
        .tree
        .unions_of(d.id)
        .into_iter()
        .map(|u| fc.decl(u))
        .collect();
    let implements = if unions.is_empty() {
        String::new()
    } else {
        format!(" implements {}", unions.join(", "))
    };
    let comps = components(fc, fields, &idents);
    open_record(w, "public", &name, &comps, &implements);
    write_sig(w, d, "public static final ");
    w.blank();
    let checked = rules::schema_suffix(d, &name);
    write_object_schema(w, fc, fields, "public static final ", &checked);
    w.blank();
    write_wire_methods(w, fc, &name, fields, &idents);
    w.blank();
    write_codec_and_canonical(w, fc, &name);
    if rules::has_rules(d) {
        w.blank();
        rules::write_members(w, fc, d, &name)?;
    }
    if crate::derived::has_derived(d) {
        w.blank();
        crate::derived::write_members(w, fc, d, &name)?;
    }
    w.close();
    Ok(())
}

fn write_enum(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, e: &Enum) {
    let name = type_name(d);
    let json = fc.rt("Json");
    let schema = fc.rt("Schema");
    head_docs(w, d, &[]);
    w.open(format!("public enum {name}"));
    let consts = enum_constants(e);
    for (i, (v, c)) in e.variants.iter().zip(&consts).enumerate() {
        doc_comment(w, v.docs.as_deref(), &[], v.deprecated.as_deref());
        deprecated_annotation(w, v.deprecated.as_deref());
        let end = if i + 1 == e.variants.len() { ";" } else { "," };
        w.line(format!("{c}({}){end}", java_str(&v.wire)));
    }
    if e.variants.is_empty() {
        w.line(";");
    }
    w.blank();
    write_sig(w, d, "public static final ");
    w.blank();
    let values: Vec<String> = e.variants.iter().map(|v| java_str(&v.wire)).collect();
    w.line("/** The wire description: validation and canonical bytes. */");
    w.line(format!(
        "public static final {schema} SCHEMA = {schema}.enumOf({});",
        values.join(", ")
    ));
    w.blank();
    w.line("private final String wire;");
    w.blank();
    w.open(format!("{name}(String wire)"));
    w.line("this.wire = wire;");
    w.close();
    w.blank();
    w.line("/** The value on the wire. */");
    w.open("public String wire()");
    w.line("return wire;");
    w.close();
    w.blank();
    w.line("/** The wire form of this value. */");
    w.open(format!("public {json} toWire()"));
    w.line(format!("return {json}.of(wire);"));
    w.close();
    w.blank();
    w.line("/** Reads the wire form; an unknown value is an `IllegalArgumentException`. */");
    w.open(format!("public static {name} fromWire({json} json)"));
    w.line("String value = json.asString();");
    w.open(format!("for ({name} v : values())"));
    w.open("if (v.wire.equals(value))");
    w.line("return v;");
    w.close();
    w.close();
    w.line(format!(
        "throw new IllegalArgumentException(\"unknown {name}: \" + value);"
    ));
    w.close();
    w.blank();
    let codec = fc.rt("Codec");
    let wire = fc.rt("Wire");
    w.line("/** A codec for use in composite types. */");
    w.open(format!("public static {codec}<{name}> codec()"));
    w.line(format!(
        "return {codec}.of({name}::fromWire, {name}::toWire);"
    ));
    w.close();
    w.blank();
    w.line("/** The canonical bytes of this value (spec/ir.md section 5.3). */");
    w.open("public String veldCanonical()");
    w.line(format!("return {wire}.canonicalOf(SCHEMA, toWire());"));
    w.close();
    if e.is_state_machine() {
        w.blank();
        write_machine_helpers(w, fc, &name, e);
    }
    w.close();
}

/// `nextStates()` and `isFinal()` of a state machine (`spec/rules.md` section 10.3).
fn write_machine_helpers(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, name: &str, e: &Enum) {
    let list = fc.name("java.util.List");
    let consts = enum_constants(e);
    let constant = |variant: &str| {
        e.variants
            .iter()
            .position(|v| v.name == variant)
            .and_then(|i| consts.get(i).cloned())
            .unwrap_or_default()
    };
    w.line(
        "/** The states that can follow this one, in declaration order of the edges (`flow`). */",
    );
    w.open(format!("public {list}<{name}> nextStates()"));
    w.open("return switch (this)");
    for (v, c) in e.variants.iter().zip(&consts) {
        let next: Vec<String> = e
            .flows
            .iter()
            .filter(|f| f.from == v.name)
            .map(|f| constant(&f.to))
            .collect();
        w.line(format!("case {c} -> {list}.of({});", next.join(", ")));
    }
    w.close_with("};");
    w.close();
    w.blank();
    w.line("/** Whether this state is final: no state follows it. */");
    w.open("public boolean isFinal()");
    if e.finals.is_empty() {
        w.line("return false;");
    } else {
        let finals: Vec<String> = e.finals.iter().map(|f| constant(f)).collect();
        w.line(format!(
            "return switch (this) {{ case {} -> true; default -> false; }};",
            finals.join(", ")
        ));
    }
    w.close();
}

/// Java constant names for the variants of an enum, unique and legal.
pub(crate) fn enum_constants(e: &Enum) -> Vec<String> {
    unique(
        e.variants
            .iter()
            .map(|v| naming::escape(naming::Lang::Java, &naming::screaming_snake(&v.name)))
            .collect(),
    )
}

fn write_union(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, u: &Union) {
    let name = type_name(d);
    let schema = fc.rt("Schema");
    head_docs(w, d, &[]);
    let package = fc.tree.decl_package(d);
    let arms: Vec<String> = u.arms.iter().map(|a| fc.decl(a.decl)).collect();
    let same_package = u
        .arms
        .iter()
        .all(|a| fc.tree.decl_package(fc.tree.ir.decl(a.decl)) == package);
    if same_package && !arms.is_empty() {
        w.open(format!(
            "public sealed interface {name} permits {}",
            arms.join(", ")
        ));
    } else {
        w.open(format!("public interface {name}"));
    }
    write_sig(w, d, "");
    w.blank();
    w.line("/** The wire description: validation and canonical bytes. */");
    if arms.is_empty() {
        w.line(format!(
            "{schema} SCHEMA = {schema}.union({});",
            java_str(&u.discriminator)
        ));
    } else {
        open_call(w, format!("{schema} SCHEMA = {schema}.union"));
        w.line(format!("{},", java_str(&u.discriminator)));
        for (i, (a, t)) in u.arms.iter().zip(&arms).enumerate() {
            let sep = if i + 1 == arms.len() { ");" } else { "," };
            w.line(format!(
                "{schema}.arm({}, () -> {t}.SCHEMA){sep}",
                java_str(&a.label)
            ));
        }
        end_call(w);
    }
    w.blank();
    write_union_methods(w, fc, &name, u, &arms);
    w.close();
}

fn write_union_methods(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    name: &str,
    u: &Union,
    arms: &[String],
) {
    let json = fc.rt("Json");
    let codec = fc.rt("Codec");
    let wire = fc.rt("Wire");
    let disc = java_str(&u.discriminator);
    w.line("/** The wire form of an arm: the arm's fields plus the discriminator. */");
    w.open(format!("static {json}.Obj toWire({name} value)"));
    for (a, t) in u.arms.iter().zip(arms) {
        w.open(format!("if (value instanceof {t} v)"));
        w.line(format!(
            "return v.toWire().with({disc}, {json}.of({}));",
            java_str(&a.label)
        ));
        w.close();
    }
    w.line(format!(
        "throw new IllegalArgumentException(\"unknown {name} arm: \" + value);"
    ));
    w.close();
    w.blank();
    w.line("/** Reads an arm by its discriminator. */");
    w.open(format!("static {name} fromWire({json} json)"));
    w.line(format!(
        "String label = {codec}.req(json.asObject(), {disc}, {codec}.string());"
    ));
    w.open("switch (label)");
    for (a, t) in u.arms.iter().zip(arms) {
        w.line(format!("case {}:", java_str(&a.label)));
        w.line(format!("  return {t}.fromWire(json);"));
    }
    w.line("default:");
    w.line(format!(
        "  throw new IllegalArgumentException(\"unknown {name} arm: \" + label);"
    ));
    w.close();
    w.close();
    w.blank();
    w.line("/** A codec for use in composite types. */");
    w.open(format!("static {codec}<{name}> codec()"));
    w.line(format!(
        "return {codec}.of({name}::fromWire, {name}::toWire);"
    ));
    w.close();
    w.blank();
    w.line("/** The canonical bytes of an arm (spec/ir.md section 5.3). */");
    w.open(format!("static String veldCanonical({name} value)"));
    w.line(format!("return {wire}.canonicalOf(SCHEMA, toWire(value));"));
    w.close();
}

fn write_error(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    e: &ErrorDecl,
) -> Result<(), Vec<GenDiagnostic>> {
    let name = type_name(d);
    let base = fc.rt("VeldError");
    let json = fc.rt("Json");
    let schema = fc.rt("Schema");
    let veld = fc.rt("Veld");
    let fields = &e.fields.fields;
    let idents = field_idents(fields);
    head_docs(w, d, &field_params(fields, &idents));
    w.open(format!("public final class {name} extends {base}"));
    w.line("private static final long serialVersionUID = 1L;");
    w.blank();
    write_sig(w, d, "public static final ");
    if let Some(status) = e.default_status {
        w.line("/** The status used when none is given. */");
        w.line(format!(
            "public static final int DEFAULT_STATUS = {status};"
        ));
    }
    w.blank();
    let checked = rules::schema_suffix(d, &name);
    write_object_schema(w, fc, fields, "public static final ", &checked);
    w.blank();
    w.line("/** The error payload. */");
    let comps = components(fc, fields, &idents);
    open_record(w, "public", "Data", &comps, "");
    write_wire_methods(w, fc, "Data", fields, &idents);
    w.close();
    w.blank();
    w.line("private final transient Data data;");
    w.blank();
    let default = if e.default_status.is_some() {
        "DEFAULT_STATUS"
    } else {
        "500"
    };
    w.line("/** Creates the error with its default status. */");
    w.open(format!("public {name}(Data data)"));
    w.line(format!("this(data, {default}, null);"));
    w.close();
    w.blank();
    w.line("/** Creates the error with an explicit status and operation signature. */");
    w.open(format!("public {name}(Data data, int status, String op)"));
    w.line(format!(
        "super(VELD_SIG, {}, status, op);",
        java_str(&d.name)
    ));
    w.line("this.data = data;");
    w.close();
    w.blank();
    w.line("/** Decodes the error from its wire data, as a client does. */");
    w.open(format!(
        "public static {name} fromWire({json} data, int status, String op)"
    ));
    w.line(format!(
        "return new {name}(Data.fromWire(data), status, op);"
    ));
    w.close();
    w.blank();
    write_error_accessors(w, &name, &json, &schema, &veld);
    if rules::has_rules(d) {
        w.blank();
        rules::write_members(w, fc, d, "Data")?;
    }
    if crate::derived::has_derived(d) {
        w.blank();
        crate::derived::write_members(w, fc, d, "Data")?;
    }
    w.close();
    Ok(())
}

fn write_error_accessors(w: &mut CodeWriter, name: &str, json: &str, schema: &str, veld: &str) {
    w.line("@Override");
    w.open("public Data data()");
    w.line("return data;");
    w.close();
    w.blank();
    w.line("@Override");
    w.open(format!("public {json} dataWire()"));
    w.line("return data.toWire();");
    w.close();
    w.blank();
    w.line("@Override");
    w.open(format!("public {schema} dataSchema()"));
    w.line("return SCHEMA;");
    w.close();
    w.blank();
    w.line(format!(
        "/** Whether {{@code error}} is a `{name}`, whichever generated package it comes from. */"
    ));
    w.open("public static boolean matches(Throwable error)");
    w.line(format!("return {veld}.matches(error, VELD_SIG, null);"));
    w.close();
    w.blank();
    w.line(format!("/** Whether {{@code error}} is a `{name}` raised by {{@code service}} (`package/Service`). */"));
    w.open("public static boolean matches(Throwable error, String service)");
    w.line(format!("return {veld}.matches(error, VELD_SIG, service);"));
    w.close();
}

// -- constants ----------------------------------------------------------------------------------

fn render_constants(
    tree: &Tree<'_>,
    package: &str,
    ids: &[DeclId],
) -> Result<String, Vec<GenDiagnostic>> {
    let mut fc = FileCtx::new(tree, package, "Constants");
    let mut w = CodeWriter::new();
    doc_comment(
        &mut w,
        Some("Constants declared by the contract."),
        &[],
        None,
    );
    w.open("public final class Constants");
    w.line("private Constants() {}");
    let names = unique(
        ids.iter()
            .map(|id| {
                naming::escape(
                    naming::Lang::Java,
                    &naming::screaming_snake(&tree.ir.decl(*id).name),
                )
            })
            .collect(),
    );
    for (id, name) in ids.iter().zip(names) {
        let d = tree.ir.decl(*id);
        let DeclKind::Const(c) = &d.kind else {
            continue;
        };
        w.blank();
        doc_comment(&mut w, d.docs.as_deref(), &[], d.deprecated.as_deref());
        deprecated_annotation(&mut w, d.deprecated.as_deref());
        let ty = java_type(&mut fc, &c.ty, false);
        let value = const_value(&mut fc, d, c)?;
        w.line(format!("public static final {ty} {name} = {value};"));
    }
    w.close();
    Ok(fc.finish(&w.finish()))
}

fn unsupported(d: &Decl, what: &str) -> Vec<GenDiagnostic> {
    vec![GenDiagnostic {
        code: "E0804",
        message: format!(
            "constant `{}` has {what}, which the java generator cannot represent",
            d.name
        ),
        origin: Some(d.origin.clone()),
    }]
}

/// The Java initialiser of a constant.
fn const_value(
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    c: &Const,
) -> Result<String, Vec<GenDiagnostic>> {
    match (&c.ty, &c.value) {
        (Type::Scalar(s), lit) => scalar_const(fc, d, *s, lit),
        (Type::Decl(id), Literal::EnumVariant { name, .. }) => {
            let enum_name = fc.decl(*id);
            let DeclKind::Enum(e) = &fc.tree.ir.decl(*id).kind else {
                return Err(unsupported(d, "a non-enum declaration type"));
            };
            let consts = enum_constants(e);
            let pos = e
                .variants
                .iter()
                .position(|v| &v.name == name)
                .ok_or_else(|| unsupported(d, "an unknown variant"))?;
            Ok(format!("{enum_name}.{}", consts[pos]))
        }
        (Type::List(_), Literal::EmptyList) => Ok(format!("{}.of()", fc.name("java.util.List"))),
        (
            Type::Decl(_),
            Literal::String(_)
            | Literal::Int(_)
            | Literal::Float(_)
            | Literal::Bool(_)
            | Literal::Null
            | Literal::EmptyList,
        )
        | (
            Type::List(_),
            Literal::String(_)
            | Literal::Int(_)
            | Literal::Float(_)
            | Literal::Bool(_)
            | Literal::Null
            | Literal::EnumVariant { .. },
        )
        | (
            Type::Map(_) | Type::Page(_),
            Literal::String(_)
            | Literal::Int(_)
            | Literal::Float(_)
            | Literal::Bool(_)
            | Literal::Null
            | Literal::EnumVariant { .. }
            | Literal::EmptyList,
        ) => Err(unsupported(d, "a composite value")),
    }
}

fn scalar_const(
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    s: Scalar,
    lit: &Literal,
) -> Result<String, Vec<GenDiagnostic>> {
    Ok(match (s, lit) {
        (Scalar::String, Literal::String(v)) => java_str(v),
        (Scalar::Int, Literal::Int(i)) if i32::try_from(*i).is_ok() => i.to_string(),
        (Scalar::Long, Literal::Int(i)) => format!("{i}L"),
        (Scalar::Float, Literal::Float(f)) => crate::types::java_double(*f),
        (Scalar::Float, Literal::Int(i)) => format!("{i}.0"),
        (Scalar::Bool, Literal::Bool(b)) => b.to_string(),
        (Scalar::Decimal, Literal::String(v)) => {
            format!("new {}({})", fc.name("java.math.BigDecimal"), java_str(v))
        }
        (Scalar::Date, Literal::String(v)) => {
            format!("{}.parse({})", fc.name("java.time.LocalDate"), java_str(v))
        }
        (Scalar::Datetime, Literal::String(v)) => {
            format!("{}.parse({})", fc.name("java.time.Instant"), java_str(v))
        }
        (Scalar::Time, Literal::String(v)) => {
            format!("{}.parse({})", fc.name("java.time.LocalTime"), java_str(v))
        }
        (Scalar::Uuid, Literal::String(v)) => {
            format!("{}.fromString({})", fc.name("java.util.UUID"), java_str(v))
        }
        // Every other pairing is unsupported and must fail loudly (E0804), never emit wrong code,
        // so the fallthrough is the intended behaviour for a new `Scalar` or `Literal` too.
        _ => return Err(unsupported(d, "a literal of an unsupported type")),
    })
}

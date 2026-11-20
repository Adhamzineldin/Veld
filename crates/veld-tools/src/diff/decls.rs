//! Declaration-level comparison: models, errors, enums, unions, constants.

use std::collections::BTreeMap;

use veld_ir::{Const, Decl, DeclKind, Enum, Field, Ir, PackageKind, Union};

use super::usage::{Role, Usage};
use super::{Change, ChangeKind, Ctx, push_constraint_changes};

/// Declarations by qualified name. Prelude declarations come and go with their
/// use (reported as added or removed errors on the endpoints), so they are skipped.
fn by_name(ir: &Ir) -> BTreeMap<&str, &Decl> {
    ir.decls
        .iter()
        .filter(|d| ir.package(d.package).kind != PackageKind::Prelude)
        .map(|d| (d.qualified.as_str(), d))
        .collect()
}

fn kind_name(d: &Decl) -> &'static str {
    match d.kind {
        DeclKind::Model(_) => "model",
        DeclKind::Error(_) => "error",
        DeclKind::Enum(_) => "enum",
        DeclKind::Union(_) => "union",
        DeclKind::Const(_) => "const",
        _ => "declaration",
    }
}

pub(super) fn diff_decls(ctx: &Ctx<'_>, usage: &Usage, out: &mut Vec<Change>) {
    let (old, new) = (by_name(ctx.old), by_name(ctx.new));
    for (q, o) in &old {
        match new.get(q) {
            None => out.push(Change::breaking(
                ChangeKind::DeclRemoved,
                q,
                format!("{} `{}` was removed or renamed", kind_name(o), o.name),
            )),
            Some(n) => diff_decl(ctx, o, n, usage.role(q), out),
        }
    }
    for (q, n) in &new {
        if !old.contains_key(q) {
            out.push(Change::non_breaking(
                ChangeKind::DeclAdded,
                q,
                format!("{} `{}` was added", kind_name(n), n.name),
            ));
        }
    }
}

fn diff_decl(ctx: &Ctx<'_>, o: &Decl, n: &Decl, role: Role, out: &mut Vec<Change>) {
    let q = &o.qualified;
    let shape = o.sig != n.sig;
    match (&o.kind, &n.kind) {
        (DeclKind::Model(_) | DeclKind::Error(_), DeclKind::Model(_) | DeclKind::Error(_))
            if kind_name(o) == kind_name(n) =>
        {
            diff_fields(ctx, q, (o.fields(), n.fields()), role, shape, out);
        }
        (DeclKind::Enum(a), DeclKind::Enum(b)) if shape => diff_enum(q, a, b, role, out),
        (DeclKind::Union(a), DeclKind::Union(b)) if shape => diff_union(ctx, q, (a, b), role, out),
        (DeclKind::Const(a), DeclKind::Const(b)) => diff_const(ctx, q, a, b, out),
        (DeclKind::Enum(_), DeclKind::Enum(_)) | (DeclKind::Union(_), DeclKind::Union(_)) => {}
        _ => out.push(Change::breaking(
            ChangeKind::DeclKindChanged,
            q,
            format!("`{}` changed from {} to {}", o.name, kind_name(o), kind_name(n)),
        )),
    }
    diff_docs(q, &o.name, (&o.docs, &n.docs), (&o.deprecated, &n.deprecated), out);
}

/// Docs and deprecation changes of a named element; always non-breaking.
pub(super) fn diff_docs(
    subject: &str,
    what: &str,
    docs: (&Option<String>, &Option<String>),
    dep: (&Option<String>, &Option<String>),
    out: &mut Vec<Change>,
) {
    if docs.0 != docs.1 {
        out.push(Change::non_breaking(ChangeKind::DocsChanged, subject, format!("docs of `{what}` changed")));
    }
    match (dep.0, dep.1) {
        (None, Some(m)) => out.push(Change::non_breaking(
            ChangeKind::Deprecated,
            subject,
            format!("`{what}` was deprecated: {m}"),
        )),
        (Some(_), None) => out.push(Change::non_breaking(
            ChangeKind::Undeprecated,
            subject,
            format!("`{what}` is no longer deprecated"),
        )),
        _ => {}
    }
}

fn wire_map(fields: &[Field]) -> BTreeMap<&str, &Field> {
    fields.iter().map(|f| (f.wire.as_str(), f)).collect()
}

fn request_required(f: &Field) -> bool {
    !f.optional && f.default.is_none()
}

fn diff_fields(
    ctx: &Ctx<'_>,
    q: &str,
    fields: (&[Field], &[Field]),
    role: Role,
    shape: bool,
    out: &mut Vec<Change>,
) {
    let (old, new) = (wire_map(fields.0), wire_map(fields.1));
    for (w, o) in &old {
        match new.get(w) {
            Some(n) => diff_field(ctx, q, (o, n), role, shape, out),
            None if shape => out.push(Change::breaking(
                ChangeKind::FieldRemoved,
                q,
                format!("field `{w}` was removed or renamed"),
            )),
            None => {}
        }
    }
    if !shape {
        return;
    }
    for (w, n) in &new {
        if old.contains_key(w) {
            continue;
        }
        if request_required(n) && role.request {
            out.push(Change::breaking(
                ChangeKind::RequiredFieldAdded,
                q,
                format!("required field `{w}` was added to a request type"),
            ));
        } else {
            out.push(Change::non_breaking(ChangeKind::FieldAdded, q, format!("field `{w}` was added")));
        }
    }
}

fn diff_field(
    ctx: &Ctx<'_>,
    q: &str,
    pair: (&Field, &Field),
    role: Role,
    shape: bool,
    out: &mut Vec<Change>,
) {
    let (o, n) = pair;
    let w = &o.wire;
    if shape && ctx.type_key(true, &o.ty) != ctx.type_key(false, &n.ty) {
        out.push(Change::breaking(ChangeKind::FieldTypeChanged, q, format!("type of field `{w}` changed")));
    }
    if !request_required(o) && request_required(n) {
        out.push(Change::when(
            role.request,
            ChangeKind::FieldBecameRequired,
            q,
            format!("field `{w}` became required"),
        ));
    }
    if !o.optional && n.optional {
        out.push(Change::when(
            role.response,
            ChangeKind::FieldBecameOptional,
            q,
            format!("field `{w}` became optional"),
        ));
    }
    push_constraint_changes(out, q, &format!("field `{w}`"), &o.constraints, &n.constraints);
    diff_docs(q, w, (&o.docs, &n.docs), (&o.deprecated, &n.deprecated), out);
}

fn diff_enum(q: &str, o: &Enum, n: &Enum, role: Role, out: &mut Vec<Change>) {
    let old: Vec<&str> = o.variants.iter().map(|v| v.wire.as_str()).collect();
    let new: Vec<&str> = n.variants.iter().map(|v| v.wire.as_str()).collect();
    for w in old.iter().filter(|w| !new.contains(w)) {
        out.push(Change::breaking(
            ChangeKind::EnumVariantRemoved,
            q,
            format!("enum value `{w}` was removed"),
        ));
    }
    for w in new.iter().filter(|w| !old.contains(w)) {
        let breaking = role.response && !role.request;
        out.push(Change::when(
            breaking,
            ChangeKind::EnumVariantAdded,
            q,
            format!("enum value `{w}` was added"),
        ));
    }
}

fn diff_union(ctx: &Ctx<'_>, q: &str, pair: (&Union, &Union), role: Role, out: &mut Vec<Change>) {
    let (o, n) = pair;
    if o.discriminator != n.discriminator {
        out.push(Change::breaking(
            ChangeKind::DiscriminatorChanged,
            q,
            format!("discriminator changed from `{}` to `{}`", o.discriminator, n.discriminator),
        ));
    }
    let target = |ir: &Ir, d| ir.decl(d).qualified.clone();
    for a in &o.arms {
        match n.arms.iter().find(|b| b.label == a.label) {
            None => out.push(Change::breaking(
                ChangeKind::UnionArmRemoved,
                q,
                format!("union arm `{}` was removed", a.label),
            )),
            Some(b) if target(ctx.old, a.decl) != target(ctx.new, b.decl) => out.push(Change::breaking(
                ChangeKind::UnionArmChanged,
                q,
                format!("union arm `{}` now points to a different model", a.label),
            )),
            Some(_) => {}
        }
    }
    for b in n.arms.iter().filter(|b| !o.arms.iter().any(|a| a.label == b.label)) {
        let breaking = role.response && !role.request;
        out.push(Change::when(
            breaking,
            ChangeKind::UnionArmAdded,
            q,
            format!("union arm `{}` was added", b.label),
        ));
    }
}

fn diff_const(ctx: &Ctx<'_>, q: &str, o: &Const, n: &Const, out: &mut Vec<Change>) {
    if ctx.type_key(true, &o.ty) != ctx.type_key(false, &n.ty) {
        out.push(Change::breaking(ChangeKind::ConstChanged, q, "constant type changed"));
    } else if o.value != n.value {
        out.push(Change::non_breaking(ChangeKind::ConstChanged, q, "constant value changed"));
    }
}

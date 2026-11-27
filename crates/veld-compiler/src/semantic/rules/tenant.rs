//! `@tenant` (`spec/platform.md` section 3): `E1110`.
//!
//! A model has at most one `@tenant` field, its own or inherited. It is required and of type
//! `uuid`, `string`, `int` or `long`, except on the model of the package's principal, whose field
//! may be optional so that a caller can have no tenant. A service that loads or returns a
//! tenant-scoped model needs a principal whose model has a `@tenant` field of the same type, and
//! every such endpoint needs auth.

use super::visible::closure;
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{DKind, Program, RField};
use veld_ir::{DeclId, Scalar, Type};

fn err(diags: &mut Vec<Diagnostic>, msg: String, span: Span, help: &str) {
    diags.push(Diagnostic::error("E1110", msg, span).with_help(help.to_string()));
}

/// True for the types a tenant can be.
fn is_tenant_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Scalar(Scalar::Uuid | Scalar::String | Scalar::Int | Scalar::Long)
    )
}

/// The model checks: where the annotation stands, how many, and of what shape.
pub(in crate::semantic) fn check(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    for (i, d) in prog.decls.iter().enumerate() {
        if !matches!(d.kind, DKind::Model | DKind::Error | DKind::Event) {
            continue;
        }
        let own: Vec<&RField> = d.res.own_fields.iter().filter(|f| f.tenant).collect();
        if own.is_empty() {
            continue;
        }
        let principal = prog.principals.get(&d.pkg).is_some_and(|p| p.decl == i);
        for f in &own {
            check_field(d.kind, principal, &d.name, f, diags);
        }
        let all: Vec<&RField> = d.res.fields.iter().filter(|f| f.tenant).collect();
        if let Some(first) = all.first() {
            for extra in all.iter().skip(1).filter(|f| f.inherited_from.is_none()) {
                diags.push(
                    Diagnostic::error(
                        "E1110",
                        format!(
                            "`{}` has two `@tenant` fields: `{}` and `{}`",
                            d.name, first.name, extra.name
                        ),
                        extra.span,
                    )
                    .with_label(first.span, "the first one"),
                );
            }
        }
    }
}

fn check_field(kind: DKind, principal: bool, model: &str, f: &RField, diags: &mut Vec<Diagnostic>) {
    if kind == DKind::Error {
        err(
            diags,
            format!("`@tenant` field `{}` is on an error", f.name),
            f.span,
            "a tenant stands on a field of a model or an event; an error carries a model if it must hold tenant data",
        );
        return;
    }
    if f.derived {
        err(
            diags,
            format!("`@tenant` field `{}` is derived", f.name),
            f.span,
            "the server sets the tenant from the caller: a derived value cannot be it",
        );
    }
    if !is_tenant_type(&f.ty) {
        err(
            diags,
            format!(
                "`@tenant` field `{}` of `{model}` is not a `uuid`, `string`, `int` or `long`",
                f.name
            ),
            f.span,
            "a tenant is an identifier: use one of those four types",
        );
    }
    if f.optional && !principal {
        err(
            diags,
            format!("`@tenant` field `{}` of `{model}` must be required", f.name),
            f.span,
            "only the model of the principal may have an optional tenant: a caller can have none",
        );
    }
}

/// The tenant-scoped models a type reaches, and the field that makes each one so.
fn scoped<'p>(prog: &'p Program<'_>, start: Vec<usize>) -> Vec<(usize, &'p RField)> {
    closure(prog, start)
        .into_iter()
        .filter(|i| prog.decls[*i].kind == DKind::Model)
        .filter_map(|i| {
            prog.decls[i]
                .res
                .fields
                .iter()
                .find(|f| f.tenant)
                .map(|f| (i, f))
        })
        .collect()
}

fn decl_indexes(types: &[&Type]) -> Vec<usize> {
    let mut ids: Vec<DeclId> = Vec::new();
    for t in types {
        t.collect_decls(&mut ids);
    }
    ids.into_iter().map(DeclId::index).collect()
}

/// The service checks: the endpoints that load or return a tenant-scoped model.
pub(in crate::semantic) fn check_services(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    for s in &prog.services {
        for ep in &s.endpoints {
            let mut start = decl_indexes(&ep.rules.facts.iter().map(|f| &f.ty).collect::<Vec<_>>());
            start.extend(decl_indexes(&ep.success_ty.iter().collect::<Vec<_>>()));
            start.extend(ep.errors.iter().map(|(_, d)| *d));
            let reached = scoped(prog, start);
            let Some((_, field)) = reached.first() else {
                continue;
            };
            let model = reached
                .first()
                .map_or("", |(i, _)| prog.decls[*i].name.as_str());
            if let Some(problem) = problem(prog, s.pkg, ep.auth.kind, &reached) {
                err(
                    diags,
                    format!(
                        "endpoint `{}` loads or returns `{model}`, which is tenant-scoped (`{}`): {problem}",
                        ep.name, field.name
                    ),
                    ep.span,
                    "the caller's tenant comes from the principal: declare `principal User`, give `User` a `@tenant` field of the same type, and put `@auth` on the endpoint",
                );
            }
        }
    }
}

/// What is missing for the endpoint to be tenant-scoped, if anything.
fn problem(
    prog: &Program<'_>,
    pkg: usize,
    auth: veld_ir::AuthKind,
    reached: &[(usize, &RField)],
) -> Option<String> {
    let Some(principal) = prog.principals.get(&pkg) else {
        return Some("the package declares no principal".to_string());
    };
    let pd = &prog.decls[principal.decl];
    let Some(own) = pd.res.fields.iter().find(|f| f.tenant) else {
        return Some(format!(
            "the principal `{}` has no `@tenant` field",
            pd.name
        ));
    };
    if let Some((i, f)) = reached.iter().find(|(_, f)| f.ty != own.ty) {
        return Some(format!(
            "the tenant of `{}` is not the type of the principal's (`{}`)",
            prog.decls[*i].name, f.name
        ));
    }
    (auth == veld_ir::AuthKind::None).then(|| "the endpoint has no auth".to_string())
}

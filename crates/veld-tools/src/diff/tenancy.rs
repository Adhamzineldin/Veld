//! `@tenant` in `veld diff` (`spec/platform.md` section 3).
//!
//! A tenant-scoped value of another tenant is never sent and a fact of another tenant is not
//! found, so every change here moves what some callers can reach, and is judged by what a caller
//! or a consumer built against the old contract relied on:
//!
//! | Change | Judged |
//! |---|---|
//! | `@tenant` added to a field | possibly breaking: the field is now set by the server, a request that carried it is ignored, and values of other tenants are no longer found or sent |
//! | `@tenant` removed | possibly breaking: a consumer may rely on the isolation the old contract promised |
//! | `@tenant` moved to another field of the model | possibly breaking: the tenant is another value |
//!
//! Only a field a declaration wrote itself is compared; an inherited one follows its parent.

use std::collections::BTreeMap;

use veld_ir::{Decl, PackageKind};

use super::{Change, ChangeKind, Ctx};

fn with_fields(ir: &veld_ir::Ir) -> BTreeMap<&str, &Decl> {
    ir.decls
        .iter()
        .filter(|d| ir.package(d.package).kind != PackageKind::Prelude && !d.fields().is_empty())
        .map(|d| (d.qualified.as_str(), d))
        .collect()
}

/// The wire name of the tenant field a declaration wrote itself.
fn own_tenant(d: &Decl) -> Option<&str> {
    d.fields()
        .iter()
        .find(|f| f.tenant && f.inherited_from.is_none())
        .map(|f| f.name.as_str())
}

pub(super) fn diff_tenancy(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (with_fields(ctx.old), with_fields(ctx.new));
    for (q, o) in &old {
        let Some(n) = new.get(q) else { continue };
        match (own_tenant(o), own_tenant(n)) {
            (None, Some(f)) => out.push(Change::possibly_breaking(
                ChangeKind::TenantAdded,
                q,
                format!(
                    "field `{f}` is now `@tenant`: the server sets it, and values of other tenants are no longer found or sent"
                ),
            )),
            (Some(f), None) => out.push(Change::possibly_breaking(
                ChangeKind::TenantRemoved,
                q,
                format!("field `{f}` is no longer `@tenant`: the isolation the old contract promised is gone"),
            )),
            (Some(a), Some(b)) if a != b => out.push(Change::possibly_breaking(
                ChangeKind::TenantChanged,
                q,
                format!("the tenant of `{}` moved from `{a}` to `{b}`", n.name),
            )),
            _ => {}
        }
    }
}

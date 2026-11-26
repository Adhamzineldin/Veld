//! `@visible` in `veld diff` (`spec/platform.md` section 2).
//!
//! The field is optional, so a client already handles its absence and no type changes. What a
//! caller built against the old contract can notice is that a value it used to receive may stop
//! coming:
//!
//! | Change | Judged |
//! |---|---|
//! | `@visible` added to a field | possibly breaking: callers that received it may not any more |
//! | the expression changed | possibly breaking: the set of callers that receive it moved |
//! | `@visible` removed | compatible: more callers receive it |
//!
//! Only a field a declaration wrote itself is compared; an inherited one follows its parent.

use std::collections::BTreeMap;

use veld_ir::{Decl, Field, PackageKind};

use super::{Change, ChangeKind, Ctx};

fn with_fields(ir: &veld_ir::Ir) -> BTreeMap<&str, &Decl> {
    ir.decls
        .iter()
        .filter(|d| ir.package(d.package).kind != PackageKind::Prelude && !d.fields().is_empty())
        .map(|d| (d.qualified.as_str(), d))
        .collect()
}

pub(super) fn diff_visibility(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (with_fields(ctx.old), with_fields(ctx.new));
    for (q, o) in &old {
        let Some(n) = new.get(q) else { continue };
        for of in o.fields().iter().filter(|f| f.inherited_from.is_none()) {
            if let Some(nf) = n.fields().iter().find(|f| f.name == of.name) {
                field(q, of, nf, out);
            }
        }
    }
}

fn field(q: &str, o: &Field, n: &Field, out: &mut Vec<Change>) {
    match (&o.visible, &n.visible) {
        (None, Some(_)) => out.push(Change::possibly_breaking(
            ChangeKind::VisibleAdded,
            q,
            format!(
                "field `{}` is now `@visible`: callers that received it may not any more",
                n.name
            ),
        )),
        (Some(_), None) => out.push(Change::non_breaking(
            ChangeKind::VisibleRemoved,
            q,
            format!(
                "field `{}` is no longer `@visible`: more callers receive it",
                n.name
            ),
        )),
        (Some(a), Some(b)) if a != b || o.visible_principal != n.visible_principal => {
            out.push(Change::possibly_breaking(
                ChangeKind::VisibleChanged,
                q,
                format!(
                    "the `@visible` condition of field `{}` changed: the set of callers that receive it moved",
                    n.name
                ),
            ));
        }
        _ => {}
    }
}

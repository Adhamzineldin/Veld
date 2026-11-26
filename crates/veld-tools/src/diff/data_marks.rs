//! `@pii` and `@secret` in `veld diff` (`spec/rules.md` section 19.3).
//!
//! Neither annotation changes a byte on the wire, a validation or a signature, so a caller built
//! against the old contract is not affected by adding or removing either: every change here is
//! non-breaking. They are listed so that a reviewer sees that a field now holds personal data, or
//! that it no longer is marked.

use std::collections::BTreeMap;

use veld_ir::{Decl, Field, PackageKind};

use super::{Change, ChangeKind, Ctx};

/// Every declaration with fields (models, errors and events), by qualified name.
fn with_fields(ir: &veld_ir::Ir) -> BTreeMap<&str, &Decl> {
    ir.decls
        .iter()
        .filter(|d| ir.package(d.package).kind != PackageKind::Prelude && !d.fields().is_empty())
        .map(|d| (d.qualified.as_str(), d))
        .collect()
}

pub(super) fn diff_data_marks(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (with_fields(ctx.old), with_fields(ctx.new));
    for (q, o) in &old {
        let Some(n) = new.get(q) else { continue };
        for of in o.fields().iter().filter(|f| f.inherited_from.is_none()) {
            if let Some(nf) = n.fields().iter().find(|f| f.name == of.name) {
                field_marks(q, of, nf, out);
            }
        }
    }
}

fn field_marks(q: &str, o: &Field, n: &Field, out: &mut Vec<Change>) {
    let marks = [
        (
            o.pii,
            n.pii,
            "@pii",
            ChangeKind::PiiAdded,
            ChangeKind::PiiRemoved,
        ),
        (
            o.secret,
            n.secret,
            "@secret",
            ChangeKind::SecretAdded,
            ChangeKind::SecretRemoved,
        ),
    ];
    for (was, is, word, added, removed) in marks {
        if was == is {
            continue;
        }
        let (kind, text) = if is {
            (added, format!("field `{}` is now `{word}`", n.name))
        } else {
            (removed, format!("field `{}` is no longer `{word}`", n.name))
        };
        out.push(Change::non_breaking(
            kind,
            q,
            format!("{text}; nothing on the wire changes"),
        ));
    }
}

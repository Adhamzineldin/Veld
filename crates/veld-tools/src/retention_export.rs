//! `veld export retention` (`spec/platform.md` section 23): the data inventory of a workspace as
//! Markdown, for people who do not read contracts.
//!
//! Per package, in name order, every model with a `@pii` or `@secret` field, in name order: those
//! fields, how long the model is kept and what happens then (or `kept without limit`), and where
//! it travels, which is what `Ir::exposure_index` says: the endpoints whose answers or declared
//! errors can contain it and the events that can. The document is deterministic. The user owns
//! the file it is written to.

use std::fmt::Write as _;

use veld_ir::exposure::Carrier;
use veld_ir::retention::RetainThen;
use veld_ir::{Decl, DeclKind, Ir};

use crate::lint::is_linted_package;

/// The text of the data inventory.
#[must_use]
pub fn retention_report(ir: &Ir) -> String {
    let index = ir.exposure_index();
    let mut packages: Vec<_> = ir
        .packages
        .iter()
        .filter(|p| is_linted_package(p.kind))
        .collect();
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = String::from(
        "# Data inventory\n\nThe models that hold personal data (`@pii`) or secrets (`@secret`), how long each is kept, and where it travels. Written by `veld export retention`.\n",
    );
    let mut any = false;
    for p in packages {
        let mut models: Vec<&Decl> = ir
            .decls
            .iter()
            .filter(|d| d.package == p.id && holds_data(d))
            .collect();
        if models.is_empty() {
            continue;
        }
        any = true;
        models.sort_by(|a, b| a.name.cmp(&b.name));
        let _ = write!(out, "\n## Package `{}`\n", p.name);
        for d in models {
            model_section(&mut out, ir, d, index.get(&d.id).map_or(&[][..], Vec::as_slice));
        }
    }
    if !any {
        out.push_str("\nNo model holds a `@pii` or `@secret` field.\n");
    }
    out
}

fn holds_data(d: &Decl) -> bool {
    matches!(d.kind, DeclKind::Model(_)) && d.fields().iter().any(|f| f.pii || f.secret)
}

fn names(d: &Decl, pick: impl Fn(&veld_ir::Field) -> bool) -> String {
    d.fields()
        .iter()
        .filter(|f| pick(f))
        .map(|f| format!("`{}`", f.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn model_section(out: &mut String, ir: &Ir, d: &Decl, carriers: &[Carrier]) {
    let _ = write!(out, "\n### `{}`\n\n", d.name);
    let pii = names(d, |f| f.pii);
    if !pii.is_empty() {
        let _ = writeln!(out, "- Personal data: {pii}");
    }
    let secret = names(d, |f| f.secret);
    if !secret.is_empty() {
        let _ = writeln!(out, "- Secrets: {secret}");
    }
    let _ = writeln!(out, "- Kept: {}", kept(d));
    if carriers.is_empty() {
        out.push_str("- Travels: nowhere: no endpoint or event can carry it\n");
        return;
    }
    out.push_str("- Travels:\n");
    for c in carriers {
        match c {
            Carrier::Endpoint { service, endpoint } => {
                let s = ir.service(*service);
                if let Some(ep) = s.endpoints.get(*endpoint) {
                    let _ = writeln!(
                        out,
                        "  - `{} {}{}` (`{}.{}`)",
                        ep.method.as_str(),
                        s.prefix,
                        ep.path,
                        s.name,
                        ep.name
                    );
                }
            }
            Carrier::Event(id) => {
                let _ = writeln!(out, "  - the event `{}`", ir.decl(*id).name);
            }
        }
    }
}

fn kept(d: &Decl) -> String {
    let DeclKind::Model(m) = &d.kind else {
        return "kept without limit".to_owned();
    };
    let Some(r) = &m.retain else {
        return "kept without limit".to_owned();
    };
    let days = r.after_ms / 86_400_000;
    let then = match r.then {
        RetainThen::Delete => "deleted".to_owned(),
        RetainThen::Anonymize => {
            let fields = names(d, |f| f.pii);
            format!("anonymized ({fields})")
        }
    };
    format!(
        "{days} day{} after `{}`, then {then}",
        if days == 1 { "" } else { "s" },
        r.since
    )
}

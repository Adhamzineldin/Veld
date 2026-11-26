//! `pii-without-auth` (`L0015`) and `secret-in-output` (`L0016`), `spec/rules.md` section 19.3.
//!
//! Both ask what an answer can contain, which is `Ir::output_decls` and `Ir::exposure_index`: the
//! success value, the items of a stream and the declared errors, and the events. A parameter or a
//! fact is not output.

use veld_ir::exposure::Carrier;
use veld_ir::{Decl, DeclId, Field, Ir, ServiceId};

use super::is_linted;
use super::rules::{Raw, push};

/// How many field names a message lists before it says "and N more".
const LISTED: usize = 4;

pub(super) fn run(ir: &Ir, out: &mut Vec<Raw>) {
    pii_without_auth(ir, out);
    secret_in_output(ir, out);
    pii_without_retention(ir, out);
}

/// `pii-without-retention` (`L0017`, `spec/platform.md` section 23): a model that holds personal
/// data, its own or inherited, and says nothing about how long it is kept. `@retain` is not
/// inherited, so a child of such a model needs its own.
fn pii_without_retention(ir: &Ir, out: &mut Vec<Raw>) {
    for d in ir
        .decls
        .iter()
        .filter(|d| is_linted(ir.package(d.package).kind))
    {
        let veld_ir::DeclKind::Model(m) = &d.kind else {
            continue;
        };
        if m.retain.is_some() {
            continue;
        }
        let pii: Vec<String> = d
            .fields()
            .iter()
            .filter(|f| f.pii)
            .map(|f| format!("`{}`", f.name))
            .collect();
        if pii.is_empty() {
            continue;
        }
        push(
            out,
            "pii-without-retention",
            d.name.clone(),
            format!(
                "model `{}` holds personal data ({}) and has no `@retain`: say how long it is kept, or `kept without limit` shows in `veld export retention`",
                d.name,
                pii.join(", ")
            ),
            &d.origin,
        );
    }
}

/// The fields a declaration wrote itself (an inherited field belongs to the parent).
fn own_fields(d: &Decl) -> impl Iterator<Item = &Field> {
    d.fields().iter().filter(|f| f.inherited_from.is_none())
}

fn pii_without_auth(ir: &Ir, out: &mut Vec<Raw>) {
    for s in ir
        .services
        .iter()
        .filter(|s| is_linted(ir.package(s.package).kind))
    {
        for ep in s
            .endpoints
            .iter()
            .filter(|e| e.auth.kind == veld_ir::AuthKind::None)
        {
            let names: Vec<String> = ir
                .output_decls(ep)
                .into_iter()
                .map(|id| ir.decl(id))
                .flat_map(|d| {
                    own_fields(d)
                        .filter(|f| f.pii)
                        .map(|f| format!("{}.{}", d.name, f.name))
                        .collect::<Vec<_>>()
                })
                .collect();
            if names.is_empty() {
                continue;
            }
            push(
                out,
                "pii-without-auth",
                format!("{}.{}", s.name, ep.name),
                format!(
                    "endpoint `{}` has no auth and can answer with personal data ({}): add `@auth` or take the `@pii` fields out of what it returns",
                    ep.name,
                    listed(&names)
                ),
                &ep.origin,
            );
        }
    }
}

fn listed(names: &[String]) -> String {
    let shown: Vec<String> = names
        .iter()
        .take(LISTED)
        .map(|n| format!("`{n}`"))
        .collect();
    let more = names.len().saturating_sub(LISTED);
    if more == 0 {
        shown.join(", ")
    } else {
        format!("{} and {more} more", shown.join(", "))
    }
}

fn secret_in_output(ir: &Ir, out: &mut Vec<Raw>) {
    let index = ir.exposure_index();
    for d in ir
        .decls
        .iter()
        .filter(|d| is_linted(ir.package(d.package).kind))
    {
        let Some(carriers) = index.get(&d.id) else {
            continue;
        };
        let Some(first) = carriers.first() else {
            continue;
        };
        for f in own_fields(d).filter(|f| f.secret) {
            push(
                out,
                "secret-in-output",
                format!("{}.{}", d.name, f.name),
                format!(
                    "`@secret` field `{}.{}` can be sent: {}; return a separate model without it",
                    d.name,
                    f.name,
                    carrier_text(ir, *first, carriers.len())
                ),
                &f.origin,
            );
        }
    }
}

fn carrier_text(ir: &Ir, first: Carrier, total: usize) -> String {
    let what = match first {
        Carrier::Endpoint { service, endpoint } => endpoint_name(ir, service, endpoint),
        Carrier::Event(id) => event_name(ir, id),
    };
    if total > 1 {
        format!("{what} and {} more", total - 1)
    } else {
        what
    }
}

fn endpoint_name(ir: &Ir, service: ServiceId, endpoint: usize) -> String {
    let s = ir.service(service);
    let ep = s.endpoints.get(endpoint).map_or("?", |e| e.name.as_str());
    format!("an answer of `{}.{ep}`", s.name)
}

fn event_name(ir: &Ir, id: DeclId) -> String {
    format!("the event `{}`", ir.decl(id).name)
}

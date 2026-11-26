//! Sagas in `veld diff` (`spec/platform.md` section 17).
//!
//! A caller sees a saga as an ordinary `post` endpoint with a required idempotency key, so what a
//! change does to it is what it does to the behaviour behind that endpoint:
//!
//! | Change | Verdict | Why |
//! |---|---|---|
//! | a saga endpoint is added or removed | as any endpoint | it is an endpoint added or removed |
//! | an endpoint becomes a saga (`SagaAdded`) | possibly breaking | the same endpoint, another behaviour: its work is now steps that can be undone, and it can answer `SagaFailed` |
//! | a saga becomes an ordinary endpoint (`SagaRemoved`) | possibly breaking | nothing undoes the work of a failure any more |
//! | the step list, a call, an argument, a compensation or the timeout changes (`SagaChanged`) | possibly breaking | callers see the same endpoint and the same types, but another behaviour |
//!
//! `SagaFailed` comes with the saga and is not reported again as an error added or removed. The
//! required idempotency key and the `Conflict` of a saga that becomes one are reported by the
//! idempotency rows.

use veld_ir::{Endpoint, Ir, Saga, SagaCall, SagaValue};

use super::expr_text::expr_text;
use super::{Change, ChangeKind, Ctx};

/// What changed in the saga of one endpoint.
pub(super) fn diff_saga(ctx: &Ctx<'_>, s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    match (&o.saga, &n.saga) {
        (None, Some(_)) => out.push(Change::possibly_breaking(
            ChangeKind::SagaAdded,
            s,
            "the endpoint is now a saga: the same endpoint, another behaviour",
        )),
        (Some(_), None) => out.push(Change::possibly_breaking(
            ChangeKind::SagaRemoved,
            s,
            "the endpoint is no longer a saga: a failure no longer undoes the steps",
        )),
        (Some(a), Some(b)) => {
            let (old, new) = (describe(ctx.old, a), describe(ctx.new, b));
            if old != new {
                out.push(Change::possibly_breaking(
                    ChangeKind::SagaChanged,
                    s,
                    "the steps, a call or the timeout of the saga changed: the same endpoint, another behaviour",
                ));
            }
        }
        (None, None) => {}
    }
}

/// The saga as text, with services and declarations by name, so two IRs compare.
fn describe(ir: &Ir, saga: &Saga) -> String {
    let mut parts: Vec<String> = Vec::new();
    for step in &saga.steps {
        let undo = step
            .compensate
            .as_ref()
            .map_or_else(|| "-".to_owned(), |c| call_text(ir, c));
        parts.push(format!(
            "{}={} / {undo}",
            step.name,
            call_text(ir, &step.call)
        ));
    }
    parts.push(format!("timeout {}", saga.timeout_ms));
    let mut passes: Vec<&str> = saga
        .passes
        .iter()
        .map(|d| ir.decl(*d).qualified.as_str())
        .collect();
    passes.sort_unstable();
    parts.push(format!("passes {}", passes.join(",")));
    parts.join("\n")
}

fn call_text(ir: &Ir, c: &SagaCall) -> String {
    let service = ir.services.get(c.service.index());
    let (pkg, name) = service.map_or(("?", "?"), |s| {
        let pkg = ir
            .packages
            .get(s.package.index())
            .map_or("?", |p| p.name.as_str());
        (pkg, s.name.as_str())
    });
    let args: Vec<String> = c
        .args
        .iter()
        .map(|a| format!("{}: {}", a.name, value_text(&a.value)))
        .collect();
    format!("{pkg}/{name}.{}({})", c.endpoint, args.join(", "))
}

fn value_text(v: &SagaValue) -> String {
    match v {
        SagaValue::Expr(e) => expr_text(e),
        SagaValue::Object { object } => {
            let items: Vec<String> = object
                .iter()
                .map(|f| format!("{}: {}", f.wire, value_text(&f.value)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
    }
}

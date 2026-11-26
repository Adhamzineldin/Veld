//! `veld diff` and sagas (`spec/platform.md` section 17): every row of the table in `saga.rs`.

use super::ChangeKind::{self, *};
use super::Severity::{self, NonBreaking as N, PossiblyBreaking as P};
use super::diff;
use crate::testutil::ir;

fn kinds(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| (c.kind, c.severity))
        .collect();
    v.sort();
    v
}

const BASE: &str = "model Req { id: uuid }\nmodel Pay { id: uuid }\nmodel Out { id: uuid }\n\
service Pays at \"/pays\" {\n  @idempotent\n  post \"/charge\" charge(body: Req) -> Pay\n  @idempotent\n  post \"/refund/{id}\" refund(id: uuid) -> 204\n  @idempotent\n  post \"/other\" other(body: Req) -> Pay\n  @idempotent\n  post \"/fin\" fin(id: uuid) -> Out\n}\n";

fn saga(body: &str) -> String {
    format!(
        "{BASE}service Shop at \"/shop\" {{\n  saga post \"/\" buy(body: Req) -> Out {{\n{body}\n  }}\n}}\n"
    )
}

const STEPS: &str = "    step pay = Pays.charge(body: body) compensate Pays.refund(id: pay.id)\n    step done = Pays.fin(id: pay.id)\n    timeout minutes(5)";

#[test]
fn a_saga_that_does_not_change_changes_nothing() {
    assert!(kinds(&saga(STEPS), &saga(STEPS)).is_empty());
}

#[test]
fn a_changed_step_call_compensation_or_timeout_is_possibly_breaking() {
    let old = saga(STEPS);
    for new in [
        STEPS.replace("Pays.charge", "Pays.other"),
        STEPS.replace(" compensate Pays.refund(id: pay.id)", ""),
        STEPS.replace("minutes(5)", "minutes(6)"),
        STEPS.replace("fin(id: pay.id)", "fin(id: body.id)"),
        STEPS.replace("step done", "step finish"),
    ] {
        assert_eq!(kinds(&old, &saga(&new)), [(SagaChanged, P)], "{new}");
    }
}

#[test]
fn an_endpoint_that_becomes_a_saga_or_stops_being_one_is_possibly_breaking() {
    let plain = format!(
        "{BASE}service Shop at \"/shop\" {{\n  @idempotent(required: true)\n  post \"/\" buy(body: Req) -> Out\n}}\n"
    );
    let sagas = saga(STEPS);
    // `SagaFailed` comes with the saga: it is not an error added; the key was required already.
    assert_eq!(kinds(&plain, &sagas), [(SagaAdded, P)]);
    assert_eq!(kinds(&sagas, &plain), [(SagaRemoved, P)]);
}

#[test]
fn a_new_saga_endpoint_is_an_endpoint_added() {
    let old = format!("{BASE}service Shop at \"/shop\" {{ get \"/ping\" ping() -> 204 }}\n");
    let new = format!(
        "{BASE}service Shop at \"/shop\" {{\n  get \"/ping\" ping() -> 204\n  saga post \"/\" buy(body: Req) -> Out {{\n{STEPS}\n  }}\n}}\n"
    );
    assert_eq!(kinds(&old, &new), [(EndpointAdded, N)]);
}

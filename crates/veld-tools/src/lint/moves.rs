//! `post-moves-state-as-created` (`L0018`, `spec/rules.md` section 10): a `post` whose
//! `transition` answers the fact it moves, and still answers with the default `201`.
//!
//! The move changes something that exists; `200` says that, `201 Created` says a resource was
//! made. The IR does not remember whether the author wrote `201`, so a contract that means it
//! turns the rule off in `[lint]`.

use veld_ir::{Endpoint, Ir, Method, Service, Type};

use super::rules::{Raw, push};

/// The status a `post` answers when the contract does not say.
const DEFAULT_POST_STATUS: u16 = 201;

pub(super) fn run(ir: &Ir, services: &[&Service], out: &mut Vec<Raw>) {
    for s in services {
        for e in s.endpoints.iter().filter(|e| moves_and_creates(e)) {
            let what = match &e.success.ty {
                Some(Type::Decl(d)) => ir.decl(*d).name.clone(),
                _ => "T".to_owned(),
            };
            push(
                out,
                "post-moves-state-as-created",
                format!("{}.{}", s.name, e.name),
                format!(
                    "`post` endpoint `{}` moves a state and answers `201`, which says a resource was created; write `-> 200 {what}` (or turn the rule off in `[lint]`)",
                    e.name
                ),
                &e.origin,
            );
        }
    }
}

fn moves_and_creates(e: &Endpoint) -> bool {
    if e.method != Method::Post || e.success.status != DEFAULT_POST_STATUS || e.success.stream {
        return false;
    }
    let (Some(t), Some(answer)) = (&e.rules.transition, &e.success.ty) else {
        return false;
    };
    e.rules
        .facts
        .iter()
        .any(|f| f.name == t.fact && matches!(f.ty, Type::Decl(_)) && &f.ty == answer)
}

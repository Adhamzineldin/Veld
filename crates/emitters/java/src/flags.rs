//! Flags in the generated code (`spec/platform.md` section 11, `spec/targets/java.md` section 16.2):
//! which flags an endpoint can reach, and the statements that carry one. The readings are the
//! runtime's (`runtime/Flags.java`).

use std::collections::BTreeSet;

use veld_ir::{DeclKind, Endpoint, Ir, Service};

use crate::tree::Tree;

/// The flags of the invariants of the declarations the tree emits: a request, a result, an error
/// or an event can carry a flagged rule, so every endpoint reads them.
#[must_use]
pub fn invariant_flags(tree: &Tree<'_>) -> BTreeSet<String> {
    tree.decls()
        .iter()
        .flat_map(|id| tree.ir.decl(*id).rules().iter())
        .filter_map(|r| r.flag.clone())
        .collect()
}

/// The flags an endpoint can reach, sorted: its own `require` and `ensure`, the clauses of the
/// policies its `allow` lines name, the flagged edges of its `transition` from a source to the
/// target, and every flagged invariant of the tree.
#[must_use]
pub fn endpoint_flags(tree: &Tree<'_>, ep: &Endpoint) -> Vec<String> {
    let ir = tree.ir;
    let mut out = invariant_flags(tree);
    out.extend(ep.rules.requires.iter().filter_map(|r| r.flag.clone()));
    out.extend(ep.rules.ensures.iter().filter_map(|e| e.flag.clone()));
    for allow in &ep.rules.allows {
        if let Some(policy) = ir.policies.get(allow.policy.index()) {
            out.extend(policy.clauses.iter().filter_map(|c| c.flag.clone()));
        }
    }
    if let Some(t) = &ep.rules.transition
        && let DeclKind::Enum(machine) = &ir.decl(t.decl).kind
    {
        out.extend(
            machine
                .flows
                .iter()
                .filter(|f| f.to == t.to && t.from.contains(&f.from))
                .filter_map(|f| f.flag.clone()),
        );
    }
    out.into_iter().collect()
}

/// Whether the decision of `can<Endpoint>` depends on a flag: a flagged clause of a policy its `allow`
/// lines name, or a flagged edge of its `transition`.
#[must_use]
pub fn can_depends_on_flags(ir: &Ir, ep: &Endpoint) -> bool {
    let policy = ep.rules.allows.iter().any(|a| {
        ir.policies
            .get(a.policy.index())
            .is_some_and(|p| p.clauses.iter().any(|c| c.flag.is_some()))
    });
    let edge = ep.rules.transition.as_ref().is_some_and(|t| {
        matches!(&ir.decl(t.decl).kind, DeclKind::Enum(m)
            if m.flows.iter().any(|f| f.flag.is_some() && f.to == t.to && t.from.contains(&f.from)))
    });
    policy || edge
}

/// Whether some endpoint of the service can reach a flag (its hooks then take the provider).
#[must_use]
pub fn service_uses(tree: &Tree<'_>, svc: &Service) -> bool {
    svc.endpoints
        .iter()
        .any(|e| !endpoint_flags(tree, e).is_empty())
}

/// Whether any of the services' contract uses a flag at all.
#[must_use]
pub fn ir_uses_flags(ir: &Ir) -> bool {
    ir.decls
        .iter()
        .any(|d| d.rules().iter().any(|r| r.flag.is_some()))
        || ir
            .policies
            .iter()
            .any(|p| p.clauses.iter().any(|c| c.flag.is_some()))
        || ir.services.iter().any(|s| {
            s.endpoints.iter().any(|e| {
                e.rules.requires.iter().any(|r| r.flag.is_some())
                    || e.rules.ensures.iter().any(|r| r.flag.is_some())
            })
        })
        || ir.decls.iter().any(
            |d| matches!(&d.kind, DeclKind::Enum(e) if e.flows.iter().any(|f| f.flag.is_some())),
        )
}

/// The Java text of a flag name.
#[must_use]
pub fn quoted(name: &str) -> String {
    crate::util::java_str(name)
}

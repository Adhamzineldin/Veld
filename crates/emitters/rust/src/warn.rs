//! Warnings of the generator (`Generator::warnings`).

use veld_codegen::{GenDiagnostic, GenRequest};
use veld_ir::Origin;

use crate::tree::Tree;

/// `W0904` (`spec/rules.md` section 5.3): the target option `validate = false` turns request
/// validation off, and with it the rules on what callers send. Reported when the target emits a
/// declaration or an endpoint with rules, naming the first and pointing at its first rule. Empty
/// when validation is on or the contract has no rules.
#[must_use]
pub fn validation_off(req: &GenRequest<'_>, validate: Option<bool>) -> Vec<GenDiagnostic> {
    if validate != Some(false) {
        return Vec::new();
    }
    let mut ruled: Vec<(String, Origin)> = Vec::new();
    for group in [&req.services, &req.consumes] {
        if group.is_empty() {
            continue;
        }
        let tree = Tree::new(req.ir, "crate", "src", (*group).clone(), false);
        for id in tree.decls() {
            let d = req.ir.decl(*id);
            let named = ruled.iter().any(|(name, _)| *name == d.qualified);
            if let (Some(rule), false) = (d.rules().first(), named) {
                ruled.push((d.qualified.clone(), rule.origin.clone()));
            }
        }
        for sid in group.iter() {
            let svc = req.ir.service(*sid);
            for ep in &svc.endpoints {
                if let Some(origin) = ep.rules.first_origin() {
                    ruled.push((format!("{}.{}", svc.name, ep.name), origin.clone()));
                }
            }
        }
    }
    let Some((first, origin)) = ruled.first() else {
        return Vec::new();
    };
    let more = match ruled.len() {
        1 => String::new(),
        n => format!(" and {} more", n - 1),
    };
    vec![GenDiagnostic {
        code: "W0904",
        message: format!(
            "request validation is off (`validate = false`), so the rules of `{first}`{more} are not enforced on requests, WebSocket messages or by the client's local check"
        ),
        origin: Some(origin.clone()),
    }]
}

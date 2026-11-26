//! The clients of a target by key: which client class each service of the target has and where
//! it lives. `runtime.ts` builds them all with `createClients(runtime, options?)`
//! (`spec/runtime-config.md` section 3.1).

use std::collections::BTreeMap;

use veld_codegen::naming;

use crate::runtime_config::ClientEntry;
use crate::tree::Tree;

/// The aggregate keys: the camelCase service name, qualified by the package when two packages
/// of one target declare a service of the same name.
#[must_use]
pub fn entries(tree: &Tree<'_>) -> Vec<ClientEntry> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for sid in &tree.services {
        *counts
            .entry(naming::camel(&tree.ir.service(*sid).name))
            .or_default() += 1;
    }
    let mut out = Vec::new();
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        let base = naming::camel(&svc.name);
        let key = if counts.get(&base).copied().unwrap_or(0) > 1 {
            let pkg = tree.ir.package(svc.package).name.trim_start_matches('@');
            let pkg = naming::camel(&pkg.replace('/', "-"));
            format!("{pkg}{}", naming::pascal(&svc.name))
        } else {
            base
        };
        out.push(ClientEntry {
            key,
            class: format!("{}Client", naming::pascal(&svc.name)),
            module: tree.client_module(svc.package),
            service: *sid,
        });
    }
    out
}

/// The `[key, class]` pairs, for the README.
#[must_use]
pub fn keys(tree: &Tree<'_>) -> Vec<(String, String)> {
    entries(tree)
        .into_iter()
        .map(|e| (e.key, e.class))
        .collect()
}

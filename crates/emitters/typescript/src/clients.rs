//! `clients.ts`: `createClients(opts)`, one client per service of the target.
//!
//! The client barrel re-exports it, so `import { createClients } from '@veld/web'` gives
//! `{ orders: OrdersClient, iam: IamClient }` from one set of `ClientOptions`.

use std::collections::BTreeMap;

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};

use crate::tree::{FileCtx, Tree};
use crate::util::{add_file, prop_key};

/// Root-relative module path (no extension) of the file.
pub const MODULE: &str = "clients";

/// One service of the aggregate.
struct Entry {
    key: String,
    class: String,
}

/// The aggregate keys: the camelCase service name, qualified by the package when two packages
/// of one target declare a service of the same name.
fn entries(tree: &Tree<'_>) -> Vec<(Entry, String)> {
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
        let class = format!("{}Client", naming::pascal(&svc.name));
        out.push((Entry { key, class }, tree.client_module(svc.package)));
    }
    out
}

/// The `[key, class]` pairs, for the README.
#[must_use]
pub fn keys(tree: &Tree<'_>) -> Vec<(String, String)> {
    entries(tree)
        .into_iter()
        .map(|(e, _)| (e.key, e.class))
        .collect()
}

/// Writes `clients.ts` and returns its module path.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<String, Vec<GenDiagnostic>> {
    let list = entries(tree);
    let mut fc = FileCtx::new(tree, MODULE);
    let opts = fc.rt("ClientOptions");
    for (e, module) in &list {
        fc.import(module, &e.class, true);
    }
    let mut w = CodeWriter::new();
    w.line("/** The clients of every service, by service name. */");
    w.open("export interface Clients");
    for (e, _) in &list {
        w.line(format!("{}: {};", prop_key(&e.key), e.class));
    }
    w.close();
    w.blank();
    w.line(
        "/** Options per service, merged over the shared options (`baseUrl` is the usual one). */",
    );
    w.line(format!(
        "export type ClientOverrides = {{ [K in keyof Clients]?: {opts} }};"
    ));
    w.blank();
    w.line("/** One client for every service, all created from the same options. */");
    w.open(format!(
        "export function createClients(opts: {opts} = {{}}, overrides: ClientOverrides = {{}}): Clients"
    ));
    w.open_with("return", " {");
    for (e, _) in &list {
        let key = prop_key(&e.key);
        w.line(format!(
            "{key}: new {}({{ ...opts, ...overrides.{} }}),",
            e.class, e.key
        ));
    }
    w.close_with("};");
    w.close();
    add_file(files, &format!("{MODULE}.ts"), fc.finish(&w.finish()))?;
    Ok(MODULE.to_string())
}

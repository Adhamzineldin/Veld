//! Message catalogs (`spec/platform.md` section 22, `spec/targets/typescript.md` section 5l).
//!
//! A member with catalogs gets `messages.ts` (`<package>/messages.ts` in a client): the catalogs as
//! data, sorted, and the `messageKey` of every rule id of the tree, from `Ir::violation_key`. A
//! server also gets the four fixed keys. The keys are never built from an id in generated code.

use std::collections::BTreeSet;

use veld_codegen::CodeWriter;
use veld_codegen::naming;
use veld_codegen::{FileSet, GenDiagnostic};
use veld_ir::message_keys::{NOT_PERMITTED, TRANSITION, TRANSITION_NOT_DUE, VERSION};
use veld_ir::{DeclId, Ir, PackageId};

use crate::tree::Tree;
use crate::util::{HEADER, add_file, prop_key, ts_str};

/// The path of the catalog module of `package`, root-relative, without extension.
#[must_use]
pub fn module(tree: &Tree<'_>, package: PackageId) -> String {
    format!("{}messages", tree.pkg_prefix(package))
}

/// The packages of the tree's services that have catalogs, in order of first appearance.
fn packages(tree: &Tree<'_>) -> Vec<PackageId> {
    tree.packages()
        .into_iter()
        .filter(|p| !tree.ir.package(*p).messages.is_empty())
        .collect()
}

/// Every rule id a violation of the tree can carry: the invariants of the declarations, the
/// computed fields, the tenant rules, and the statements, guards and policies of the endpoints.
fn rule_ids(tree: &Tree<'_>) -> BTreeSet<String> {
    let ir = tree.ir;
    let mut ids = BTreeSet::new();
    let decls: Vec<DeclId> = tree.decls().to_vec();
    for id in decls {
        let d = ir.decl(id);
        ids.extend(d.rules().iter().map(|r| r.id.clone()));
        for f in d.fields() {
            ids.extend(f.derived_id.clone());
            ids.extend(f.tenant_id.clone());
        }
    }
    for sid in &tree.services {
        for ep in &ir.service(*sid).endpoints {
            let rules = &ep.rules;
            ids.extend(rules.requires.iter().map(|r| r.id.clone()));
            ids.extend(rules.ensures.iter().map(|r| r.id.clone()));
            ids.extend(rules.transition.iter().map(|t| t.id.clone()));
            ids.extend(rules.version.iter().map(|v| v.id.clone()));
            ids.extend(
                rules
                    .allows
                    .iter()
                    .filter_map(|a| ir.policies.get(a.policy.index()))
                    .map(|p| p.id.clone()),
            );
        }
    }
    ids
}

/// The `messageKey` of each rule id of the tree, sorted by id.
fn keys(ir: &Ir, tree: &Tree<'_>) -> Vec<(String, String)> {
    rule_ids(tree)
        .into_iter()
        .filter_map(|id| ir.violation_key(&id, None).map(|key| (id, key)))
        .collect()
}

fn write_catalogs(w: &mut CodeWriter, ir: &Ir, package: PackageId) {
    w.line("/** The message catalogs of this member (`spec/platform.md` section 22): language tag, as the file is named, then message key, then text. */");
    w.open_with(
        "export const messages: Readonly<Record<string, Readonly<Record<string, string>>>> =",
        " {",
    );
    for (tag, texts) in &ir.package(package).messages {
        w.open_with(format!("{}:", prop_key(tag)), " {");
        for (key, text) in texts {
            w.line(format!("{}: {},", prop_key(key), ts_str(text)));
        }
        w.close_with("},");
    }
    w.close_with("};");
}

fn write_keys(w: &mut CodeWriter, keys: &[(String, String)]) {
    w.line("/** The `messageKey` of every rule of this member, by rule id. */");
    w.open_with(
        "export const messageKeys: Readonly<Record<string, string>> =",
        " {",
    );
    for (id, key) in keys {
        w.line(format!("{}: {},", prop_key(id), ts_str(key)));
    }
    w.close_with("};");
}

fn write_fixed(w: &mut CodeWriter) {
    w.line("/** The keys of the texts the runtime writes itself. */");
    w.open_with("export const fixedMessageKeys =", " {");
    w.line(format!("transition: {},", ts_str(TRANSITION)));
    w.line(format!("transitionNotDue: {},", ts_str(TRANSITION_NOT_DUE)));
    w.line(format!("version: {},", ts_str(VERSION)));
    w.line(format!("notPermitted: {},", ts_str(NOT_PERMITTED)));
    w.close_with("} as const;");
}

/// Writes the catalog module of each package of the tree that has catalogs. Nothing for a tree
/// whose packages have none: such a target keeps its files.
///
/// # Errors
/// A generated file cannot be added.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    if !tree.uses_messages() {
        return Ok(());
    }
    let all = keys(tree.ir, tree);
    for package in packages(tree) {
        let mut w = CodeWriter::new();
        w.line(HEADER);
        w.blank();
        write_catalogs(&mut w, tree.ir, package);
        w.blank();
        write_keys(&mut w, &all);
        if tree.uses_messages_server() {
            w.blank();
            write_fixed(&mut w);
        }
        add_file(files, &format!("{}.ts", module(tree, package)), w.finish())?;
    }
    Ok(())
}

/// The lines of the barrel that export the catalogs: `messages` and `messageKeys` of the first
/// package, and the others under the name of their package (`<package>Messages`).
#[must_use]
pub fn exports(tree: &Tree<'_>, barrel: &str) -> Vec<String> {
    let from = format!("{barrel}index.ts");
    let mut out = Vec::new();
    for (n, package) in packages(tree).into_iter().enumerate() {
        let path = crate::util::rel_import(&from, &module(tree, package));
        if n == 0 {
            // The fixed keys are the server's (`spec/platform.md` section 22).
            let fixed = if tree.uses_messages_server() {
                "fixedMessageKeys, "
            } else {
                ""
            };
            out.push(format!(
                "export {{ {fixed}messageKeys, messages }} from '{path}';"
            ));
        } else {
            let base = naming::camel(
                &tree
                    .ir
                    .package(package)
                    .name
                    .trim_start_matches('@')
                    .replace('/', "-"),
            );
            out.push(format!(
                "export {{ messageKeys as {base}MessageKeys, messages as {base}Messages }} from '{path}';"
            ));
        }
    }
    out
}

/// The catalog module the `ClientCore` of a client of `package` reads its keys from, with the
/// name it is imported by: `None` for a package without catalogs.
#[must_use]
pub fn client_keys(tree: &Tree<'_>, package: PackageId) -> Option<String> {
    (tree.uses_messages() && !tree.ir.package(package).messages.is_empty())
        .then(|| module(tree, package))
}

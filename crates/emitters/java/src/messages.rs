//! Message catalogs (`spec/platform.md` section 22, `spec/targets/java.md` section 18.3).
//!
//! A member with catalogs gets a class `Messages` in the Java package of its contract package
//! (`<base>.<package>.Messages`): the catalogs as data, sorted, and the `messageKey` of every rule id
//! of the tree, from `Ir::violation_key`. A server also gets the four fixed keys. The keys are never
//! built from an id in generated code.

use std::collections::BTreeSet;

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::message_keys::{NOT_PERMITTED, TRANSITION, TRANSITION_NOT_DUE, VERSION};
use veld_ir::{DeclId, Ir, PackageId, Service};

use crate::tree::{FileCtx, Tree, java_path};
use crate::util::{add_file, doc_comment, java_str};

/// The name of the class.
const CLASS: &str = "Messages";

/// How many entries one method of the class holds: a method of a class has at most 64 KiB of code.
const CHUNK: usize = 250;

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

/// A private static method that builds a map of strings from `entries`, in chunks.
fn write_map(w: &mut CodeWriter, name: &str, entries: &[(String, String)]) {
    w.open(format!(
        "private static java.util.Map<String, String> {name}()"
    ));
    w.line("java.util.Map<String, String> out = new java.util.LinkedHashMap<>();");
    for n in 0..entries.chunks(CHUNK).len() {
        w.line(format!("{name}{n}(out);"));
    }
    w.line("return java.util.Collections.unmodifiableMap(out);");
    w.close();
    for (n, chunk) in entries.chunks(CHUNK).enumerate() {
        w.blank();
        w.open(format!(
            "private static void {name}{n}(java.util.Map<String, String> out)"
        ));
        for (k, v) in chunk {
            w.line(format!("out.put({}, {});", java_str(k), java_str(v)));
        }
        w.close();
    }
}

fn write_class(
    w: &mut CodeWriter,
    ir: &Ir,
    package: PackageId,
    all: &[(String, String)],
    server: bool,
) {
    doc_comment(
        w,
        Some(
            "The message catalogs of this member (`spec/platform.md` section 22) and the `messageKey` of every rule of it.",
        ),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {CLASS}"));
    w.line(format!("private {CLASS}() {{}}"));
    if server {
        w.blank();
        for (name, value) in [
            ("TRANSITION", TRANSITION),
            ("TRANSITION_NOT_DUE", TRANSITION_NOT_DUE),
            ("VERSION", VERSION),
            ("NOT_PERMITTED", NOT_PERMITTED),
        ] {
            w.line(format!(
                "/** The key of the texts the runtime writes itself: `{value}`. */"
            ));
            w.line(format!(
                "public static final String {name} = {};",
                java_str(value)
            ));
        }
    }
    let languages: Vec<(&String, &std::collections::BTreeMap<String, String>)> =
        ir.package(package).messages.iter().collect();
    w.blank();
    w.line("private static final java.util.Map<String, java.util.Map<String, String>> CATALOGS = catalogs0();");
    w.line("private static final java.util.Map<String, String> KEYS = ruleKeys();");
    w.blank();
    w.line("/** The catalogs: language tag as the file is named (lower case), then message key, then text. */");
    w.open("public static java.util.Map<String, java.util.Map<String, String>> catalogs()");
    w.line("return CATALOGS;");
    w.close();
    w.blank();
    w.line("/** The `messageKey` of every rule of this member, by rule id. */");
    w.open("public static java.util.Map<String, String> keys()");
    w.line("return KEYS;");
    w.close();
    w.blank();
    w.open("private static java.util.Map<String, java.util.Map<String, String>> catalogs0()");
    w.line("java.util.Map<String, java.util.Map<String, String>> out = new java.util.LinkedHashMap<>();");
    for (n, (tag, _)) in languages.iter().enumerate() {
        w.line(format!("out.put({}, catalog{n}());", java_str(tag)));
    }
    w.line("return java.util.Collections.unmodifiableMap(out);");
    w.close();
    for (n, (_, texts)) in languages.iter().enumerate() {
        w.blank();
        let entries: Vec<(String, String)> =
            texts.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        write_map(w, &format!("catalog{n}"), &entries);
    }
    w.blank();
    write_map(w, "ruleKeys", all);
    w.close();
}

/// Writes the `Messages` class of each package of the tree that has catalogs. Nothing for a tree
/// whose packages have none: such a target keeps its files.
///
/// # Errors
/// A generated file cannot be added.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    let all = keys(tree.ir, tree);
    for package in packages(tree) {
        let java_package = tree.pkg_package(package);
        let fc = FileCtx::new(tree, &java_package, CLASS);
        let mut w = CodeWriter::new();
        write_class(&mut w, tree.ir, package, &all, tree.messages_server);
        add_file(
            files,
            &java_path(&java_package, CLASS),
            fc.finish(&w.finish()),
        )?;
    }
    Ok(())
}

/// `builder.messages(Catalogs.support(Messages.catalogs(), Messages.keys()));` for a service of a
/// server whose package has catalogs: what `<Service>Hooks` registers. Nothing otherwise.
#[must_use]
pub fn registrations(fc: &mut FileCtx<'_, '_>, svc: &Service) -> Vec<String> {
    if !fc.tree.messages_server || fc.tree.ir.package(svc.package).messages.is_empty() {
        return Vec::new();
    }
    let messages = fc.name(&format!("{}.{CLASS}", fc.tree.pkg_package(svc.package)));
    let catalogs = fc.rt("Catalogs");
    vec![format!(
        "builder.messages({catalogs}.support({messages}.catalogs(), {messages}.keys()));"
    )]
}

/// `this.core.messageKeys(Messages.keys());` for a client of a package that has catalogs: the
/// violations of its local check have their `messageKey`. Nothing otherwise.
#[must_use]
pub fn client_registration(fc: &mut FileCtx<'_, '_>, svc: &Service) -> Option<String> {
    if fc.tree.ir.package(svc.package).messages.is_empty() || !fc.tree.rules {
        return None;
    }
    let messages = fc.name(&format!("{}.{CLASS}", fc.tree.pkg_package(svc.package)));
    Some(format!("this.core.messageKeys({messages}.keys());"))
}

/// The key of the `n`th `forbid` (from 1) of the policy with id `policy_id`, for the generated
/// policy method: the key of a `forbid` with a message in a catalog.
#[must_use]
pub fn forbid_key(ir: &Ir, policy_id: &str, n: usize) -> Option<String> {
    ir.violation_key(policy_id, Some(n))
}

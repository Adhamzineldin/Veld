//! Who may `use` which registry package.
//!
//! A project may only `use` registry packages it lists in `[dependencies]`; a registry package
//! may only `use` the packages it declares as its own dependencies. A package that is merely
//! installed because something else needs it (a transitive dependency) is not importable from
//! the project, so `use @org/name:...` always names a package the project controls the version of.

use std::collections::{BTreeMap, BTreeSet};

use veld_ir::PackageKind;

use crate::diagnostics::Diagnostic;
use crate::semantic::ParsedPackage;

/// For each package name, the registry packages it may `use`. A package that is not listed is
/// unrestricted (the check only guards registry packages).
#[derive(Debug, Clone, Default)]
pub struct AccessRules {
    pub allowed: BTreeMap<String, BTreeSet<String>>,
}

/// Reports `E0206` for every `use @org/name:...` of a registry package that the importing
/// package does not depend on directly.
pub(crate) fn check(rules: &AccessRules, packages: &[ParsedPackage]) -> Vec<Diagnostic> {
    let kinds: BTreeMap<&str, PackageKind> =
        packages.iter().map(|p| (p.name.as_str(), p.kind)).collect();
    let mut out = Vec::new();
    for pkg in packages {
        let Some(allowed) = rules.allowed.get(&pkg.name) else {
            continue;
        };
        for file in &pkg.files {
            for u in &file.ast.uses {
                let Some(target) = &u.package else { continue };
                let is_registry = kinds.get(target.name.as_str()) == Some(&PackageKind::Registry);
                if is_registry && target.name != pkg.name && !allowed.contains(&target.name) {
                    out.push(violation(&pkg.name, &target.name, target.span));
                }
            }
        }
    }
    out
}

fn violation(importer: &str, target: &str, span: crate::diagnostics::Span) -> Diagnostic {
    let (owner, help) = if importer.starts_with('@') {
        (
            format!("package `{importer}`"),
            format!(
                "`{importer}` must declare `{target}` in its `[dependencies]` and be republished"
            ),
        )
    } else {
        (
            "the project".to_string(),
            format!("add it to `[dependencies]` in veld.toml with `veld add {target}`"),
        )
    };
    Diagnostic::error(
        "E0206",
        format!("`{target}` is not a direct dependency of {owner}"),
        span,
    )
    .with_note(
        "a package that is only installed because another package depends on it cannot be imported",
    )
    .with_help(help)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompileInput, PackageInput, SourceFile, compile_with};

    fn pkg(name: &str, kind: PackageKind, file: &str, text: &str) -> PackageInput {
        PackageInput {
            name: name.into(),
            kind,
            files: vec![SourceFile {
                path: file.into(),
                text: text.into(),
            }],
            base_url: None,
        }
    }

    fn input(app: &str) -> CompileInput {
        CompileInput {
            packages: vec![
                pkg("app", PackageKind::Service, "app.veld", app),
                pkg(
                    "@acme/money",
                    PackageKind::Registry,
                    "money.veld",
                    "model Money { cents: int }\n",
                ),
                pkg(
                    "@acme/billing",
                    PackageKind::Registry,
                    "invoice.veld",
                    "use @acme/money:money.{Money}\nmodel Invoice { total: Money }\n",
                ),
            ],
            consumes: BTreeMap::new(),
        }
    }

    fn rules(app: &[&str]) -> AccessRules {
        let set = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<BTreeSet<_>>();
        AccessRules {
            allowed: BTreeMap::from([
                ("app".to_string(), set(app)),
                ("@acme/billing".to_string(), set(&["@acme/money"])),
                ("@acme/money".to_string(), set(&[])),
            ]),
        }
    }

    fn codes(src: &str, rules: &AccessRules) -> Vec<&'static str> {
        compile_with(&input(src), Some(rules))
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect()
    }

    const USES_BILLING: &str =
        "use @acme/billing:invoice.{Invoice}\nmodel Order { invoice: Invoice }\n";
    const USES_MONEY: &str = "use @acme/money:money.{Money}\nmodel Order { total: Money }\n";

    #[test]
    fn a_direct_dependency_may_be_used_and_it_may_use_its_own() {
        assert!(codes(USES_BILLING, &rules(&["@acme/billing"])).is_empty());
    }

    #[test]
    fn a_transitive_dependency_may_not_be_used_directly() {
        let out = compile_with(&input(USES_MONEY), Some(&rules(&["@acme/billing"])));
        let d: Vec<_> = out.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(d, ["E0206"]);
        let text = out.render();
        assert!(
            text.contains("`@acme/money` is not a direct dependency of the project"),
            "{text}"
        );
        assert!(text.contains("veld add @acme/money"), "{text}");
        assert!(out.ir.is_none());
    }

    #[test]
    fn a_package_may_not_use_what_it_does_not_declare() {
        let mut r = rules(&["@acme/billing"]);
        r.allowed.insert("@acme/billing".into(), BTreeSet::new());
        let out = compile_with(&input(USES_BILLING), Some(&r));
        assert_eq!(out.diagnostics.len(), 1);
        assert!(
            out.render().contains("package `@acme/billing`"),
            "{}",
            out.render()
        );
    }

    #[test]
    fn without_rules_nothing_is_checked() {
        assert!(crate::compile(&input(USES_MONEY)).diagnostics.is_empty());
    }
}

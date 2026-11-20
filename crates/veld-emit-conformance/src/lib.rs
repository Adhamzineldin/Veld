//! Shared conformance checks for Veld generators (test support).
//!
//! A generator crate's `tests/conformance.rs` calls [`check_generator`]. The checks:
//!
//! - **Capability coverage.** For every fixture contract and every role: a feature the
//!   generator does not declare is reported as `E0801` by the capability gate, and a
//!   contract whose features are all declared must generate.
//! - **Signatures.** Every declaration and operation `sig` of the IR appears verbatim in the
//!   generated files. Signatures are computed only by the compiler; generators embed them.
//! - **Determinism.** Two runs on the same IR produce identical files.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
// This crate is test support: it panics on purpose, with a report, when a check fails.
#![allow(clippy::panic)]

mod fixtures;

use std::collections::BTreeMap;

use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

pub use fixtures::{FIXTURES, Fixture};

/// Compiles one contract as the single service package `app`.
///
/// # Panics
/// When the contract does not compile; fixtures are fixed and must always compile.
#[must_use]
pub fn compile(name: &str, source: &str) -> Ir {
    use veld_compiler::{CompileInput, PackageInput, SourceFile};
    let out = veld_compiler::compile(&CompileInput {
        packages: vec![PackageInput {
            name: "app".into(),
            kind: veld_ir::PackageKind::Service,
            files: vec![SourceFile {
                path: format!("{name}.veld"),
                text: source.into(),
            }],
            base_url: Some("http://localhost:3000".into()),
        }],
        consumes: BTreeMap::new(),
    });
    let rendered = out.render();
    match out.ir {
        Some(ir) => ir,
        None => panic!("fixture `{name}` does not compile:\n{rendered}"),
    }
}

/// Every declaration signature and operation signature of `services` and what they reach.
#[must_use]
pub fn expected_signatures(ir: &Ir, services: &[ServiceId]) -> Vec<String> {
    let mut sigs: Vec<String> = ir
        .reachable_decls(services)
        .into_iter()
        .map(|d| ir.decl(d).sig.clone())
        .collect();
    for sid in services {
        sigs.extend(ir.service(*sid).endpoints.iter().map(|e| e.sig.clone()));
    }
    sigs
}

/// Checks that every expected signature occurs verbatim in some generated file.
pub fn check_signatures(ir: &Ir, services: &[ServiceId], files: &FileSet) -> Result<(), String> {
    let contents: Vec<String> = files
        .iter()
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect();
    let missing: Vec<String> = expected_signatures(ir, services)
        .into_iter()
        .filter(|s| !contents.iter().any(|c| c.contains(s.as_str())))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} signature(s) are not embedded verbatim, for example `{}`",
            missing.len(),
            missing[0]
        ))
    }
}

/// Runs every check; returns the failures (empty when the generator conforms).
#[must_use]
pub fn run_checks(generator: &dyn DynGenerator) -> Vec<String> {
    let mut failures = Vec::new();
    if generator.id().is_empty() || generator.roles().is_empty() {
        failures.push("a generator needs a non-empty id and at least one role".to_string());
    }
    for role in generator.roles() {
        for fixture in FIXTURES {
            if let Err(e) = check_fixture(generator, *role, fixture) {
                failures.push(format!(
                    "[{} / {} / {}] {e}",
                    generator.id(),
                    role.name(),
                    fixture.name
                ));
            }
        }
    }
    failures
}

/// Panics with every failure when the generator does not conform.
///
/// # Panics
/// When any check fails.
pub fn check_generator(generator: &dyn DynGenerator) {
    let failures = run_checks(generator);
    assert!(
        failures.is_empty(),
        "generator conformance failed:\n  {}",
        failures.join("\n  ")
    );
}

fn check_fixture(
    generator: &dyn DynGenerator,
    role: Role,
    fixture: &Fixture,
) -> Result<(), String> {
    let ir = compile(fixture.name, fixture.source);
    if let Some(feature) = fixture.feature {
        let used = ir.features_used(&all_services(&ir));
        if !used.contains(&feature) {
            return Err(format!(
                "the fixture does not use `{}` (a bug in veld-emit-conformance)",
                feature.name()
            ));
        }
    }
    let services = all_services(&ir);
    let req = GenRequest {
        ir: &ir,
        role,
        services: services.clone(),
        consumes: vec![],
        framework: None,
    };
    let caps = generator.capabilities(role);
    let gate = check_capabilities(&ir, &req, &caps);
    let unsupported: Vec<Feature> = ir
        .features_used(&services)
        .into_iter()
        .filter(|f| !caps.supports(*f))
        .collect();
    if gate.len() != unsupported.len() || gate.iter().any(|d| d.code != "E0801") {
        return Err(format!(
            "the capability gate reported {gate:?} for the unsupported features {unsupported:?}"
        ));
    }
    if !unsupported.is_empty() {
        return Ok(());
    }
    let first = generate(generator, &req)?;
    check_signatures(&ir, &services, &first)?;
    let second = generate(generator, &req)?;
    if first != second {
        return Err("two runs on the same IR produced different files".to_string());
    }
    Ok(())
}

fn all_services(ir: &Ir) -> Vec<ServiceId> {
    ir.services.iter().map(|s| s.id).collect()
}

fn generate(generator: &dyn DynGenerator, req: &GenRequest<'_>) -> Result<FileSet, String> {
    generator
        .generate_dyn(req, &serde_json::Value::Null)
        .map_err(|d| {
            let lines: Vec<String> = d
                .iter()
                .map(|d| format!("{}: {}", d.code, d.message))
                .collect();
            format!(
                "generation failed although every used feature is declared:\n    {}",
                lines.join("\n    ")
            )
        })
}

#[cfg(test)]
mod tests {
    use veld_codegen::{Capabilities, GenDiagnostic};

    use super::*;

    /// A deliberately minimal generator: declares nothing, embeds every signature.
    struct Minimal {
        embed: bool,
        caps: Capabilities,
    }

    impl DynGenerator for Minimal {
        fn id(&self) -> &'static str {
            "minimal"
        }
        fn roles(&self) -> &'static [Role] {
            &[Role::Client]
        }
        fn frameworks(&self, _: Role) -> Vec<veld_codegen::FrameworkInfo> {
            Vec::new()
        }
        fn capabilities(&self, _: Role) -> Capabilities {
            self.caps.clone()
        }
        fn generate_dyn(
            &self,
            req: &GenRequest<'_>,
            _: &serde_json::Value,
        ) -> Result<FileSet, Vec<GenDiagnostic>> {
            let mut files = FileSet::new();
            let sigs = if self.embed {
                expected_signatures(req.ir, &req.services)
            } else {
                vec![]
            };
            files
                .add("all.txt", sigs.join("\n"))
                .map_err(|e| vec![GenDiagnostic::new("E0899", e.to_string())])?;
            Ok(files)
        }
    }

    #[test]
    fn every_fixture_compiles_and_uses_its_feature() {
        for f in FIXTURES {
            let ir = compile(f.name, f.source);
            let used = ir.features_used(&all_services(&ir));
            match f.feature {
                Some(feature) => assert!(used.contains(&feature), "{}: {used:?}", f.name),
                None => assert!(used.is_empty(), "{}: {used:?}", f.name),
            }
        }
        let covered: Vec<Feature> = FIXTURES.iter().filter_map(|f| f.feature).collect();
        // `consts` is never reported by `features_used` (see fixtures.rs), so it has no fixture.
        for feature in Feature::ALL.into_iter().filter(|f| *f != Feature::Consts) {
            assert!(
                covered.contains(&feature),
                "no fixture for `{}`",
                feature.name()
            );
        }
    }

    #[test]
    fn a_minimal_generator_with_no_capabilities_conforms() {
        assert_eq!(
            run_checks(&Minimal {
                embed: true,
                caps: Capabilities::default()
            }),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_generator_that_drops_signatures_fails() {
        let failures = run_checks(&Minimal {
            embed: false,
            caps: Capabilities::default(),
        });
        assert!(
            failures.iter().any(|f| f.contains("not embedded verbatim")),
            "{failures:?}"
        );
    }

    #[test]
    fn claiming_a_feature_it_cannot_generate_is_caught_by_the_signature_check() {
        let failures = run_checks(&Minimal {
            embed: false,
            caps: Capabilities::all_except(&[]),
        });
        assert!(failures.len() >= FIXTURES.len(), "{failures:?}");
    }
}

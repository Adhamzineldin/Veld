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

/// Where the invariant of [`ir_with_rule`] is declared.
pub const RULE_ORIGIN_LINE: u32 = 4;

/// The plain fixture with one invariant attached to its first model, built by hand (the language
/// has no rule syntax in this crate's dependencies yet). `rules_invariants` is the feature it uses.
///
/// # Panics
/// When the plain fixture no longer has a model.
#[must_use]
pub fn ir_with_rule() -> Ir {
    use veld_ir::{DeclKind, Expr, ExprKind, ExprLit, ExprType, Origin, Rule, Scalar, Type};
    let plain = FIXTURES
        .iter()
        .find(|f| f.feature.is_none())
        .map(|f| compile(f.name, f.source));
    let Some(mut ir) = plain else {
        panic!("veld-emit-conformance has no plain fixture");
    };
    let rule = Rule {
        name: "alwaysTrue".into(),
        id: "veld:app/Greeting.alwaysTrue@0".into(),
        expr: Expr {
            t: ExprType::Type(Type::Scalar(Scalar::Bool)),
            e: ExprKind::Lit(ExprLit::Bool(true)),
        },
        message: None,
        reads: vec![],
        reads_readonly: false,
        inherited_from: None,
        docs: None,
        deprecated: None,
        origin: Origin {
            file: "rules.veld".into(),
            line: RULE_ORIGIN_LINE,
            col: 3,
        },
    };
    let model = ir.decls.iter_mut().find_map(|d| match &mut d.kind {
        DeclKind::Model(m) if d.name == "Greeting" => Some(m),
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Enum(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_) => None,
    });
    match model {
        Some(m) => m.rules.push(rule),
        None => panic!("the plain fixture has no `Greeting` model"),
    }
    ir
}

/// The generator must neither declare a rules feature nor generate a contract with a rule: the
/// capability gate reports `E0801` at the rule's origin, and `generate` refuses with `E0801`.
///
/// # Errors
/// A description of the first role for which the generator does not refuse.
pub fn check_rules_are_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    let ir = ir_with_rule();
    let services = all_services(&ir);
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        if let Some(f) = Feature::RULES.into_iter().find(|f| caps.supports(*f)) {
            return Err(format!("{}: declares `{}`", role.name(), f.name()));
        }
        let req = GenRequest {
            ir: &ir,
            role: *role,
            services: services.clone(),
            consumes: vec![],
            framework: None,
        };
        let gate = check_capabilities(&ir, &req, &caps);
        let at_rule = |d: &veld_codegen::GenDiagnostic| {
            d.code == "E0801"
                && d.message.contains("rules_invariants")
                && d.origin.as_ref().map(|o| o.line) == Some(RULE_ORIGIN_LINE)
        };
        if gate.len() != 1 || !gate.iter().all(at_rule) {
            return Err(format!("{}: the gate reported {gate:?}", role.name()));
        }
        match generator.generate_dyn(&req, &serde_json::Value::Null) {
            Err(d) if !d.is_empty() && d.iter().all(|d| d.code == "E0801") => {}
            other => {
                return Err(format!(
                    "{}: generate did not answer E0801: {other:?}",
                    role.name()
                ));
            }
        }
    }
    Ok(())
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

/// How to run the checks for a generator whose support depends on the framework.
///
/// The `framework` applies to the `server` role only (a client has no framework). Every feature in
/// `rejected` must be refused with `E0801` for that framework, by a generator that declares it in
/// its capabilities; every other feature must generate.
#[derive(Debug, Clone, Copy, Default)]
pub struct Setup<'a> {
    /// The framework of the server role, or `None` for the generator's default.
    pub framework: Option<&'a str>,
    /// Features that this framework cannot serve.
    pub rejected: &'a [Feature],
}

/// Runs every check; returns the failures (empty when the generator conforms).
#[must_use]
pub fn run_checks(generator: &dyn DynGenerator) -> Vec<String> {
    run_checks_with(generator, Setup::default())
}

/// [`run_checks`] for a given framework and the features it must reject with `E0801`.
#[must_use]
pub fn run_checks_with(generator: &dyn DynGenerator, setup: Setup<'_>) -> Vec<String> {
    let mut failures = Vec::new();
    if generator.id().is_empty() || generator.roles().is_empty() {
        failures.push("a generator needs a non-empty id and at least one role".to_string());
    }
    for role in generator.roles() {
        for fixture in FIXTURES {
            let setup = if *role == Role::Server {
                setup
            } else {
                Setup::default()
            };
            if let Err(e) = check_fixture(generator, *role, fixture, setup) {
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
    check_generator_with(generator, Setup::default());
}

/// [`check_generator`] for a given framework and the features it must reject with `E0801`.
///
/// # Panics
/// When any check fails.
pub fn check_generator_with(generator: &dyn DynGenerator, setup: Setup<'_>) {
    let failures = run_checks_with(generator, setup);
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
    setup: Setup<'_>,
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
        framework: setup.framework.map(str::to_string),
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
    if services_use_rejected(&ir, &services, setup) {
        return check_rejected(generator, &req);
    }
    let first = generate(generator, &req)?;
    check_signatures(&ir, &services, &first)?;
    let second = generate(generator, &req)?;
    if first != second {
        return Err("two runs on the same IR produced different files".to_string());
    }
    Ok(())
}

/// Whether the contract uses a feature that the framework of `setup` must reject.
fn services_use_rejected(ir: &Ir, services: &[ServiceId], setup: Setup<'_>) -> bool {
    ir.features_used(services)
        .iter()
        .any(|f| setup.rejected.contains(f))
}

/// The generator must refuse the request, with `E0801` only, and write nothing.
fn check_rejected(generator: &dyn DynGenerator, req: &GenRequest<'_>) -> Result<(), String> {
    match generator.generate_dyn(req, &serde_json::Value::Null) {
        Err(d) if !d.is_empty() && d.iter().all(|d| d.code == "E0801") => Ok(()),
        Err(d) => Err(format!(
            "expected only E0801 for a rejected feature, got {d:?}"
        )),
        Ok(_) => Err("generated a feature that the framework must reject with E0801".to_string()),
    }
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
        // The rules features have no fixture until the language has rule syntax; `ir_with_rule`
        // covers `rules_invariants` by hand.
        for feature in Feature::ALL
            .into_iter()
            .filter(|f| *f != Feature::Consts && !Feature::RULES.contains(f))
        {
            assert!(
                covered.contains(&feature),
                "no fixture for `{}`",
                feature.name()
            );
        }
    }

    #[test]
    fn the_hand_built_rule_uses_the_invariants_feature() {
        let ir = ir_with_rule();
        let used = ir.features_used(&all_services(&ir));
        assert!(used.contains(&Feature::RulesInvariants), "{used:?}");
    }

    #[test]
    fn a_generator_that_claims_rules_is_caught() {
        let claims = Minimal {
            embed: true,
            caps: Capabilities::all_except(&[]),
        };
        assert!(check_rules_are_refused(&claims).is_err());
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
    /// A server that declares everything and, for one framework, refuses `streaming_ws`.
    struct Framed {
        refuse_ws: bool,
    }

    impl DynGenerator for Framed {
        fn id(&self) -> &'static str {
            "framed"
        }
        fn roles(&self) -> &'static [Role] {
            &[Role::Server]
        }
        fn frameworks(&self, _: Role) -> Vec<veld_codegen::FrameworkInfo> {
            Vec::new()
        }
        fn capabilities(&self, _: Role) -> Capabilities {
            Capabilities::all_except(&[])
        }
        fn generate_dyn(
            &self,
            req: &GenRequest<'_>,
            _: &serde_json::Value,
        ) -> Result<FileSet, Vec<GenDiagnostic>> {
            if self.refuse_ws
                && req
                    .ir
                    .features_used(&req.services)
                    .contains(&Feature::StreamingWs)
            {
                return Err(vec![GenDiagnostic::new("E0801", "no WebSocket here")]);
            }
            let mut files = FileSet::new();
            files
                .add(
                    "all.txt",
                    expected_signatures(req.ir, &req.services).join("\n"),
                )
                .map_err(|e| vec![GenDiagnostic::new("E0899", e.to_string())])?;
            Ok(files)
        }
    }

    #[test]
    fn a_framework_that_rejects_a_feature_with_e0801_conforms() {
        let setup = Setup {
            framework: None,
            rejected: &[Feature::StreamingWs],
        };
        assert_eq!(
            run_checks_with(&Framed { refuse_ws: true }, setup),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_framework_that_generates_a_feature_it_must_reject_fails() {
        let setup = Setup {
            framework: None,
            rejected: &[Feature::StreamingWs],
        };
        let failures = run_checks_with(&Framed { refuse_ws: false }, setup);
        assert!(
            failures.iter().any(|f| f.contains("must reject")),
            "{failures:?}"
        );
        // Without the rejection list the same generator generates everything and conforms.
        assert_eq!(
            run_checks(&Framed { refuse_ws: false }),
            Vec::<String>::new()
        );
        // A generator that refuses something it was not told to refuse fails too.
        assert!(!run_checks(&Framed { refuse_ws: true }).is_empty());
    }
}

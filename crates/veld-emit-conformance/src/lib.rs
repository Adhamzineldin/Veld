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

mod event_rollout;
mod events;
mod fixtures;
mod rollout;

use std::collections::BTreeMap;

use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities, checked_features};
use veld_ir::{Feature, Ir, ServiceId};

pub use event_rollout::{
    EventRolloutContract, compiled_event_rollout_contracts, without_the_annotation,
};
pub use events::{
    check_events_are_generated, check_events_are_ignored_by_clients, check_events_are_refused,
    compiled_events_contract,
};
pub use fixtures::{FIXTURES, Fixture};
pub use rollout::{
    check_rollout_is_generated, check_rollout_is_refused, compiled_rollout_contracts,
};

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
        rollout: None,
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
        | DeclKind::Const(_)
        | DeclKind::Event(_) => None,
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

/// Where the first statement of [`ir_with_endpoint_rules`] is declared.
pub const ENDPOINT_RULE_ORIGIN_LINE: u32 = 7;

/// The plain fixture with a block on its first endpoint (one `ensure`), built by hand. The
/// feature it uses is `rules_endpoint`.
///
/// # Panics
/// When the plain fixture no longer has an endpoint.
#[must_use]
pub fn ir_with_endpoint_rules() -> Ir {
    use veld_ir::{EndpointEnsure, Expr, ExprKind, ExprLit, ExprType, Origin, Scalar, Type};
    let plain = FIXTURES
        .iter()
        .find(|f| f.feature.is_none())
        .map(|f| compile(f.name, f.source));
    let Some(mut ir) = plain else {
        panic!("veld-emit-conformance has no plain fixture");
    };
    let ensure = EndpointEnsure {
        name: None,
        id: "veld:app/Greeter.hello.ensure@0".into(),
        expr: Expr {
            t: ExprType::Type(Type::Scalar(Scalar::Bool)),
            e: ExprKind::Lit(ExprLit::Bool(true)),
        },
        message: None,
        reads: vec![],
        docs: None,
        rollout: None,
        origin: Origin {
            file: "rules.veld".into(),
            line: ENDPOINT_RULE_ORIGIN_LINE,
            col: 5,
        },
    };
    let endpoint = ir.services.iter_mut().find_map(|s| s.endpoints.first_mut());
    match endpoint {
        Some(ep) => ep.rules.ensures.push(ensure),
        None => panic!("the plain fixture has no endpoint"),
    }
    ir
}

/// The generator must not declare `rules_endpoint` and must refuse a contract with endpoint rules:
/// the capability gate reports exactly one `E0801` at the first statement, and `generate` refuses
/// with `E0801`. Other rules features (invariants) may be declared.
///
/// # Errors
/// A description of the first role for which the generator does not refuse.
pub fn check_endpoint_rules_are_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    refuses_endpoint_rules(
        generator,
        &ir_with_endpoint_rules(),
        Some(ENDPOINT_RULE_ORIGIN_LINE),
    )
}

/// The contract `veld_compiler` makes of a block with a fact, a `require` and an `ensure`: what a
/// real contract with endpoint rules looks like to a generator.
#[must_use]
pub fn compiled_endpoint_rules_contract() -> Ir {
    compile(
        "compiled_endpoint_rules",
        "model Booking { id: uuid, guests: int }

service Bookings at \"/bookings\" {
  post \"/\" create(body: Booking) -> 201 Booking {
    fact room: Booking = body.id
    require body.guests > 0
    ensure result.guests > 0
  }
}
",
    )
}

/// Like [`check_endpoint_rules_are_refused`], for a contract that went through the compiler: the
/// gate and `generate` answer `E0801` for it as well.
///
/// # Errors
/// A description of the first role for which the generator does not refuse.
pub fn check_compiled_endpoint_rules_are_refused(
    generator: &dyn DynGenerator,
) -> Result<(), String> {
    refuses_endpoint_rules(generator, &compiled_endpoint_rules_contract(), None)
}

fn refuses_endpoint_rules(
    generator: &dyn DynGenerator,
    ir: &Ir,
    origin_line: Option<u32>,
) -> Result<(), String> {
    let services = all_services(ir);
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        if caps.supports(Feature::RulesEndpoint) {
            return Err(format!("{}: declares `rules_endpoint`", role.name()));
        }
        let req = GenRequest {
            ir,
            role: *role,
            services: services.clone(),
            consumes: vec![],
            framework: None,
        };
        let gate = check_capabilities(ir, &req, &caps);
        let at_statement = |d: &veld_codegen::GenDiagnostic| {
            d.code == "E0801"
                && d.message.contains("rules_endpoint")
                && origin_line.is_none_or(|l| d.origin.as_ref().map(|o| o.line) == Some(l))
        };
        if gate.len() != 1 || !gate.iter().all(at_statement) {
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

const STATE_MACHINE_SOURCE: &str = "enum Status {
  pending, paid, cancelled
  flow pending -> paid
  flow pending -> cancelled
  final paid, cancelled
}

model Booking { id: uuid, status: Status }

service Bookings at \"/bookings\" {
  get \"/{id}\" get(id: uuid) -> Booking
}
";

const TRANSITION_SOURCE: &str = "enum Status {
  pending, paid, cancelled
  flow pending -> paid
  flow pending -> cancelled
  final paid, cancelled
}

model Booking { id: uuid, status: Status }

service Bookings at \"/bookings\" {
  post \"/{id}/pay\" pay(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending -> paid
  }
}
";

/// The contract of a state machine (an enum with flows) without any endpoint rule, and the line
/// where the machine is declared.
#[must_use]
pub fn compiled_state_machine_contract() -> (Ir, u32) {
    let ir = compile("compiled_state_machine", STATE_MACHINE_SOURCE);
    (ir, line_of(STATE_MACHINE_SOURCE, "enum Status"))
}

/// The contract of an endpoint with a `transition` (and a machine), and the line of the statement.
#[must_use]
pub fn compiled_transition_contract() -> (Ir, u32) {
    let ir = compile("compiled_transition", TRANSITION_SOURCE);
    (ir, line_of(TRANSITION_SOURCE, "transition booking"))
}

fn line_of(source: &str, needle: &str) -> u32 {
    let at = source.lines().position(|l| l.contains(needle));
    match at.and_then(|i| u32::try_from(i + 1).ok()) {
        Some(line) => line,
        None => panic!("`{needle}` is not in the fixture"),
    }
}

/// The generator must not declare `rules_transitions` and must refuse a compiled contract with a
/// state machine, and one with a `transition`: the gate reports `E0801` for `rules_transitions`
/// at the first flow or transition, and `generate` answers `E0801` only. (A generator that does
/// not support endpoint rules reports those too: the contract with a transition has a fact.)
///
/// # Errors
/// A description of the first role and contract for which the generator does not refuse.
pub fn check_state_machines_are_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    for (ir, line) in [
        compiled_state_machine_contract(),
        compiled_transition_contract(),
    ] {
        refuses_state_machines(generator, &ir, line)?;
    }
    Ok(())
}

fn refuses_state_machines(
    generator: &dyn DynGenerator,
    ir: &Ir,
    origin_line: u32,
) -> Result<(), String> {
    let services = all_services(ir);
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        if caps.supports(Feature::RulesTransitions) {
            return Err(format!("{}: declares `rules_transitions`", role.name()));
        }
        let req = GenRequest {
            ir,
            role: *role,
            services: services.clone(),
            consumes: vec![],
            framework: None,
        };
        let gate = check_capabilities(ir, &req, &caps);
        let mine: Vec<_> = gate
            .iter()
            .filter(|d| d.message.contains("rules_transitions"))
            .collect();
        let at_the_machine = mine.len() == 1
            && mine.iter().all(|d| {
                d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(origin_line)
            });
        if !at_the_machine || gate.iter().any(|d| d.code != "E0801") {
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

const POLICY_SOURCE: &str = "model User { id: uuid }
principal User

model Booking { id: uuid, owner: uuid }

policy Manage(user: User, booking: Booking) {
  permit when booking.owner == user.id
}

@auth(bearer)
service Bookings at \"/bookings\" {
  get \"/{id}\" get(id: uuid) -> Booking {
    fact booking: Booking = id
    allow Manage(user, booking)
  }
}
";

const UNUSED_POLICY_SOURCE: &str = "model User { id: uuid }
principal User

policy Anyone(user: User) {
  permit when true
}

@auth(bearer)
service Users at \"/users\" {
  get \"/\" list() -> User
}
";

/// The contract of an endpoint with an `allow` (a principal and a policy), and the line of the
/// statement.
#[must_use]
pub fn compiled_policy_contract() -> (Ir, u32) {
    let ir = compile("compiled_policy", POLICY_SOURCE);
    (ir, line_of(POLICY_SOURCE, "allow Manage"))
}

const PRINCIPAL_READ_SOURCE: &str = "model User { id: uuid, banned: bool }
principal User

@auth(bearer)
service Users at \"/users\" {
  get \"/\" list() -> User {
    require not user.banned
  }
}
";

/// The contract of an endpoint that reads the principal in a `require` and has no `allow`, and the
/// line of the statement. A generator must refuse it all the same: the principal is a root only
/// the policy feature knows.
#[must_use]
pub fn compiled_principal_read_contract() -> (Ir, u32) {
    let ir = compile("compiled_principal_read", PRINCIPAL_READ_SOURCE);
    (ir, line_of(PRINCIPAL_READ_SOURCE, "require not user"))
}

/// A contract that declares a principal and a policy and uses neither in an endpoint: it needs
/// no feature (`spec/rules.md` section 11.4).
#[must_use]
pub fn compiled_unused_policy_contract() -> Ir {
    compile("compiled_unused_policy", UNUSED_POLICY_SOURCE)
}

/// The generator must not declare `rules_policies` and must refuse a compiled contract with an
/// `allow`: the gate reports `E0801` for `rules_policies` at the statement, and `generate` answers
/// `E0801` only. A contract that declares a policy and a principal but no `allow` needs no
/// feature: the gate lets it through for `rules_policies` and `generate` makes files for it.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_policies_are_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    for (ir, line) in [
        compiled_policy_contract(),
        compiled_principal_read_contract(),
    ] {
        refuses_policies(generator, &ir, line)?;
    }
    accepts_unused_policies(generator)
}

fn refuses_policies(generator: &dyn DynGenerator, ir: &Ir, line: u32) -> Result<(), String> {
    let services = all_services(ir);
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        if caps.supports(Feature::RulesPolicies) {
            return Err(format!("{}: declares `rules_policies`", role.name()));
        }
        let req = GenRequest {
            ir,
            role: *role,
            services: services.clone(),
            consumes: vec![],
            framework: None,
        };
        let gate = check_capabilities(ir, &req, &caps);
        let mine: Vec<_> = gate
            .iter()
            .filter(|d| d.message.contains("rules_policies"))
            .collect();
        let at_the_allow = mine.len() == 1
            && mine
                .iter()
                .all(|d| d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(line));
        if !at_the_allow || gate.iter().any(|d| d.code != "E0801") {
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

fn accepts_unused_policies(generator: &dyn DynGenerator) -> Result<(), String> {
    let ir = compiled_unused_policy_contract();
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        let req = GenRequest {
            ir: &ir,
            role: *role,
            services: all_services(&ir),
            consumes: vec![],
            framework: None,
        };
        let gate = check_capabilities(&ir, &req, &caps);
        if !gate.is_empty() {
            return Err(format!(
                "{}: a policy no endpoint uses was refused: {gate:?}",
                role.name()
            ));
        }
        if let Err(d) = generator.generate_dyn(&req, &serde_json::Value::Null) {
            return Err(format!(
                "{}: generate failed for an unused policy: {d:?}",
                role.name()
            ));
        }
    }
    Ok(())
}

/// A generator that supports invariants and endpoint rules and nothing else of the rules: it
/// declares `rules_invariants` and `rules_endpoint`, the gate lets both a hand-built block and one
/// the compiler made through, and `generate` produces files for them (transitions and policies stay
/// refused).
///
/// # Errors
/// A description of the first role that does not match.
pub fn check_invariants_and_endpoint_rules_supported(
    generator: &dyn DynGenerator,
) -> Result<(), String> {
    let contracts = [
        ir_with_rule(),
        ir_with_endpoint_rules(),
        compiled_endpoint_rules_contract(),
    ];
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        for f in Feature::RULES {
            let want = matches!(f, Feature::RulesInvariants | Feature::RulesEndpoint);
            if caps.supports(f) != want {
                return Err(format!(
                    "{}: `{}` should be supported: {want}",
                    role.name(),
                    f.name()
                ));
            }
        }
        for ir in &contracts {
            let req = GenRequest {
                ir,
                role: *role,
                services: all_services(ir),
                consumes: vec![],
                framework: None,
            };
            let gate = check_capabilities(ir, &req, &caps);
            if !gate.is_empty() {
                return Err(format!("{}: the gate reported {gate:?}", role.name()));
            }
            if let Err(d) = generator.generate_dyn(&req, &serde_json::Value::Null) {
                return Err(format!("{}: generate failed: {d:?}", role.name()));
            }
        }
    }
    Ok(())
}

/// A generator that supports invariants and nothing else of the rules: it declares `rules_invariants`
/// only, and generates a contract with a rule (the other rules features stay refused).
///
/// # Errors
/// A description of the first role that does not match.
pub fn check_only_invariants_supported(generator: &dyn DynGenerator) -> Result<(), String> {
    let ir = ir_with_rule();
    let services = all_services(&ir);
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        for f in Feature::RULES {
            let want = f == Feature::RulesInvariants;
            if caps.supports(f) != want {
                return Err(format!(
                    "{}: `{}` should be supported: {want}",
                    role.name(),
                    f.name()
                ));
            }
        }
        let req = GenRequest {
            ir: &ir,
            role: *role,
            services: services.clone(),
            consumes: vec![],
            framework: None,
        };
        let gate = check_capabilities(&ir, &req, &caps);
        if !gate.is_empty() {
            return Err(format!("{}: the gate reported {gate:?}", role.name()));
        }
        if let Err(d) = generator.generate_dyn(&req, &serde_json::Value::Null) {
            return Err(format!("{}: generate failed: {d:?}", role.name()));
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
    let unsupported: Vec<Feature> = checked_features(&ir, &req)
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
        // covers `rules_invariants` by hand. `rules_rollout` is covered by `compiled_rollout_contracts`.
        for feature in Feature::ALL.into_iter().filter(|f| {
            *f != Feature::Consts && *f != Feature::RulesRollout && !Feature::RULES.contains(f)
        }) {
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
    fn the_hand_built_endpoint_rules_use_the_endpoint_feature() {
        let ir = ir_with_endpoint_rules();
        let used = ir.features_used(&all_services(&ir));
        assert!(used.contains(&Feature::RulesEndpoint), "{used:?}");
        assert!(!used.contains(&Feature::RulesInvariants), "{used:?}");
    }

    #[test]
    fn a_compiled_block_uses_the_endpoint_feature() {
        let ir = compiled_endpoint_rules_contract();
        let used = ir.features_used(&all_services(&ir));
        assert!(used.contains(&Feature::RulesEndpoint), "{used:?}");
        let ep = &ir.services[0].endpoints[0];
        assert_eq!(ep.rules.facts.len(), 1);
        assert_eq!((ep.rules.requires.len(), ep.rules.ensures.len()), (1, 1));
    }

    #[test]
    fn a_generator_that_claims_a_compiled_block_is_caught() {
        let claims = Minimal {
            embed: true,
            caps: Capabilities::all_except(&[]),
        };
        assert!(check_compiled_endpoint_rules_are_refused(&claims).is_err());
    }

    #[test]
    fn the_state_machine_contracts_use_the_transitions_feature() {
        for (ir, line) in [
            compiled_state_machine_contract(),
            compiled_transition_contract(),
        ] {
            let used = ir.features_used(&all_services(&ir));
            assert!(used.contains(&Feature::RulesTransitions), "{used:?}");
            assert!(line >= 1);
        }
        let (ir, _) = compiled_state_machine_contract();
        let used = ir.features_used(&all_services(&ir));
        assert!(!used.contains(&Feature::RulesEndpoint), "{used:?}");
        let (ir, _) = compiled_transition_contract();
        assert!(ir.services[0].endpoints[0].rules.transition.is_some());
    }

    #[test]
    fn a_generator_that_claims_state_machines_is_caught() {
        let claims = Minimal {
            embed: true,
            caps: Capabilities::all_except(&[]),
        };
        assert!(check_state_machines_are_refused(&claims).is_err());
    }

    #[test]
    fn the_rollout_contracts_use_the_rollout_feature_at_a_known_line() {
        let contracts = compiled_rollout_contracts();
        assert_eq!(contracts.len(), 3);
        for (ir, line) in contracts {
            let used = ir.features_used(&all_services(&ir));
            assert!(used.contains(&Feature::RulesRollout), "{used:?}");
            assert!(line > 1);
        }
    }

    #[test]
    fn a_generator_that_claims_rollout_is_caught() {
        let claims = Minimal {
            embed: true,
            caps: Capabilities::all_except(&[]).with(Feature::RulesRollout),
        };
        let err = check_rollout_is_refused(&claims).unwrap_err();
        assert!(err.contains("declares `rules_rollout`"), "{err}");
    }

    #[test]
    fn a_generator_that_refuses_rollout_is_caught_by_the_generating_check() {
        let refuses = Minimal {
            embed: true,
            caps: Capabilities::all_except(&[]),
        };
        let err = check_rollout_is_generated(&refuses).unwrap_err();
        assert!(err.contains("does not declare `rules_rollout`"), "{err}");
        let opted_in = Minimal {
            embed: true,
            caps: Capabilities::all_except(&[]).with(Feature::RulesRollout),
        };
        check_rollout_is_generated(&opted_in).unwrap();
    }

    #[test]
    fn a_generator_that_does_not_gate_rollout_is_caught() {
        let ungated = Minimal {
            embed: true,
            caps: Capabilities::default(),
        };
        let err = check_rollout_is_refused(&ungated).unwrap_err();
        assert!(err.contains("did not answer E0801"), "{err}");
    }

    #[test]
    fn the_policy_contracts_use_the_policies_feature_only_with_an_allow() {
        let (ir, line) = compiled_policy_contract();
        let used = ir.features_used(&all_services(&ir));
        assert!(used.contains(&Feature::RulesPolicies), "{used:?}");
        assert!(line >= 1);
        assert_eq!(ir.services[0].endpoints[0].rules.allows.len(), 1);
        let (read, line) = compiled_principal_read_contract();
        let used = read.features_used(&all_services(&read));
        assert!(used.contains(&Feature::RulesPolicies), "{used:?}");
        assert!(read.services[0].endpoints[0].rules.allows.is_empty());
        assert!(line >= 1);
        let unused = compiled_unused_policy_contract();
        assert_eq!(unused.policies.len(), 1);
        let used = unused.features_used(&all_services(&unused));
        assert!(!used.contains(&Feature::RulesPolicies), "{used:?}");
    }

    #[test]
    fn a_generator_that_claims_policies_is_caught() {
        let claims = Minimal {
            embed: true,
            caps: Capabilities::all_except(&[]),
        };
        assert!(check_policies_are_refused(&claims).is_err());
    }

    #[test]
    fn a_generator_that_does_not_gate_policies_is_caught() {
        let ungated = Minimal {
            embed: true,
            caps: Capabilities::default(),
        };
        let err = check_policies_are_refused(&ungated).unwrap_err();
        assert!(err.contains("did not answer E0801"), "{err}");
    }

    #[test]
    fn a_generator_that_does_not_gate_state_machines_is_caught() {
        // It declares nothing, so the gate fires, but `generate` happily produces files.
        let ungated = Minimal {
            embed: true,
            caps: Capabilities::default(),
        };
        let err = check_state_machines_are_refused(&ungated).unwrap_err();
        assert!(err.contains("did not answer E0801"), "{err}");
    }

    #[test]
    fn a_generator_that_claims_endpoint_rules_is_caught() {
        let claims = Minimal {
            embed: true,
            caps: Capabilities::all_except(&[]),
        };
        assert!(check_endpoint_rules_are_refused(&claims).is_err());
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

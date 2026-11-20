//! File templates for `cargo xtask new-emitter`. Placeholders: `{{name}}`, `{{Struct}}`,
//! `{{roles}}` (Rust list), `{{roles_text}}` (prose).

pub const CARGO_TOML: &str = r#"[package]
name = "veld-emit-{{name}}"
description = "Veld {{Struct}} generator ({{roles_text}})"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
veld-ir.workspace = true
veld-codegen.workspace = true
serde.workspace = true

[dev-dependencies]
veld-emit-conformance.workspace = true
serde_json.workspace = true

[lints]
workspace = true
"#;

pub const LIB_RS: &str = r#"//! The `{{name}}` generator ({{roles_text}}).
//!
//! Scaffolded by `cargo xtask new-emitter {{name}}`. Start with README.md in this crate.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use serde::Deserialize;
use veld_codegen::{
    Capabilities, FileSet, FrameworkInfo, GenDiagnostic, GenRequest, Generator, LanguageInfo, Profile,
    Role,
};

/// Options of the `{{name}}` generator (none yet).
// TODO({{name}}): add typed options here; they come from `[targets.options]` in veld.toml.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {}

/// The `{{name}}` generator.
#[derive(Debug, Default, Clone, Copy)]
pub struct {{Struct}};

impl Generator for {{Struct}} {
    type Options = Options;

    fn id(&self) -> &'static str {
        "{{name}}"
    }

    fn roles(&self) -> &'static [Role] {
        &[{{roles}}]
    }

    fn frameworks(&self, _role: Role) -> Vec<FrameworkInfo> {
        // TODO({{name}}): list the framework adapters as ONE registry slice of `FrameworkAdapter`s
        // (see `crates/emitters/java/src/frameworks`) and return `framework_infos(ADAPTERS)`.
        Vec::new()
    }

    fn language(&self) -> Option<LanguageInfo> {
        // TODO({{name}}): return the language and its supported versions (the first is the default)
        // to let targets pick one with the option `<language> = "<version>"`.
        None
    }

    fn capabilities(&self, _role: Role) -> Capabilities {
        // EMPTY on purpose. While a feature is not declared here, the host reports `E0801` for
        // every contract that uses it, so this generator can never emit wrong code for it.
        // TODO({{name}}): add one `Feature` at a time, only after its output is implemented,
        // snapshot-tested and round-tripped through the language's real toolchain.
        Capabilities::default()
    }

    fn generate(
        &self,
        req: &GenRequest<'_>,
        _opts: &Options,
        _profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        let mut services = req.services.clone();
        services.extend(req.consumes.iter().copied());
        let mut files = FileSet::new();
        // TODO({{name}}): replace this placeholder. It writes one file per declaration and one
        // per service so that the signature and determinism checks already pass.
        for id in req.ir.reachable_decls(&services) {
            let decl = req.ir.decl(id);
            let text = format!("// TODO({{name}}): emit `{}`\n// sig: {}\n", decl.name, decl.sig);
            add(&mut files, format!("types/{}.txt", stem(&decl.qualified)), text)?;
        }
        for sid in &services {
            let svc = req.ir.service(*sid);
            let mut text = format!("// TODO({{name}}): emit the `{}` service\n", svc.name);
            for ep in &svc.endpoints {
                text.push_str(&format!("// {} {} -> sig: {}\n", ep.name, ep.path, ep.sig));
            }
            add(&mut files, format!("services/{}_{}.txt", sid.0, stem(&svc.name)), text)?;
        }
        Ok(files)
    }
}

/// A file-name-safe form of a qualified name.
fn stem(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}

fn add(files: &mut FileSet, path: String, text: String) -> Result<(), Vec<GenDiagnostic>> {
    files.add(path, text).map_err(|e| vec![GenDiagnostic::new("E0899", e.to_string())])
}
"#;

pub const CONFORMANCE_RS: &str = r#"//! The shared checks every generator must pass: capability coverage, signatures, determinism.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use veld_codegen::{DynGenerator, GenRequest, Role};
use veld_emit_conformance::{FIXTURES, check_generator, check_signatures, compile};
use veld_emit_{{name}}::{{Struct}};

#[test]
fn passes_the_shared_conformance_suite() {
    check_generator(&{{Struct}});
}

/// Every `sig` in the IR (declarations and operations) must appear verbatim in the output.
#[test]
fn every_generated_sig_equals_the_ir_sig() {
    let plain = FIXTURES.iter().find(|f| f.feature.is_none()).unwrap();
    let ir = compile(plain.name, plain.source);
    let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    for role in {{Struct}}.roles() {
        let req = GenRequest { ir: &ir, role: *role, services: services.clone(), consumes: vec![], framework: None };
        let files = {{Struct}}.generate_dyn(&req, &serde_json::Value::Null).unwrap();
        check_signatures(&ir, &services, &files).unwrap();
    }
}

#[test]
fn output_is_deterministic() {
    let plain = FIXTURES.iter().find(|f| f.feature.is_none()).unwrap();
    let ir = compile(plain.name, plain.source);
    let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    let req = GenRequest { ir: &ir, role: {{Struct}}.roles()[0], services, consumes: vec![], framework: None };
    let a = {{Struct}}.generate_dyn(&req, &serde_json::Value::Null).unwrap();
    let b = {{Struct}}.generate_dyn(&req, &serde_json::Value::Null).unwrap();
    assert_eq!(a, b);
}

#[test]
fn a_capability_that_is_not_declared_is_reported_not_silently_ignored() {
    for fixture in FIXTURES.iter().filter(|f| f.feature.is_some()) {
        let ir = compile(fixture.name, fixture.source);
        let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
        let role = {{Struct}}.roles()[0];
        let req = GenRequest { ir: &ir, role, services, consumes: vec![], framework: None };
        let caps = {{Struct}}.capabilities(role);
        let feature = fixture.feature.unwrap();
        let reported = veld_codegen::check_capabilities(&ir, &req, &caps).iter().any(|d| d.code == "E0801");
        assert_eq!(reported, !caps.supports(feature), "{}", fixture.name);
    }
}
"#;

pub const README_MD: &str = r#"# veld-emit-{{name}}

The `{{name}}` generator ({{roles_text}}). Scaffolded by `cargo xtask new-emitter {{name}}`.

Read `docs/emitter-guide.md` first: it explains the generator contract, signatures and the
test suites. This file is your checklist.

## Where things stand

`capabilities()` is **empty**, so every contract that uses a feature reports `E0801`, and
`generate` writes one placeholder file per declaration. The shared conformance test
(`tests/conformance.rs`) passes already. It must keep passing at every step.

## Checklist

Work top to bottom. Do not skip a step, and flip **one capability at a time**.

1. [ ] **Types.** Map every `Scalar` to a native type (`veld_ir::Scalar::ALL`), and `List`, `Map`
   and `Decl` references. Put the mapping in one module (`types.rs`) with unit tests.
2. [ ] **Models.** Emit `model`, `enum` and `error` declarations. Use `veld_codegen::naming` for
   identifiers (casing and reserved words). Embed `decl.sig` **verbatim** in each declaration
   (a comment or a constant). Never compute a signature yourself.
3. [ ] **Validation.** Generate checks for `constraints` (`@min`, `@max`, `@length`, `@pattern`,
   `@format`). Then declare `Feature::Constraints`.
4. [ ] **Client** (role `client`). One method per endpoint: path, query, header and body binding,
   typed errors from `ep.errors`, the success status and type. Embed `ep.sig` verbatim.
5. [ ] **Server** (role `server`). One handler or interface per endpoint, plus the router
   adapter for each framework you list in `frameworks()`.
6. [ ] **Toolchain round trip.** Compile the generated code with the language's real compiler, and
   run it against a generated peer (for example: your client against the `node` server). Skip
   the test with a message when the toolchain is missing, but fail when
   `VELD_REQUIRE_TOOLCHAINS=1` is set.
7. [ ] **Snapshots.** Add `insta` snapshots of the generated tree for the fixtures in
   `veld-emit-conformance`. Review every snapshot by hand.

## Flipping a capability

For each `Feature` (`veld_ir::Feature::ALL`):

1. Implement the output for it.
2. Add it to `capabilities()`.
3. Run `cargo test -p veld-emit-{{name}}`. The conformance suite now generates the matching
   fixture and checks its signatures and determinism.
4. Add a snapshot and a toolchain round trip for it.

Never declare a capability you did not implement: the host would then emit wrong code instead
of `E0801`.

## Definition of done

- [ ] `cargo test -p veld-emit-{{name}}` is green, with `VELD_REQUIRE_TOOLCHAINS=1`.
- [ ] `cargo clippy -p veld-emit-{{name}} --all-targets -- -D warnings` is clean, and the crate has
  no `unwrap`, `expect` or `panic` outside tests.
- [ ] Every capability you declare has a snapshot and a toolchain round trip.
- [ ] Every other feature is reported as `E0801` by `veld generate` (try it on a contract that
  uses it).
- [ ] The generated code has zero runtime dependencies, apart from the framework the user chose.
- [ ] Output is deterministic: no timestamps, absolute paths or host names; sorted iteration.
- [ ] Generated files never import anything the user must install for type-only use.
- [ ] A `spec/targets/{{name}}.md` page describes the type mapping, options and frameworks.
"#;

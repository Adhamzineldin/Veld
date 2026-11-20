//! The protocol conformance suite behind `veld plugin test`.
//!
//! It runs a plugin against a sample IR and checks what the host would check (handshake, file
//! paths and limits) plus what first-party plugins must also satisfy: deterministic output and
//! embedded signatures.

use veld_codegen::{GenRequest, Role};
use veld_ir::{Ir, ServiceId};

use crate::error::PluginError;
use crate::generator::Runner;
use crate::protocol::Handshake;

/// One named check and how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub result: Result<(), String>,
}

impl Check {
    fn new(name: impl Into<String>, result: Result<(), String>) -> Self {
        Self {
            name: name.into(),
            result,
        }
    }
}

/// True when every check passed.
#[must_use]
pub fn all_passed(checks: &[Check]) -> bool {
    checks.iter().all(|c| c.result.is_ok())
}

/// Runs every check against `runner`'s plugin with `ir` as the sample contract.
#[must_use]
pub fn run(runner: &Runner, ir: &Ir) -> Vec<Check> {
    let exe = match runner.command.resolve(&runner.config.project_root) {
        Ok(p) => p,
        Err(m) => return vec![Check::new("start", Err(m))],
    };
    let hello = match runner.handshake(&exe) {
        Ok(h) => h,
        Err(e) => return vec![Check::new("handshake", Err(e.to_string()))],
    };
    let mut checks = vec![Check::new("handshake", Ok(()))];
    for role in hello.roles.clone() {
        checks.extend(role_checks(runner, &exe, &hello, ir, role));
    }
    checks
}

fn role_checks(
    runner: &Runner,
    exe: &std::path::Path,
    _hello: &Handshake,
    ir: &Ir,
    role: Role,
) -> Vec<Check> {
    let services: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    let req = GenRequest {
        ir,
        role,
        services: services.clone(),
        consumes: vec![],
        framework: None,
    };
    let label = |what: &str| format!("{what} ({})", role.name());
    let first = match runner.generate(exe, &req, &serde_json::Value::Null) {
        Ok(o) => o,
        Err(e) => return failed_generate(&label, &e),
    };
    let errors: Vec<String> = first
        .diagnostics
        .iter()
        .filter(|d| !crate::protocol::is_warning(d))
        .map(|d| d.message.clone())
        .collect();
    let mut checks = vec![
        Check::new(label("path safety"), Ok(())),
        Check::new(
            label("generate"),
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors.join("; "))
            },
        ),
    ];
    let second = runner.generate(exe, &req, &serde_json::Value::Null);
    checks.push(Check::new(
        label("determinism"),
        same_output(&first.files, second),
    ));
    checks.push(Check::new(
        label("signatures"),
        signatures_embedded(ir, &services, &first.files),
    ));
    checks
}

fn failed_generate(label: &dyn Fn(&str) -> String, e: &PluginError) -> Vec<Check> {
    let name = if e.code() == "E0707" {
        "path safety"
    } else {
        "generate"
    };
    vec![Check::new(label(name), Err(e.to_string()))]
}

fn same_output(
    first: &veld_codegen::FileSet,
    second: Result<crate::protocol::Outcome, PluginError>,
) -> Result<(), String> {
    let second = second.map_err(|e| format!("the second run failed: {e}"))?;
    if *first == second.files {
        return Ok(());
    }
    let differing: Vec<&str> = first
        .paths()
        .chain(second.files.paths())
        .filter(|p| first.get(p) != second.files.get(p))
        .collect();
    let mut unique: Vec<&str> = differing;
    unique.sort_unstable();
    unique.dedup();
    Err(format!(
        "two runs on the same IR differ in: {}",
        unique
            .iter()
            .take(5)
            .copied()
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Every declaration and operation signature of the request must appear in some generated file.
fn signatures_embedded(
    ir: &Ir,
    services: &[ServiceId],
    files: &veld_codegen::FileSet,
) -> Result<(), String> {
    let mut sigs: Vec<&str> = ir
        .reachable_decls(services)
        .into_iter()
        .map(|d| ir.decl(d).sig.as_str())
        .collect();
    for sid in services {
        sigs.extend(ir.service(*sid).endpoints.iter().map(|e| e.sig.as_str()));
    }
    let contents: Vec<String> = files
        .iter()
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect();
    let missing: Vec<&str> = sigs
        .into_iter()
        .filter(|s| !contents.iter().any(|c| c.contains(s)))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} signature(s) are not embedded, for example `{}`",
            missing.len(),
            missing[0]
        ))
    }
}

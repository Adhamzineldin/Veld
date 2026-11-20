//! Shared helpers: compile the realistic contract and run the Java generator.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_java::Java;
use veld_ir::{Ir, PackageKind, ServiceId};

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn read_dir(dir: &str, files: &[&str], patch: &dyn Fn(&str) -> String) -> Vec<SourceFile> {
    files
        .iter()
        .map(|f| {
            let path = repo_root().join("tests/toolchain/contract").join(dir).join(f);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            SourceFile { path: (*f).to_string(), text: patch(&text) }
        })
        .collect()
}

/// Compiles the contract. `orders_pkg` renames the orders package; `patch` edits its sources.
pub fn compile_contract(orders_pkg: &str, patch: &dyn Fn(&str) -> String) -> Ir {
    let identity = |s: &str| s.to_string();
    let out = compile(&CompileInput {
        packages: vec![
            PackageInput {
                name: "iam".into(),
                kind: PackageKind::Service,
                files: read_dir("iam", &["users.veld"], &identity),
                base_url: Some("http://iam:3001".into()),
            },
            PackageInput {
                name: orders_pkg.into(),
                kind: PackageKind::Service,
                files: read_dir("orders", &["catalog.veld", "orders.veld"], patch),
                base_url: None,
            },
        ],
        consumes: BTreeMap::from([(orders_pkg.to_string(), vec!["iam".to_string()])]),
    });
    assert!(out.ir.is_some(), "contract failed to compile:\n{}", out.render());
    out.ir.unwrap_or_else(|| unreachable!())
}

pub fn standard_ir() -> Ir {
    compile_contract("orders", &|s| s.to_string())
}

pub fn service_id(ir: &Ir, name: &str) -> ServiceId {
    ir.services.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no service {name}")).id
}

pub fn run(
    ir: &Ir,
    role: Role,
    services: Vec<ServiceId>,
    consumes: Vec<ServiceId>,
    framework: Option<&str>,
    options: serde_json::Value,
) -> FileSet {
    let req = GenRequest { ir, role, services, consumes, framework: framework.map(str::to_string) };
    let caps = Java.capabilities(role);
    let diags = check_capabilities(ir, &req, &caps);
    assert!(diags.is_empty(), "capability errors: {diags:?}");
    Java.generate_dyn(&req, &options).unwrap_or_else(|d| panic!("generation failed: {d:?}"))
}

/// Client files for every service of the IR, in the Java package `package`.
pub fn gen_client(ir: &Ir, package: &str) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, Role::Client, all, vec![], None, serde_json::json!({ "package": package }))
}

/// Server files for `service`, with clients for `consumes`.
pub fn gen_server(ir: &Ir, service: &str, consumes: &[&str], package: &str, framework: Option<&str>) -> FileSet {
    let consumed = consumes.iter().map(|n| service_id(ir, n)).collect();
    run(ir, Role::Server, vec![service_id(ir, service)], consumed, framework, serde_json::json!({ "package": package }))
}

pub fn text<'a>(files: &'a FileSet, path: &str) -> &'a str {
    std::str::from_utf8(files.get(path).unwrap_or_else(|| panic!("missing generated file {path}"))).unwrap_or_default()
}

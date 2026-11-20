//! Shared helpers: compile the realistic contract and run the generators.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_node::Node;
use veld_emit_typescript::TypeScript;
use veld_ir::{Ir, PackageKind, ServiceId};

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn read_dir(dir: &str, files: &[&str], patch: &dyn Fn(&str) -> String) -> Vec<SourceFile> {
    files
        .iter()
        .map(|f| {
            let path = repo_root()
                .join("tests/toolchain/contract")
                .join(dir)
                .join(f);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            SourceFile {
                path: (*f).to_string(),
                text: patch(&text),
            }
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
    assert!(
        out.ir.is_some(),
        "contract failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

pub fn standard_ir() -> Ir {
    compile_contract("orders", &|s| s.to_string())
}

pub fn service_id(ir: &Ir, name: &str) -> ServiceId {
    ir.services
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no service {name}"))
        .id
}

fn run(
    generator: &dyn DynGenerator,
    ir: &Ir,
    role: Role,
    services: Vec<ServiceId>,
    consumes: Vec<ServiceId>,
    options: &serde_json::Value,
    framework: Option<&str>,
) -> FileSet {
    let req = GenRequest {
        ir,
        role,
        services,
        consumes,
        framework: framework.map(str::to_string),
    };
    let caps = generator.capabilities(role);
    let diags = check_capabilities(ir, &req, &caps);
    assert!(diags.is_empty(), "capability errors: {diags:?}");
    generator
        .generate_dyn(&req, options)
        .unwrap_or_else(|d| panic!("generation failed: {d:?}"))
}

/// The language versions every toolchain test compiles for: the target option `typescript` and
/// the matching `tsc --target`.
pub const TARGETS: [(&str, &str); 2] = [("es2020", "ES2020"), ("es2022", "ES2022")];

/// The target options that select the ECMAScript edition `edition`.
pub fn typescript_option(edition: &str) -> serde_json::Value {
    serde_json::json!({ "typescript": edition })
}

/// Client files for every service of the IR.
pub fn gen_client(ir: &Ir) -> FileSet {
    gen_client_with(ir, &serde_json::Value::Null)
}

/// Client files for every service of the IR, with target options.
pub fn gen_client_with(ir: &Ir, options: &serde_json::Value) -> FileSet {
    gen_client_fw(ir, options, None)
}

/// Client files for every service of the IR, with target options and a framework.
pub fn gen_client_fw(ir: &Ir, options: &serde_json::Value, framework: Option<&str>) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(
        &TypeScript,
        ir,
        Role::Client,
        all,
        vec![],
        options,
        framework,
    )
}

/// Server files for `service`, with clients for `consumes`.
pub fn gen_server(ir: &Ir, service: &str, consumes: &[&str]) -> FileSet {
    gen_server_with(ir, service, consumes, &serde_json::Value::Null)
}

/// Server files for `service`, with clients for `consumes`, with target options.
pub fn gen_server_with(
    ir: &Ir,
    service: &str,
    consumes: &[&str],
    options: &serde_json::Value,
) -> FileSet {
    gen_server_fw(ir, service, consumes, options, None)
}

/// Server files for `service`, with clients for `consumes`, target options and a framework.
pub fn gen_server_fw(
    ir: &Ir,
    service: &str,
    consumes: &[&str],
    options: &serde_json::Value,
    framework: Option<&str>,
) -> FileSet {
    let consumed = consumes.iter().map(|n| service_id(ir, n)).collect();
    run(
        &Node,
        ir,
        Role::Server,
        vec![service_id(ir, service)],
        consumed,
        options,
        framework,
    )
}

pub fn text<'a>(files: &'a FileSet, path: &str) -> &'a str {
    std::str::from_utf8(
        files
            .get(path)
            .unwrap_or_else(|| panic!("missing generated file {path}")),
    )
    .unwrap_or_default()
}

/// The hazard fixture: every identifier that is dangerous in TypeScript, JavaScript or Java,
/// used for endpoints, fields, models and enum variants (`tests/toolchain/contract/hazards`).
pub fn hazards_ir() -> Ir {
    let path = repo_root().join("tests/toolchain/contract/hazards/hazards.veld");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "weird".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "hazards.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "hazard fixture failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

/// The HTTP equality fixture: one package with two services (`Alpha`, `Beta`) whose `fetch`
/// endpoints share name, path and errors (`tests/toolchain/contract/equality`).
pub fn equality_ir() -> Ir {
    let path = repo_root().join("tests/toolchain/contract/equality/shop.veld");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "shop".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "shop.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "equality fixture failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

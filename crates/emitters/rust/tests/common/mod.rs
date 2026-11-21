//! Shared helpers: compile the realistic contracts and run the Rust generator.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_rust::Rust;
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

fn compiled(input: &CompileInput, what: &str) -> Ir {
    let out = compile(input);
    assert!(
        out.ir.is_some(),
        "{what} failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

/// The orders contract: `orders` consumes `iam`. `orders_pkg` renames the orders package; `patch`
/// edits its sources.
pub fn compile_contract(orders_pkg: &str, patch: &dyn Fn(&str) -> String) -> Ir {
    let identity = |s: &str| s.to_string();
    compiled(
        &CompileInput {
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
        },
        "contract",
    )
}

pub fn standard_ir() -> Ir {
    compile_contract("orders", &|s| s.to_string())
}

fn single(name: &str, dir: &str, file: &str) -> Ir {
    let path = repo_root()
        .join("tests/toolchain/contract")
        .join(dir)
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    compiled(
        &CompileInput {
            packages: vec![PackageInput {
                name: name.into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: file.into(),
                    text,
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        },
        dir,
    )
}

/// The hazard fixture: identifiers that are dangerous in several languages.
pub fn hazards_ir() -> Ir {
    single("weird", "hazards", "hazards.veld")
}

/// Two models with identical fields in one contract (`tests/toolchain/contract/equality`).
pub fn equality_ir() -> Ir {
    single("shop", "equality", "shop.veld")
}

/// The streaming fixture: SSE and WebSocket endpoints.
pub fn streaming_ir() -> Ir {
    single("live", "streaming", "live.veld")
}

/// The streaming fixture with the WebSocket endpoints cut out (what a generator without
/// WebSocket support can serve).
pub fn streaming_sse_ir() -> Ir {
    let path = repo_root().join("tests/toolchain/contract/streaming/live.veld");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out: Vec<&str> = Vec::new();
    let mut docs: Vec<&str> = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        let indented = line.len() != t.len();
        if indented && t.starts_with("///") {
            docs.push(line);
        } else if indented && t.starts_with("ws ") {
            docs.clear();
        } else {
            out.append(&mut docs);
            out.push(line);
        }
    }
    compiled(
        &CompileInput {
            packages: vec![PackageInput {
                name: "live".into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: "live.veld".into(),
                    text: out.join("\n"),
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        },
        "streaming (sse only)",
    )
}

pub fn greeter_ir() -> Ir {
    single("greeter", "validate", "greeter.veld")
}

pub fn service_id(ir: &Ir, name: &str) -> ServiceId {
    ir.services
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no service {name}"))
        .id
}

pub fn run(
    ir: &Ir,
    role: Role,
    services: Vec<ServiceId>,
    consumes: Vec<ServiceId>,
    framework: Option<&str>,
    options: serde_json::Value,
) -> FileSet {
    let req = GenRequest {
        ir,
        role,
        services,
        consumes,
        framework: framework.map(str::to_string),
    };
    let caps = Rust.capabilities(role);
    let diags = check_capabilities(ir, &req, &caps);
    assert!(diags.is_empty(), "capability errors: {diags:?}");
    Rust.generate_dyn(&req, &options)
        .unwrap_or_else(|d| panic!("generation failed: {d:?}"))
}

fn named(name: &str) -> serde_json::Value {
    serde_json::json!({ "crate-name": name })
}

/// A client crate for every service of the IR.
pub fn gen_client(ir: &Ir, name: &str, framework: Option<&str>) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, Role::Client, all, vec![], framework, named(name))
}

/// A server crate for `service`, with clients for `consumes`.
pub fn gen_server(ir: &Ir, service: &str, consumes: &[&str], name: &str) -> FileSet {
    let consumed = consumes.iter().map(|n| service_id(ir, n)).collect();
    run(
        ir,
        Role::Server,
        vec![service_id(ir, service)],
        consumed,
        None,
        named(name),
    )
}

/// A server crate for several services of one package.
pub fn gen_server_services(ir: &Ir, services: &[&str], name: &str) -> FileSet {
    run(
        ir,
        Role::Server,
        services.iter().map(|n| service_id(ir, n)).collect(),
        vec![],
        None,
        named(name),
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

/// Writes a file set below `dir` (replacing what was there).
pub fn write_files(dir: &Path, files: &FileSet) {
    let _ = std::fs::remove_dir_all(dir);
    for (path, bytes) in files.iter() {
        let target = dir.join(path);
        std::fs::create_dir_all(target.parent().unwrap_or(dir))
            .unwrap_or_else(|e| panic!("{}: {e}", target.display()));
        std::fs::write(&target, bytes).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
    }
}

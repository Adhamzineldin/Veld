//! Shared helpers: compile the realistic contract and run the Java generator.
#![allow(dead_code)]

pub mod live;
pub mod p4;

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

/// Compiles one `.veld` file of the repository (a path relative to the repository root, for
/// example `tests/rules/endpoints/transitions.veld`) as the single package `package`.
pub fn compile_path(rel: &str, package: &str) -> Ir {
    let text =
        std::fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    compile_source(text, package, rel)
}

/// Compiles `text` as the single package `package`; `what` names it in a failure.
pub fn compile_source(text: String, package: &str, what: &str) -> Ir {
    let rel = what;
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: package.into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: format!("{package}.veld"),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let rendered = out.render();
    out.ir
        .unwrap_or_else(|| panic!("{rel} does not compile:\n{rendered}"))
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
    let caps = Java.capabilities(role);
    let diags = check_capabilities(ir, &req, &caps);
    assert!(diags.is_empty(), "capability errors: {diags:?}");
    Java.generate_dyn(&req, &options)
        .unwrap_or_else(|d| panic!("generation failed: {d:?}"))
}

/// The Java releases every toolchain test compiles for: the target option `java` and the matching
/// `javac --release`. The first is the default.
pub const RELEASES: [&str; 2] = ["17", "21"];

/// [`RELEASES`] narrowed by the env var `VELD_JAVA_RELEASES` (a comma list, for example `17` on a
/// machine whose `javac` is JDK 17 and cannot compile `--release 21`).
pub fn releases() -> Vec<&'static str> {
    match std::env::var("VELD_JAVA_RELEASES") {
        Ok(only) => RELEASES
            .into_iter()
            .filter(|r| only.split(',').any(|o| o.trim() == *r))
            .collect(),
        Err(_) => RELEASES.to_vec(),
    }
}

/// Target options with the base package and, when given, the Java release.
pub fn options(package: &str, release: Option<&str>) -> serde_json::Value {
    let value = match release {
        Some(r) => serde_json::json!({ "package": package, "java": r }),
        None => serde_json::json!({ "package": package }),
    };
    veld_codegen::with_target_name(&value, &target_name(package))
}

/// The name of the target a test generates for: its package with `-` for `.` (`gen.a` is `gen-a`,
/// so the runtime config is `GenARuntime`).
pub fn target_name(package: &str) -> String {
    package.replace('.', "-")
}

/// Client files for every service of the IR, in the Java package `package`.
pub fn gen_client(ir: &Ir, package: &str) -> FileSet {
    gen_client_for(ir, package, None)
}

/// Like [`gen_client`], for the Java release `release`.
pub fn gen_client_for(ir: &Ir, package: &str, release: Option<&str>) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(
        ir,
        Role::Client,
        all,
        vec![],
        None,
        options(package, release),
    )
}

/// Server files for `service`, with clients for `consumes`.
pub fn gen_server(
    ir: &Ir,
    service: &str,
    consumes: &[&str],
    package: &str,
    framework: Option<&str>,
) -> FileSet {
    gen_server_for(ir, service, consumes, package, framework, None)
}

/// Like [`gen_server`], for the Java release `release`.
pub fn gen_server_for(
    ir: &Ir,
    service: &str,
    consumes: &[&str],
    package: &str,
    framework: Option<&str>,
    release: Option<&str>,
) -> FileSet {
    let consumed = consumes.iter().map(|n| service_id(ir, n)).collect();
    run(
        ir,
        Role::Server,
        vec![service_id(ir, service)],
        consumed,
        framework,
        options(package, release),
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

/// Server files for several services of one package.
pub fn gen_server_services(
    ir: &Ir,
    services: &[&str],
    package: &str,
    framework: Option<&str>,
    release: Option<&str>,
) -> FileSet {
    run(
        ir,
        Role::Server,
        services.iter().map(|n| service_id(ir, n)).collect(),
        vec![],
        framework,
        options(package, release),
    )
}

fn streaming_source() -> String {
    let path = repo_root().join("tests/toolchain/contract/streaming/live.veld");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The streaming contract without its `ws` endpoints (and the doc comments of those): what the
/// `jdk` framework can serve.
pub fn without_ws(source: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut docs: Vec<&str> = Vec::new();
    for line in source.lines() {
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
    out.join("\n")
}

fn compile_streaming(text: String) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "live".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "live.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "streaming fixture failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

/// The streaming fixture: SSE and WebSocket endpoints (`tests/toolchain/contract/streaming`).
pub fn streaming_ir() -> Ir {
    compile_streaming(streaming_source())
}

/// The streaming fixture with the SSE endpoints only.
pub fn streaming_sse_ir() -> Ir {
    compile_streaming(without_ws(&streaming_source()))
}

/// The `validate = false` fixture: one endpoint whose query parameter has a length constraint
/// (`tests/toolchain/contract/validate`).
pub fn validate_ir() -> Ir {
    let path = repo_root().join("tests/toolchain/contract/validate/greeter.veld");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "greeter".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "greeter.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "validate fixture failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

/// Generator options with `validate = false`.
pub fn options_no_validate(package: &str, release: Option<&str>) -> serde_json::Value {
    let mut o = options(package, release);
    o["validate"] = serde_json::json!(false);
    o
}

/// The `Greeter` server (JDK adapter) and its client in one package, generated with `validate = false`.
pub fn gen_unvalidated(ir: &Ir, role: Role, package: &str, release: Option<&str>) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(
        ir,
        role,
        all,
        vec![],
        None,
        options_no_validate(package, release),
    )
}

/// Whether a Maven log shows a download that failed, and not a build that failed.
fn is_download_failure(log: &str) -> bool {
    [
        "Failed to fetch",
        "Could not transfer artifact",
        "Connection reset",
        "Connection timed out",
        "Read timed out",
        "Temporary failure in name resolution",
        "502 Bad Gateway",
        "503 Service",
        // Maven Central limits how often one address may ask; the wrapper on Windows reports it
        // through PowerShell (`Exception calling "DownloadFile" ... (429) Too Many Requests`).
        "Too Many Requests",
        "(429)",
        "status code: 429",
        "Exception calling \"DownloadFile\"",
    ]
    .iter()
    .any(|sign| log.contains(sign))
}

/// Runs `mvnw -q -B package -DskipTests -Djava.version=<release>` in `work`, where the Maven
/// wrapper of `tests/toolchain/java/spring` has been copied, logging to `work/mvnw.log`. It panics
/// with the log when the build fails.
///
/// The wrapper downloads Maven, and Maven downloads Spring, from Maven Central. A download that
/// fails for a moment is not a finding about generated code (it failed a CI run once:
/// `wget: Failed to fetch .../apache-maven-3.9.9-bin.zip`), so a run whose log shows a failed
/// download is tried again, three times in all. Any other failure fails at once.
pub fn mvnw_package(work: &std::path::Path, release: &str) {
    const ATTEMPTS: u64 = 3;
    let log = work.join("mvnw.log");
    for attempt in 1..=ATTEMPTS {
        let mut mvn = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.args(["/c", r".\mvnw.cmd"]);
            c
        } else {
            let mut c = std::process::Command::new("sh");
            c.arg("./mvnw");
            c
        };
        mvn.args(["-q", "-B", "package", "-DskipTests"])
            .arg(format!("-Djava.version={release}"))
            .current_dir(work);
        let mut build = veld_test_support::ProcessTree::spawn_logged(&mut mvn, &log)
            .unwrap_or_else(|e| panic!("mvnw package: cannot start: {e}"));
        let status = build.child_mut().wait().unwrap();
        if status.success() {
            return;
        }
        let output = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            is_download_failure(&output) && attempt < ATTEMPTS,
            "mvnw package failed ({status}):\n{output}"
        );
        eprintln!("mvnw package: a download failed, trying again ({attempt} of {ATTEMPTS})");
        // A limit on requests needs more than a moment: 20 seconds, then 40.
        std::thread::sleep(std::time::Duration::from_secs(20 * attempt));
    }
}

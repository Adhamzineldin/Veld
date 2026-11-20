//! The host against a real child process (`veld-plugin-stub`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use veld_codegen::{DynGenerator, GenDiagnostic, GenRequest, Role, check_capabilities};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Feature, Ir, PackageKind};
use veld_plugin_host::conformance;
use veld_plugin_host::{CommandLine, HostConfig, PluginGenerator, Runner};
use veld_project::config::PluginSpec;

const SRC: &str = r#"/// A greeting.
model Greeting {
  message: string
}

/// Says hello.
service Hello at "/hello" {
  /// Greets.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}
"#;

fn ir() -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "hello".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "hello.veld".into(),
                text: SRC.into(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let rendered = out.render();
    out.ir.unwrap_or_else(|| panic!("{rendered}"))
}

fn stub_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_veld-plugin-stub"))
}

fn quote(p: &Path) -> String {
    format!("\"{}\"", p.display())
}

fn spec_for(exe: &Path, args: &str) -> PluginSpec {
    PluginSpec {
        name: "stub".into(),
        command: format!("{} {args}", quote(exe)),
        version: None,
        sha256: None,
    }
}

fn spec(args: &str) -> PluginSpec {
    spec_for(&stub_path(), args)
}

fn config() -> HostConfig {
    HostConfig::new(std::env::temp_dir())
}

fn request(ir: &Ir, role: Role) -> GenRequest<'_> {
    GenRequest {
        ir,
        role,
        services: vec![ir.services[0].id],
        consumes: vec![],
        framework: None,
    }
}

fn codes(diags: &[GenDiagnostic]) -> Vec<&'static str> {
    diags.iter().map(|d| d.code).collect()
}

fn sha_of(path: &Path) -> String {
    Sha256::digest(fs::read(path).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn happy_path_returns_files_with_signatures_and_declared_capabilities() {
    let ir = ir();
    let plugin = PluginGenerator::connect(&spec("--mode ok"), config()).unwrap();
    assert_eq!(plugin.id(), "stub");
    assert_eq!(plugin.roles(), [Role::Server, Role::Client]);
    let caps = plugin.capabilities(Role::Client);
    assert!(caps.supports(Feature::Auth) && !caps.supports(Feature::Unions));
    let files = plugin
        .generate_dyn(&request(&ir, Role::Client), &serde_json::Value::Null)
        .unwrap();
    let greeting = ir.decls.iter().find(|d| d.name == "Greeting").unwrap();
    let text = files
        .iter()
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains(&greeting.sig),
        "the signature is embedded verbatim"
    );
    assert!(files.len() >= 2);
}

#[test]
fn capability_check_applies_to_plugins_like_built_ins() {
    let ir = ir();
    let plugin = PluginGenerator::connect(&spec("--mode ok"), config()).unwrap();
    let req = request(&ir, Role::Client);
    assert!(check_capabilities(&ir, &req, &plugin.capabilities(Role::Client)).is_empty());
    let none = veld_codegen::Capabilities::default();
    let _ = none;
}

#[test]
fn plugin_runs_in_an_empty_directory_with_only_path_in_its_environment() {
    let ir = ir();
    let plugin = PluginGenerator::connect(&spec("--mode env"), config()).unwrap();
    let files = plugin
        .generate_dyn(&request(&ir, Role::Client), &serde_json::Value::Null)
        .unwrap();
    let text = String::from_utf8(files.get("env.txt").unwrap().to_vec()).unwrap();
    assert!(text.contains("cwd_entries=0"), "{text}");
    let keys = text
        .split("keys=")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap();
    let names: Vec<String> = keys
        .trim_start_matches('[')
        .split(", ")
        .map(|k| k.trim_matches('"').to_ascii_uppercase())
        .collect();
    assert!(
        names
            .iter()
            .all(|k| k == "PATH" || (cfg!(windows) && k == "SYSTEMROOT")),
        "{names:?}"
    );
    assert!(names.iter().any(|k| k == "PATH"), "{names:?}");
}

#[test]
fn handshake_timeout_kills_the_plugin() {
    let mut cfg = config();
    cfg.handshake_timeout = Duration::from_millis(400);
    let start = Instant::now();
    let err = PluginGenerator::connect(&spec("--mode hang-hello"), cfg).unwrap_err();
    assert_eq!(err.code(), "E0708");
    assert!(err.to_string().contains("killed"), "{err}");
    assert!(start.elapsed() < Duration::from_secs(4));
}

#[test]
fn generate_timeout_kills_the_plugin() {
    let ir = ir();
    let plugin = PluginGenerator::connect(&spec("--mode hang-generate"), config()).unwrap();
    let start = Instant::now();
    let err = plugin
        .generate_dyn(
            &request(&ir, Role::Client),
            &serde_json::json!({"timeout": 1}),
        )
        .unwrap_err();
    assert_eq!(codes(&err), ["E0708"]);
    assert!(err[0].message.contains("within 1 s"), "{}", err[0].message);
    assert!(start.elapsed() < Duration::from_secs(8));
}

#[test]
fn invalid_timeout_option_is_refused_before_starting_anything() {
    let ir = ir();
    let plugin = PluginGenerator::connect(&spec("--mode ok"), config()).unwrap();
    for bad in [0, 601] {
        let err = plugin
            .generate_dyn(
                &request(&ir, Role::Client),
                &serde_json::json!({"timeout": bad}),
            )
            .unwrap_err();
        assert_eq!(codes(&err), ["E0802"]);
    }
}

#[test]
fn escaping_paths_are_refused() {
    let ir = ir();
    for path in ["../x", "/abs/x", "C:/x", "a\\b", "a/../../b"] {
        let plugin = PluginGenerator::connect(
            &spec(&format!("--mode escape --path {}", quote(Path::new(path)))),
            config(),
        )
        .unwrap();
        let err = plugin
            .generate_dyn(&request(&ir, Role::Client), &serde_json::Value::Null)
            .unwrap_err();
        assert_eq!(codes(&err), ["E0707"], "{path}");
    }
}

#[test]
fn hash_is_checked_at_load_and_before_every_run() {
    let ir = ir();
    let tmp = tempfile::tempdir().unwrap();
    let exe = tmp.path().join(stub_path().file_name().unwrap());
    fs::copy(stub_path(), &exe).unwrap();
    let mut spec = spec_for(&exe, "--mode ok");
    spec.sha256 = Some("0".repeat(64));
    let err = PluginGenerator::connect(&spec, config()).unwrap_err();
    assert_eq!(err.code(), "E0705");
    spec.sha256 = Some(sha_of(&exe));
    let plugin = PluginGenerator::connect(&spec, config()).unwrap();
    assert!(
        plugin
            .generate_dyn(&request(&ir, Role::Client), &serde_json::Value::Null)
            .is_ok()
    );
    let mut bytes = fs::read(&exe).unwrap();
    bytes.push(0);
    fs::write(&exe, bytes).unwrap();
    let err = plugin
        .generate_dyn(&request(&ir, Role::Client), &serde_json::Value::Null)
        .unwrap_err();
    assert_eq!(
        codes(&err),
        ["E0705"],
        "a swapped executable is refused at run time"
    );
}

#[test]
fn protocol_and_version_mismatches_are_e0706() {
    let err = PluginGenerator::connect(&spec("--mode proto2"), config()).unwrap_err();
    assert_eq!(err.code(), "E0706");
    assert!(err.to_string().contains("protocol `2`"), "{err}");
    let err = PluginGenerator::connect(&spec("--mode garbage-hello"), config()).unwrap_err();
    assert_eq!(err.code(), "E0706");
    let mut pinned = spec("--version 1.0.0");
    pinned.version = Some("9.9.9".into());
    let err = PluginGenerator::connect(&pinned, config()).unwrap_err();
    assert_eq!(err.code(), "E0706");
    assert!(err.to_string().contains("pins version 9.9.9"), "{err}");
}

#[test]
fn crashes_and_non_zero_exits_are_e0708_and_yield_no_files() {
    let ir = ir();
    for mode in ["crash", "exit-after"] {
        let plugin = PluginGenerator::connect(&spec(&format!("--mode {mode}")), config()).unwrap();
        let err = plugin
            .generate_dyn(&request(&ir, Role::Client), &serde_json::Value::Null)
            .unwrap_err();
        assert_eq!(codes(&err), ["E0708"], "{mode}");
        assert!(err[0].message.contains("exit code 3"), "{}", err[0].message);
    }
}

#[test]
fn a_plugin_that_lingers_after_its_result_is_killed_without_failing_the_run() {
    let ir = ir();
    let mut cfg = config();
    cfg.exit_grace = Duration::from_millis(300);
    let plugin = PluginGenerator::connect(&spec("--mode linger"), cfg).unwrap();
    assert!(
        plugin
            .generate_dyn(&request(&ir, Role::Client), &serde_json::Value::Null)
            .is_ok()
    );
}

#[test]
fn plugin_error_diagnostics_fail_the_run_with_their_code() {
    let ir = ir();
    let plugin = PluginGenerator::connect(&spec("--mode error-diag"), config()).unwrap();
    let err = plugin
        .generate_dyn(&request(&ir, Role::Client), &serde_json::Value::Null)
        .unwrap_err();
    assert_eq!(codes(&err), ["E0801"]);
    assert!(err[0].message.contains("stub cannot do that"));
}

#[test]
fn unsupported_role_and_missing_executable_are_reported() {
    let ir = ir();
    let plugin = PluginGenerator::connect(&spec("--roles client"), config()).unwrap();
    let err = plugin
        .generate_dyn(&request(&ir, Role::Server), &serde_json::Value::Null)
        .unwrap_err();
    assert_eq!(codes(&err), ["E0706"]);
    let missing = PluginSpec {
        name: "x".into(),
        command: "./no-such-plugin".into(),
        version: None,
        sha256: None,
    };
    assert_eq!(
        PluginGenerator::connect(&missing, config())
            .unwrap_err()
            .code(),
        "E0708"
    );
}

fn runner(args: &str) -> Runner {
    let command = CommandLine::parse(&format!("{} {args}", quote(&stub_path()))).unwrap();
    Runner {
        name: "stub".into(),
        command,
        config: config(),
    }
}

#[test]
fn conformance_passes_for_a_well_behaved_plugin() {
    let checks = conformance::run(&runner("--mode ok"), &ir());
    let failed: Vec<_> = checks.iter().filter(|c| c.result.is_err()).collect();
    assert!(failed.is_empty(), "{failed:?}");
    let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
    for needed in [
        "handshake",
        "generate (client)",
        "determinism (server)",
        "signatures (client)",
        "path safety (server)",
    ] {
        assert!(names.contains(&needed), "{needed} missing in {names:?}");
    }
}

#[test]
fn conformance_catches_each_kind_of_misbehaviour() {
    let ir = ir();
    let failing = |args: &str| -> Vec<String> {
        conformance::run(&runner(args), &ir)
            .into_iter()
            .filter(|c| c.result.is_err())
            .map(|c| c.name)
            .collect()
    };
    assert!(
        failing("--mode nondeterministic")
            .iter()
            .any(|n| n.starts_with("determinism"))
    );
    assert!(
        failing("--mode nosig")
            .iter()
            .any(|n| n.starts_with("signatures"))
    );
    assert!(
        failing("--mode escape --path ../x")
            .iter()
            .any(|n| n.starts_with("path safety"))
    );
    assert_eq!(failing("--mode proto2"), ["handshake"]);
    assert!(
        failing("--mode crash")
            .iter()
            .any(|n| n.starts_with("generate"))
    );
}

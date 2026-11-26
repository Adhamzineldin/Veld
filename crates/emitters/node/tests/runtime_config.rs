//! The typed runtime config of the generated TypeScript (`spec/runtime-config.md`, the TypeScript
//! column): the goldens of `tests/wire/runtime-config/cases.json` through the generated
//! `checkRuntime` and `fromEnv` of one generated target per model of the file, the server entry of
//! a target whose only setting is optional, snapshots of the runtime module, and the files a
//! target with and without settings has.
//!
//! The toolchain tests need `node` and `tests/toolchain/ts/node_modules`; without them they are
//! skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use common::toolchain::{run, scratch, tsc_path};
use common::*;
use serde_json::{Value, json};
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_node::Node;
use veld_emit_typescript::TypeScript;
use veld_ir::{Ir, PackageKind};
use veld_project::output::{WriteOptions, write};

const IAM: &str =
    "model Id { n: int }\nservice Iam at \"/iam\" {\n  get \"/ping\" ping() -> Id\n}\n";
const PAYMENTS: &str = "model Receipt { n: int }\nservice Payments at \"/payments\" {\n  get \"/ping\" ping() -> Receipt\n}\n";
const BOOKINGS: &str = "model Booking {\n  start: int\n  end: int\n  rule order: end > start\n}\nservice Bookings at \"/bookings\" {\n  post \"/\" create(body: Booking) -> 201 Booking\n}\n";
const ORDERS: &str = "model Order {\n  low: int\n  high: int\n  rule order: high >= low\n}\nservice Orders at \"/orders\" {\n  post \"/\" create(body: Order) -> 201 Order\n  get \"/ping\" ping() -> int\n}\n";
const A: &str = "model Ping { n: int }\nservice A at \"/a\" {\n  get \"/ping\" ping() -> Ping\n}\n";
const AB: &str =
    "model Pong { n: int }\nservice Ab at \"/ab\" {\n  get \"/pong\" pong() -> Pong\n}\n";

fn workspace(packages: &[(&str, &str)], consumes: &[(&str, &[&str])]) -> Ir {
    let out = compile(&CompileInput {
        packages: packages
            .iter()
            .map(|(name, text)| PackageInput {
                name: (*name).to_string(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: format!("{name}.veld"),
                    text: (*text).to_string(),
                }],
                base_url: None,
            })
            .collect(),
        consumes: consumes
            .iter()
            .map(|(k, v)| {
                (
                    (*k).to_string(),
                    v.iter().map(|s| (*s).to_string()).collect(),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    });
    assert!(
        out.ir.is_some(),
        "the contract failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

/// One generated target per model of `cases.json`: the files, and where the driver finds them.
struct Generated {
    model: &'static str,
    files: FileSet,
}

fn id_of(ir: &Ir, service: &str) -> veld_ir::ServiceId {
    service_id(ir, service)
}

fn generate(
    generator: &dyn DynGenerator,
    ir: &Ir,
    request: (Role, Vec<veld_ir::ServiceId>, Vec<veld_ir::ServiceId>),
    (target, edition): (&str, &str),
) -> FileSet {
    let (role, services, consumes) = request;
    let req = GenRequest {
        ir,
        role,
        services,
        consumes,
        framework: None,
    };
    let options = veld_codegen::with_target_name(&json!({ "typescript": edition }), target);
    generator
        .generate_dyn(&req, &options)
        .unwrap_or_else(|d| panic!("generation failed: {d:?}"))
}

/// The four models of the golden file, as generated targets.
fn models(edition: &str) -> Vec<Generated> {
    let server_ir = workspace(
        &[("iam", IAM), ("payments", PAYMENTS), ("bookings", BOOKINGS)],
        &[("bookings", &["iam", "payments"])],
    );
    let server = generate(
        &Node,
        &server_ir,
        (
            Role::Server,
            vec![id_of(&server_ir, "Bookings")],
            vec![id_of(&server_ir, "Iam"), id_of(&server_ir, "Payments")],
        ),
        ("bookings", edition),
    );
    let client_ir = workspace(&[("iam", IAM), ("bookings", BOOKINGS)], &[]);
    let all = |ir: &Ir| ir.services.iter().map(|s| s.id).collect::<Vec<_>>();
    let client = generate(
        &TypeScript,
        &client_ir,
        (Role::Client, all(&client_ir), vec![]),
        ("web", edition),
    );
    let ordering_ir = workspace(&[("a", A), ("a-b", AB)], &[]);
    let ordering = generate(
        &TypeScript,
        &ordering_ir,
        (Role::Client, all(&ordering_ir), vec![]),
        ("card-service", edition),
    );
    let rules_ir = workspace(&[("orders", ORDERS)], &[]);
    let rules_only = generate(
        &Node,
        &rules_ir,
        (Role::Server, vec![id_of(&rules_ir, "Orders")], vec![]),
        ("orders", edition),
    );
    vec![
        Generated {
            model: "server",
            files: server,
        },
        Generated {
            model: "client",
            files: client,
        },
        Generated {
            model: "ordering",
            files: ordering,
        },
        Generated {
            model: "rules_only",
            files: rules_only,
        },
    ]
}

fn cases() -> Value {
    let path = repo_root().join("tests/wire/runtime-config/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

// ---- the names of the type ------------------------------------------------------------------

#[test]
fn the_type_of_every_target_name_of_the_golden_file_is_the_one_the_module_gives() {
    let ir = workspace(&[("orders", ORDERS)], &[]);
    let cases = cases();
    for entry in cases["type_names"].as_array().unwrap() {
        let (target, ty) = (
            entry["target"].as_str().unwrap(),
            entry["type"].as_str().unwrap(),
        );
        let files = generate(
            &Node,
            &ir,
            (Role::Server, vec![id_of(&ir, "Orders")], vec![]),
            (target, "es2020"),
        );
        let runtime = text(&files, "runtime.ts");
        assert!(
            runtime.contains(&format!("export interface {ty} {{")),
            "{target}: {ty}"
        );
        assert!(runtime.contains(&format!("export interface {ty}Env {{")));
        assert!(
            runtime.contains(&format!("): {ty} {{")),
            "checkRuntime returns {ty}"
        );
    }
}

#[test]
fn a_target_with_settings_and_no_name_is_e0802() {
    let ir = workspace(&[("orders", ORDERS)], &[]);
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![id_of(&ir, "Orders")],
        consumes: vec![],
        framework: None,
    };
    let err = Node.generate_dyn(&req, &Value::Null).unwrap_err();
    assert_eq!(err[0].code, "E0802");
    let ir = workspace(&[("iam", IAM)], &[]);
    let req = GenRequest {
        ir: &ir,
        role: Role::Client,
        services: vec![id_of(&ir, "Iam")],
        consumes: vec![],
        framework: None,
    };
    let err = TypeScript.generate_dyn(&req, &Value::Null).unwrap_err();
    assert_eq!(err[0].code, "E0802");
}

#[test]
fn a_server_without_settings_keeps_its_files_and_has_no_runtime_module() {
    let ir = workspace(&[("iam", IAM)], &[]);
    let files = generate(
        &Node,
        &ir,
        (Role::Server, vec![id_of(&ir, "Iam")], vec![]),
        ("iam-api", "es2020"),
    );
    assert!(files.get("runtime.ts").is_none());
    let hooks = text(&files, "server/iam.ts");
    assert!(hooks.contains("hooks: IamHooks = {}"), "{hooks}");
    assert!(!hooks.contains("checkRuntime"));
    // The same bytes with no name at all: nothing to name.
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![id_of(&ir, "Iam")],
        consumes: vec![],
        framework: None,
    };
    let unnamed = Node.generate_dyn(&req, &Value::Null).unwrap();
    assert_eq!(unnamed, files);
    assert!(!text(&files, "runtime/veld.ts").contains("runtime_config"));
    assert!(
        text(&files, "runtime/veld.ts").contains("envValue"),
        "the old client code is still there"
    );
}

#[test]
fn the_runtime_module_and_the_entry_of_a_consuming_server_with_rules() {
    let ir = workspace(
        &[("iam", IAM), ("payments", PAYMENTS), ("bookings", BOOKINGS)],
        &[("bookings", &["iam", "payments"])],
    );
    let files = generate(
        &Node,
        &ir,
        (
            Role::Server,
            vec![id_of(&ir, "Bookings")],
            vec![id_of(&ir, "Iam"), id_of(&ir, "Payments")],
        ),
        ("bookings", "es2020"),
    );
    let entry = text(&files, "server/bookings.ts");
    // The runtime config is the last argument, after the hooks; it is required (the server consumes).
    assert!(
        entry.contains("createBookingsHandler(impl: BookingsService, hooks: BookingsHooks, runtime: BookingsRuntime)"),
        "{entry}"
    );
    // The check runs first; the output mode comes from it, not from the hooks.
    assert!(
        entry.contains("const config = checkRuntime(runtime);"),
        "{entry}"
    );
    assert!(
        entry.contains("rules: { output: config.rules?.output ?? 'strict' }"),
        "{entry}"
    );
    assert!(
        !entry.contains("rules?: { output"),
        "the hooks no longer have the output mode"
    );
    insta::assert_snapshot!("runtime_config_server_bookings", entry);
    insta::assert_snapshot!("runtime_config_server_runtime", text(&files, "runtime.ts"));
    insta::assert_snapshot!(
        "runtime_config_server_file_list",
        files.paths().collect::<Vec<_>>().join("\n")
    );
    let veld = text(&files, "runtime/veld.ts");
    assert!(
        !veld.contains("VELD_"),
        "the client has no environment variable any more"
    );
    assert!(!veld.contains("envValue"));
    assert!(text(&files, "index.ts").contains("export * from './runtime';"));
}

#[test]
fn a_target_whose_only_setting_is_optional_takes_the_runtime_config_as_an_optional_argument() {
    let ir = workspace(&[("orders", ORDERS)], &[]);
    let files = generate(
        &Node,
        &ir,
        (Role::Server, vec![id_of(&ir, "Orders")], vec![]),
        ("orders", "es2020"),
    );
    let entry = text(&files, "server/orders.ts");
    assert!(
        entry.contains("hooks: OrdersHooks = {}, runtime?: OrdersRuntime)"),
        "{entry}"
    );
    let runtime = text(&files, "runtime.ts");
    assert!(
        runtime.contains("rules?: { output?: 'strict' | 'audit' | 'off' };"),
        "{runtime}"
    );
    // No client is consumed, so there is no `createClients`.
    assert!(!runtime.contains("createClients"));
    insta::assert_snapshot!("runtime_config_rules_only_runtime", runtime);
}

#[test]
fn the_runtime_module_of_a_client_target() {
    let ir = workspace(&[("iam", IAM), ("bookings", BOOKINGS)], &[]);
    let all: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    let files = generate(
        &TypeScript,
        &ir,
        (Role::Client, all, vec![]),
        ("web", "es2020"),
    );
    assert!(
        files.get("clients.ts").is_none(),
        "createClients moved into runtime.ts"
    );
    let runtime = text(&files, "runtime.ts");
    assert!(runtime.contains("export function createClients(runtime: WebRuntime"));
    assert!(runtime.contains("options: Omit<veld.ClientOptions, 'baseUrl'> = {}"));
    insta::assert_snapshot!("runtime_config_client_runtime", runtime);
    insta::assert_snapshot!(
        "runtime_config_client_file_list",
        files.paths().collect::<Vec<_>>().join("\n")
    );
    let client = text(&files, "iam/client.ts");
    assert!(
        client.contains("constructor(opts: veld.ClientOptions)"),
        "{client}"
    );
    assert!(!client.contains("envVar") && !client.contains("VELD_"));
    insta::assert_snapshot!("runtime_config_client_iam", client);
}

// ---- the goldens, through the generated code ------------------------------------------------

fn tsc(tsc_path: &Path, work: &Path) {
    run(
        Command::new("node")
            .arg(tsc_path)
            .args(["-p", "tsconfig.json"])
            .current_dir(work),
        "tsc runtime config",
    );
}

fn golden_through_the_generated_code(tsc_path: &Path, edition: &str, target: &str) {
    let guard = scratch("runtime-config-");
    let work = guard.path();
    for g in models(edition) {
        write(
            &work.join(format!("m_{}", g.model)),
            &g.files,
            &WriteOptions::default(),
        )
        .unwrap();
    }
    std::fs::copy(
        repo_root().join("tests/toolchain/ts/runtime_golden.ts"),
        work.join("runtime_golden.ts"),
    )
    .unwrap();
    std::fs::write(
        work.join("tsconfig.json"),
        serde_json::to_string_pretty(&json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "m_server/**/*.ts", "m_client/**/*.ts", "m_ordering/**/*.ts", "m_rules_only/**/*.ts"]
        }))
        .unwrap(),
    )
    .unwrap();
    tsc(tsc_path, work);
    let mut cmd = Command::new("node");
    cmd.arg("out/runtime_golden.js")
        .env(
            "CASES",
            repo_root().join("tests/wire/runtime-config/cases.json"),
        )
        .current_dir(work);
    run(&mut cmd, "node runtime_golden.js");
}

#[test]
fn the_goldens_of_the_runtime_config_hold_through_the_generated_check_and_from_env() {
    let Some(tsc_path) = tsc_path() else {
        return;
    };
    for (edition, target) in TARGETS {
        golden_through_the_generated_code(&tsc_path, edition, target);
    }
}

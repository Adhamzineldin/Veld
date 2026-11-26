//! The typed runtime config of the Rust target (`spec/runtime-config.md`,
//! `spec/targets/rust.md` section 15): what is generated for a target with settings and what is
//! not for a target without, the goldens of `tests/wire/runtime-config/cases.json` and the
//! behaviour of the generated server entry and clients.
//!
//! The four targets of the goldens are generated from one contract of eight packages:
//! `bookings` (rules, consumes `iam` and `payments`), `web` (a client of `bookings` and `iam`),
//! `card-service` (consumes `a-b` and `a`) and `orders` (rules, consumes nothing). The toolchain
//! test needs `cargo`; it is skipped without it unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;

use common::*;
use serde_json::{Value, json};
use veld_codegen::runtime_config::runtime_type_name;
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

const REPORT: &str = "model Report {\n  parts: int[]\n  total: int\n  rule sums: sum(parts, p => p) == total\n}\n";

/// One service package: `source` is its one file, `<name>.veld` (a `-` of the name is `_`).
fn package(name: &str, source: &str) -> PackageInput {
    PackageInput {
        name: name.into(),
        kind: PackageKind::Service,
        files: vec![SourceFile {
            path: format!("{}.veld", name.replace('-', "_")),
            text: source.to_string(),
        }],
        base_url: Some(format!("http://{name}:9")),
    }
}

fn ping(service: &str, path: &str) -> String {
    format!("service {service} at \"{path}\" {{\n  get \"/\" ping() -> int\n}}\n")
}

fn contracts() -> Ir {
    let bookings = format!(
        "model Booking {{\n  guests: int\n  rule party: guests <= 10\n}}\n{}",
        ping("Bookings", "/bookings")
    );
    let orders = format!(
        "{REPORT}service Orders at \"/orders\" {{\n  get \"/report\" report(@query broken: bool) -> Report\n}}\n"
    );
    let out = compile(&CompileInput {
        packages: vec![
            package("iam", "service Users at \"/users\" {\n  get \"/\" who() -> int\n}\n"),
            package("payments", &ping("Payments", "/payments")),
            package("bookings", &bookings),
            package("a", &ping("Alpha", "/a")),
            package("a-b", &ping("Beta", "/b")),
            package("card-service", &ping("Cards", "/cards")),
            package("orders", &orders),
        ],
        consumes: BTreeMap::from([
            ("bookings".to_string(), vec!["iam".to_string(), "payments".to_string()]),
            ("card-service".to_string(), vec!["a-b".to_string(), "a".to_string()]),
        ]),
    });
    assert!(out.ir.is_some(), "the contract failed to compile:\n{}", out.render());
    out.ir.unwrap_or_else(|| unreachable!())
}

fn server(ir: &Ir, service: &str, consumes: &[&str], name: &str) -> FileSet {
    gen_server(ir, service, consumes, name)
}

/// The four targets of the goldens, by crate directory.
fn targets(ir: &Ir) -> Vec<(&'static str, FileSet)> {
    vec![
        ("bookings", server(ir, "Bookings", &["Users", "Payments"], "bookings")),
        (
            "web",
            run(
                ir,
                Role::Client,
                vec![service_id(ir, "Bookings"), service_id(ir, "Users")],
                vec![],
                None,
                json!({ "crate-name": "web" }),
            ),
        ),
        ("card_service", server(ir, "Cards", &["Beta", "Alpha"], "card-service")),
        ("orders", server(ir, "Orders", &[], "orders")),
    ]
}

fn cases() -> Value {
    let path = repo_root().join("tests/wire/runtime-config/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn snapshots_of_the_runtime_modules() {
    let ir = contracts();
    let t = targets(&ir);
    let files = |dir: &str| &t.iter().find(|(d, _)| *d == dir).unwrap().1;
    insta::assert_snapshot!("runtime_server_with_rules_and_services", text(files("bookings"), "src/runtime_config.rs"));
    insta::assert_snapshot!("runtime_client", text(files("web"), "src/runtime_config.rs"));
    insta::assert_snapshot!("runtime_rules_only", text(files("orders"), "src/runtime_config.rs"));
    insta::assert_snapshot!("runtime_services_only", text(files("card_service"), "src/runtime_config.rs"));
    insta::assert_snapshot!("runtime_engine", text(files("web"), "src/veld/config.rs"));
    insta::assert_snapshot!("runtime_client_mod", text(files("web"), "src/veld/mod.rs"));
    insta::assert_snapshot!(
        "runtime_server_entry",
        text(files("bookings"), "src/bookings/bookings_server.rs")
    );
    insta::assert_snapshot!(
        "runtime_server_entry_optional",
        text(files("orders"), "src/orders/orders_server.rs")
    );
    insta::assert_snapshot!("runtime_client_constructor", text(files("web"), "src/bookings/bookings_client.rs"));
}

#[test]
fn the_type_names_are_those_of_the_goldens() {
    let file = cases();
    for entry in file["type_names"].as_array().unwrap() {
        let target = entry["target"].as_str().unwrap();
        assert_eq!(runtime_type_name(target), entry["type"].as_str().unwrap(), "{target}");
    }
    let ir = contracts();
    for (dir, files) in targets(&ir) {
        let module = text(&files, "src/runtime_config.rs");
        let name = match dir {
            "bookings" => "BookingsRuntime",
            "web" => "WebRuntime",
            "card_service" => "CardServiceRuntime",
            _ => "OrdersRuntime",
        };
        assert!(module.contains(&format!("pub struct {name} ")), "{dir}");
    }
}

#[test]
fn the_settings_of_the_four_targets_are_those_of_the_models_of_the_goldens() {
    let file = cases();
    let ir = contracts();
    for (dir, files) in targets(&ir) {
        let model = match dir {
            "bookings" => "server",
            "web" => "client",
            "card_service" => "ordering",
            _ => "rules_only",
        };
        let module = text(&files, "src/runtime_config.rs");
        let settings = file["models"][model]["settings"].as_array().unwrap();
        // Every path of the model, in the order of the file, and none other.
        let mut at = 0;
        for s in settings {
            let literal = format!("Setting {{ path: {:?}", s["path"].as_str().unwrap());
            let found = module[at..].find(&literal);
            assert!(found.is_some(), "{dir}: {literal} in order");
            at += found.unwrap() + literal.len();
        }
        assert_eq!(module.matches("Setting { path:").count(), settings.len(), "{dir}");
    }
}

#[test]
fn a_target_without_settings_has_no_runtime_config() {
    // A server without rules that consumes nothing keeps its bytes: no module, no engine.
    let ir = streaming_sse_ir();
    let server = gen_server_services(&ir, &["Live", "Vault"], "s");
    assert!(!server.paths().any(|p| p == "src/runtime_config.rs" || p == "src/veld/config.rs"));
    assert!(!text(&server, "src/lib.rs").contains("runtime_config"));
    assert!(!text(&server, "src/veld/mod.rs").contains("config"));
    let live = text(&server, "src/live/live_server.rs");
    assert!(live.contains("pub fn plain<S: LiveService>(service: S) ->"), "{live}");
    assert!(!live.contains("runtime"), "no runtime argument");
}

#[test]
fn the_hooks_no_longer_have_the_output_mode_and_the_entry_takes_the_config() {
    let ir = contracts();
    let t = targets(&ir);
    let files = |dir: &str| &t.iter().find(|(d, _)| *d == dir).unwrap().1;
    let bookings = text(files("bookings"), "src/bookings/bookings_server.rs");
    // The hooks trait has no `rules_output`; the dispatch holds the mode the config gave.
    assert!(!bookings.contains("self.hooks.rules_output()"), "{bookings}");
    assert!(!bookings.contains("RuleOutput::Strict\n"), "no default in the hooks");
    assert!(bookings.contains("output: veld::rules::RuleOutput,"), "{bookings}");
    assert!(bookings.contains("runtime: &crate::runtime_config::BookingsRuntime"), "required: {bookings}");
    assert!(bookings.contains("runtime.rule_output()"), "{bookings}");
    let orders = text(files("orders"), "src/orders/orders_server.rs");
    assert!(
        orders.contains("runtime: Option<&crate::runtime_config::OrdersRuntime>"),
        "no required path, so optional: {orders}"
    );
    // The server of `card-service` has services only: it checks the config and keeps the hooks.
    let cards = text(files("card_service"), "src/cards/cards_server.rs");
    assert!(cards.contains("runtime: &crate::runtime_config::CardServiceRuntime"), "{cards}");
    assert!(!cards.contains("output: veld::rules::RuleOutput"), "{cards}");
}

#[test]
fn a_client_has_no_environment_variable_and_no_compiled_in_url() {
    let ir = contracts();
    let t = targets(&ir);
    let web = &t.iter().find(|(d, _)| *d == "web").unwrap().1;
    for (path, bytes) in web.iter() {
        let s = String::from_utf8_lossy(bytes);
        assert!(!s.contains("VELD_BOOKINGS_URL") && !s.contains("VELD_IAM_URL"), "{path}");
        // The contract's `base-url` (`http://iam:9`) never reaches generated code.
        assert!(!s.contains("http://iam:9") && !s.contains("http://bookings:9"), "{path}");
    }
    let client = text(web, "src/veld/client.rs");
    assert!(!client.contains("std::env::var"), "the runtime reads no environment variable");
    assert!(client.contains("no base URL: pass `base_url`"), "{client}");
    let service = text(web, "src/bookings/bookings_client.rs");
    assert!(service.contains("veld::client::Core::new(transport, STD,"), "{service}");
    // Generated code outside `runtime_config` never reads the environment either.
    for (path, bytes) in web.iter() {
        if path.starts_with("src/") && path != "src/runtime_config.rs" {
            assert!(!String::from_utf8_lossy(bytes).contains("std::env"), "{path}");
        }
    }
}

#[test]
fn the_output_is_deterministic() {
    let ir = contracts();
    assert_eq!(targets(&ir), targets(&ir));
}

#[test]
fn a_server_that_consumes_two_services_with_the_same_field_name_is_refused() {
    // `a-b` and `a_b` both become the Rust field `a_b`: E0801, never wrong code.
    let out = compile(&CompileInput {
        packages: vec![
            package("a-b", &ping("One", "/one")),
            package("a_b", &ping("Two", "/two")),
            package("host", &ping("Host", "/host")),
        ],
        consumes: BTreeMap::from([("host".to_string(), vec!["a-b".to_string(), "a_b".to_string()])]),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    let ir = out.ir.unwrap_or_else(|| unreachable!());
    let req = veld_codegen::GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![service_id(&ir, "Host")],
        consumes: vec![service_id(&ir, "One"), service_id(&ir, "Two")],
        framework: None,
    };
    let opts = veld_codegen::with_target_name(&json!({ "crate-name": "host" }), "host");
    let err = veld_codegen::DynGenerator::generate_dyn(&veld_emit_rust::Rust, &req, &opts).unwrap_err();
    assert_eq!(err[0].code, "E0801", "{err:?}");
}

#[test]
fn a_target_with_settings_and_no_name_is_e0802() {
    let ir = contracts();
    let req = veld_codegen::GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![service_id(&ir, "Orders")],
        consumes: vec![],
        framework: None,
    };
    let err = veld_codegen::DynGenerator::generate_dyn(&veld_emit_rust::Rust, &req, &json!({})).unwrap_err();
    assert_eq!(err[0].code, "E0802", "{err:?}");
}

/// The goldens of `cases.json` through the generated check, the flat-map reader and the environment
/// reader of four generated targets, and the behaviour of the generated entries and clients
/// (`tests/toolchain/rust/runtime-harness`).
#[test]
fn the_goldens_and_the_behaviour_through_generated_code() {
    if !cargo_or_skip() {
        return;
    }
    let ir = contracts();
    let ws = work_dir().join("runtime-ws");
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let mut members = vec!["\"runtime-harness\"".to_string()];
    for (dir, files) in targets(&ir) {
        write_files(&ws.join(dir), &files);
        members.push(format!("\"{dir}\""));
    }
    copy_dir(&repo_root().join("tests/toolchain/rust/runtime-harness"), &ws.join("runtime-harness"));
    std::fs::write(
        ws.join("Cargo.toml"),
        format!("[workspace]\nresolver = \"2\"\nmembers = [{}]\n", members.join(", ")),
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    run_ok(&mut cargo(&ws, &["build", "--workspace", "--bins"]), "cargo build");
    let clippy = ["clippy", "--all-targets", "-p", "bookings", "-p", "web", "-p", "orders", "-p", "card-service", "--", "-D", "warnings"];
    run_ok(&mut cargo(&ws, &clippy), "cargo clippy");
    let file = cases();
    let path = repo_root().join("tests/wire/runtime-config/cases.json");
    let golden = run_ok(
        &mut cargo(&ws, &["run", "-q", "-p", "runtime-harness", "--bin", "golden", "--", path.to_str().unwrap()]),
        "the goldens",
    );
    // Every case and every URL of the file ran: none was skipped.
    let want = format!(
        "runtime goldens ok: {} cases, {} urls",
        file["cases"].as_array().unwrap().len(),
        file["urls"].as_array().unwrap().len()
    );
    assert!(golden.contains(&want), "{golden}");
    let behave = run_ok(
        &mut cargo(&ws, &["run", "-q", "-p", "runtime-harness", "--bin", "behave"]),
        "the behaviour",
    );
    assert!(behave.contains("runtime behaviour ok"), "{behave}");
}

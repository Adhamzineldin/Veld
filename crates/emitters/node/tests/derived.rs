//! Derived fields in the generated TypeScript (`spec/targets/typescript.md` section 5i,
//! `spec/rules.md` section 17.2).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;

use veld_ir::Ir;

const ENDPOINTS: &str = "tests/rules/endpoints/derived.veld.pending";
const SHOP: &str = "tests/toolchain/contract/derived/shop.veld";

fn shop_ir() -> Ir {
    compile_path(ENDPOINTS)
}

fn examples_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = shop_ir();
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Shop", &["derived_serve.ts"], edition);
    let server = Live::start(work, "derived_serve.js");
    let passed = run_examples(&server.url, &ir);
    assert_eq!(passed, 9);
    assert_eq!(ir.examples.len(), 9, "every example ran, none was skipped");
}

#[test]
fn the_derived_examples_compile_strict_and_pass_es2020() {
    examples_run("es2020", "derived-2020-");
}

#[test]
fn the_derived_examples_compile_strict_and_pass_es2022() {
    examples_run("es2022", "derived-2022-");
}

/// The production handler over HTTP and the generated client, against the shop contract: a request
/// whose derived values are replaced and drop what was sent, a fact and a principal, a result, a
/// declared error and a stream item that are given theirs, output mode `off`, the client's local
/// check and the public compute function.
fn http_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(SHOP);
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Shop", &["derived_http.ts"], edition);
    run(
        std::process::Command::new("node")
            .arg("out/derived_http.js")
            .current_dir(work),
        "node derived_http.js",
    );
}

#[test]
fn derived_fields_behave_over_http_es2020() {
    http_run("es2020", "derived-http-2020-");
}

#[test]
fn derived_fields_behave_over_http_es2022() {
    http_run("es2022", "derived-http-2022-");
}

const EVENTS: &str = "tests/toolchain/contract/derived/events.veld";

/// Two generated services in one process: the publisher computes the derived fields of an event,
/// a derived field that fails stops the publish, and the subscriber computes them again.
fn events_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(EVENTS);
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Biller", "Ledger"],
        &["derived_events.ts"],
        edition,
        &serde_json::json!({}),
    );
    run(
        std::process::Command::new("node")
            .arg("out/derived_events.js")
            .current_dir(work),
        "node derived_events.js",
    );
}

#[test]
fn derived_fields_of_an_event_are_computed_by_the_publisher_and_again_by_the_subscriber_es2020() {
    events_run("es2020", "derived-events-2020-");
}

#[test]
fn derived_fields_of_an_event_are_computed_by_the_publisher_and_again_by_the_subscriber_es2022() {
    events_run("es2022", "derived-events-2022-");
}

// --- what is generated -------------------------------------------------------------------------

#[test]
fn snapshots_of_a_contract_with_a_chain_of_derived_fields() {
    let ir = compile_path(SHOP);
    let server = gen_server_with(&ir, "Shop", &[], &json!({ "testing": true }));
    insta::assert_snapshot!(
        "derived_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("derived_server_models", text(&server, "models/bookings.ts"));
    insta::assert_snapshot!("derived_server_errors", text(&server, "errors/bookings.ts"));
    insta::assert_snapshot!("derived_server_module", text(&server, "server/shop.ts"));
    insta::assert_snapshot!(
        "derived_test_handler",
        text(&server, "server/shop-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!("derived_client", text(&client, "bookings/client.ts"));
    insta::assert_snapshot!(
        "derived_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
}

#[test]
fn the_chain_is_computed_in_dependency_order_and_the_schema_carries_the_function() {
    let ir = compile_path(SHOP);
    let server = gen_server(&ir, "Shop", &[]);
    let models = text(&server, "models/bookings.ts");
    let at = |needle: &str| {
        models
            .find(needle)
            .unwrap_or_else(|| panic!("no `{needle}`"))
    };
    // `subtotal` is computed before `payable`, which reads it.
    assert!(at("self.subtotal =") < at("self.payable ="));
    assert!(models.contains("derive: computeCartDerived"));
    assert!(models.contains("{ n: 'subtotal', t: { k: 'int' }, o: true, dv: true }"));
    // The three positions of a value.
    assert!(models.contains("export type CartOutput = Omit<Cart,"));
    assert!(models.contains("export type CartInput = Omit<Cart,"));
    assert!(models.contains("export type LineInput = Omit<Line, 'total'>;"));
    let module = text(&server, "server/shop.ts");
    assert!(module.contains("Promise<CartOutput>"), "{module}");
    assert!(module.contains("Promise<ReceiptOutput>"), "{module}");
    assert!(module.contains("wallet(key: string, ctx: veld.RequestContext): Promise<WalletOutput"));
    assert!(module.contains("Promise<veld.UserOutput") || module.contains("UserOutput"));
    let client = gen_client(&ir);
    assert!(text(&client, "bookings/client.ts").contains("body: CartInput;"));
    // The compute function of an error is next to its `check` (`static compute`).
    assert!(text(&server, "errors/bookings.ts").contains("static compute(data:"));
}

#[test]
fn a_contract_without_derived_fields_has_none_of_the_code() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"]);
    let client = gen_client(&ir);
    for (name, files) in [("server", &server), ("client", &client)] {
        for path in files.paths() {
            let body = String::from_utf8_lossy(files.get(path).unwrap_or_default()).into_owned();
            for word in [
                "deriveNode",
                "deriveValue",
                "toWireSend",
                "DeriveFn",
                "computeDerived",
                "Output = Omit<",
                "Input = Omit<",
            ] {
                assert!(!body.contains(word), "{name}: `{word}` is in {path}");
            }
        }
    }
}

#[test]
fn a_declaration_called_like_a_generated_type_is_refused_with_e0801() {
    use veld_codegen::{DynGenerator, GenRequest, Role};
    let source = std::fs::read_to_string(repo_root().join(SHOP)).unwrap();
    let clashing = format!("{source}\nmodel LineOutput {{\n  note: string\n}}\n");
    let ir = compile_text(clashing);
    for (generator, role) in [
        (&veld_emit_node::Node as &dyn DynGenerator, Role::Server),
        (
            &veld_emit_typescript::TypeScript as &dyn DynGenerator,
            Role::Client,
        ),
    ] {
        let req = GenRequest {
            ir: &ir,
            role,
            services: ir.services.iter().map(|s| s.id).collect(),
            consumes: vec![],
            framework: None,
        };
        let err = generator
            .generate_dyn(&req, &common::named(role, &serde_json::Value::Null))
            .unwrap_err();
        assert!(err.iter().all(|d| d.code == "E0801"), "{err:?}");
        assert!(err[0].message.contains("LineOutput"), "{err:?}");
        assert!(err[0].origin.is_some(), "{err:?}");
    }
}

/// A derived field next to a state machine, a version guard with an `@idempotent` endpoint, and
/// an audited rule: the runtime of each combination compiles under `tsc --strict` for both
/// editions (the regions of derived fields pair up with every other region). Nothing is run.
#[test]
fn derived_fields_compile_next_to_transitions_versions_and_audited_rules() {
    let Some(tsc) = tsc_path() else { return };
    let cases = [
        (
            "tests/rules/endpoints/transitions.veld",
            "model Payment {\n  amount: decimal",
            "model Payment {\n  amount: decimal\n  copy: decimal = amount",
            "Bookings",
        ),
        (
            "tests/rules/endpoints/version.veld.pending",
            "model Rename { name: string }",
            "model Rename {\n  name: string\n  shout: string = name\n}",
            "Rooms",
        ),
        (
            "tests/rules/endpoints/rollout.veld",
            "model Party {\n  roomId: uuid\n  guests: int @min(1)",
            "model Party {\n  roomId: uuid\n  guests: int @min(1)\n  twice: int = guests * 2",
            "Parties",
        ),
    ];
    for (path, from, to, service) in cases {
        let text = std::fs::read_to_string(repo_root().join(path)).unwrap();
        assert!(text.contains(from), "{path}");
        let ir = compile_text(text.replacen(from, to, 1));
        for (edition, _) in TARGETS {
            let guard = scratch("derived-combo-");
            build_service(guard.path(), &tsc, &ir, service, &[], edition);
        }
    }
}

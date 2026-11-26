//! Retention in the generated TypeScript server (`spec/targets/typescript.md` section 5l,
//! `spec/platform.md` section 23).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::{Value, json};
use veld_codegen::{DynGenerator, GenRequest, Role};
use veld_emit_node::Node;

const CONTRACT: &str = "tests/toolchain/contract/platform_p4/retention.veld";

/// The runner with recording jobs (the `before` of each entry for a fixed `now`, the order, the
/// fields of an `anonymize`, the system clock by default, an entry that throws and one that
/// rejects), and the plan the file holds, compared with `Ir::retention_plan` here.
fn run_driver(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Guests"],
        &["retention_runner.ts"],
        edition,
        &json!({}),
    );
    run(
        std::process::Command::new("node")
            .arg("out/retention_runner.js")
            .current_dir(work),
        "node retention_runner.js",
    );
    let written: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(work.join("plan.json")).unwrap()).unwrap();
    let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    let plan = ir.retention_plan(&services);
    assert_eq!(written.len(), plan.len(), "{written:?}");
    for (entry, expected) in written.iter().zip(&plan) {
        assert_eq!(entry["model"], expected.model);
        assert_eq!(entry["afterMs"], expected.after_ms);
        assert_eq!(entry["since"], expected.since);
        assert_eq!(entry["then"], expected.then.word());
        assert_eq!(entry["fields"], json!(expected.fields));
    }
}

#[test]
fn the_runner_asks_for_what_is_due_and_the_plan_is_the_one_of_the_ir_es2020() {
    run_driver("es2020", "retention-2020-");
}

#[test]
fn the_runner_asks_for_what_is_due_and_the_plan_is_the_one_of_the_ir_es2022() {
    run_driver("es2022", "retention-2022-");
}

#[test]
fn snapshots_of_a_server_with_retention() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Guests", &[], &json!({}));
    insta::assert_snapshot!("retention_server", text(&server, "retention.ts"));
    insta::assert_snapshot!("retention_barrel", text(&server, "index.ts"));
}

#[test]
fn the_names_come_from_the_target_and_the_plan_from_the_ir() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Guests", &[], &json!({}));
    let file = text(&server, "retention.ts");
    for line in [
        "export const retentionPlan: readonly RetentionEntry[] =",
        "export interface RetentionJobs",
        "export async function runApiRetention(",
        "afterMs: 2592000000,",
        "afterMs: 31536000000,",
        "afterMs: 7776000000,",
        "fields: ['name', 'phone'],",
    ] {
        assert!(file.contains(line), "`{line}` is missing from retention.ts");
    }
    // A model without `@retain` is not in the plan, and a client has none of it.
    assert!(!file.contains("Kept"));
    let client = gen_client(&ir);
    for path in client.paths() {
        let body = String::from_utf8_lossy(client.get(path).unwrap_or_default()).into_owned();
        assert!(
            !body.contains("retentionPlan") && !body.contains("RetentionJobs"),
            "{path}"
        );
    }
}

/// A contract without `@retain` has no file for it, in the server and in the barrel.
#[test]
fn a_server_without_retained_models_has_none_of_it() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"]);
    assert!(server.get("retention.ts").is_none());
    for path in server.paths() {
        let body = String::from_utf8_lossy(server.get(path).unwrap_or_default()).into_owned();
        assert!(!body.contains("retentionPlan"), "{path}");
    }
}

/// A retention that a `number` cannot hold exactly is `E0801`, never a wrong date.
#[test]
fn a_retention_beyond_a_number_is_refused() {
    let text = "@retain(days(106000000), since: closedAt, then: delete)
model Old {
  id: uuid
  closedAt: datetime
}

service Olds at \"/olds\" {
  get \"/{id}\" getOld(id: uuid) -> Old
}
";
    let ir = compile_text(text.to_owned());
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: None,
    };
    let diags = Node
        .generate_dyn(&req, &named(Role::Server, &json!({})))
        .expect_err("a retention that is not a number of milliseconds is refused");
    assert!(diags.iter().all(|d| d.code == "E0801"), "{diags:?}");
}

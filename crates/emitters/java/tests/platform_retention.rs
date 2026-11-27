//! Retention in the generated Java server (`spec/targets/java.md` section 18.4,
//! `spec/platform.md` section 23).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::require_javac;
use common::p4::{compiles, generate, run_driver};
use common::*;
use serde_json::{Value, json};
use veld_codegen::{DynGenerator, GenRequest, Role};
use veld_emit_java::Java;

const CONTRACT: &str = "tests/toolchain/contract/platform_p4/retention.veld";

/// The runner with recording jobs (the `before` of each entry for a fixed `now`, the order, the
/// fields of an `anonymize`, the system clock by default, an entry that throws), and the plan the
/// class holds, compared with `Ir::retention_plan` here.
#[test]
fn the_runner_asks_for_what_is_due_and_the_plan_is_the_one_of_the_ir() {
    if !require_javac("the_runner_asks_for_what_is_due_and_the_plan_is_the_one_of_the_ir") {
        return;
    }
    let ir = compile_path(CONTRACT, "bookings");
    let server = generate(&ir, Role::Server, "gen.b", &json!({ "target-name": "api" }));
    let work = run_driver(&[&server], "RetentionRunner", &[], &[]);
    let written: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(work.path().join("plan.json")).unwrap())
            .unwrap();
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
fn snapshots_of_a_server_with_retention() {
    let ir = compile_path(CONTRACT, "bookings");
    let server = generate(&ir, Role::Server, "gen.b", &json!({ "target-name": "api" }));
    for name in ["Retention", "RetentionEntry", "RetentionJobs"] {
        insta::assert_snapshot!(
            format!("retention_{name}"),
            text(&server, &format!("gen/b/retention/{name}.java"))
        );
    }
}

/// The names come from the target and the plan from the IR; a client has none of it.
#[test]
fn the_names_come_from_the_target_and_the_plan_from_the_ir() {
    let ir = compile_path(CONTRACT, "bookings");
    let server = generate(&ir, Role::Server, "gen.b", &json!({ "target-name": "api" }));
    let file = text(&server, "gen/b/retention/Retention.java");
    for line in [
        "public static java.util.List<RetentionEntry> retentionPlan()",
        "public static java.util.List<RetentionResult> runApiRetention(RetentionJobs jobs)",
        "public static java.util.List<RetentionResult> runApiRetention(RetentionJobs jobs, RetentionOptions options)",
        "2592000000L, \"checkedOutAt\", RetentionEntry.Then.ANONYMIZE, java.util.List.of(\"name\", \"phone\")",
        "31536000000L",
        "7776000000L",
    ] {
        assert!(
            file.contains(line),
            "`{line}` is missing from Retention.java"
        );
    }
    // A model without `@retain` is not in the plan.
    assert!(!file.contains("Kept"));
    // Another target name, another runner.
    let other = generate(
        &ir,
        Role::Server,
        "gen.b",
        &json!({ "target-name": "billing-api" }),
    );
    assert!(text(&other, "gen/b/retention/Retention.java").contains("runBillingApiRetention("));
    // A client has none of it.
    let client = generate(&ir, Role::Client, "gen.c", &json!({}));
    for (path, bytes) in client.iter() {
        let body = String::from_utf8_lossy(bytes);
        assert!(
            !body.contains("retentionPlan") && !body.contains("RetentionJobs"),
            "{path}"
        );
    }
}

/// A contract without `@retain` has no file for it: the files are what they were.
#[test]
fn a_server_without_retained_models_has_none_of_it() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"], "gen.a", None);
    for (path, bytes) in server.iter() {
        let body = String::from_utf8_lossy(bytes);
        assert!(!path.contains("/retention/"), "{path}");
        assert!(!body.contains("retentionPlan"), "{path}");
    }
}

/// A server whose target has no name cannot name `run<Target>Retention`: `E0802`, never a guess.
#[test]
fn a_target_without_a_name_is_refused() {
    let ir = compile_path(CONTRACT, "bookings");
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: None,
    };
    let diags = Java
        .generate_dyn(&req, &json!({ "package": "gen.b" }))
        .expect_err("a target without a name is refused");
    assert!(diags.iter().any(|d| d.code == "E0802"), "{diags:?}");
}

/// Retention next to catalogs, captures and the platform features: the files compile with
/// `-Xlint:all -Werror`. Nothing is run.
#[test]
fn retention_compiles_next_to_the_other_features() {
    if !require_javac("retention_compiles_next_to_the_other_features") {
        return;
    }
    let ir = compile_path(CONTRACT, "bookings");
    for extra in [
        json!({ "target-name": "api" }),
        json!({ "target-name": "api", "capture-violations": true, "testing": true }),
    ] {
        let server = generate(&ir, Role::Server, "gen.b", &extra);
        compiles(&[&server]);
    }
}

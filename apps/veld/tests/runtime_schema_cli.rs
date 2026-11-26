//! `veld export runtime-schema` through the real CLI (`spec/runtime-config.md` section 6).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::Path;

use common::*;
use predicates::prelude::*;
use serde_json::{Value, json};
use tempfile::tempdir;

const ROOT: &str = "[workspace]\nname = \"shop\"\nmembers = [\"services/*\", \"apps/*\"]\n";
const ORDERS_TOML: &str = "[service]\nconsumes = [\"payments\"]\n\n[server]\nemitter = \"node\"\n";
const PAYMENTS_TOML: &str = "[service]\n\n[server]\nemitter = \"node\"\n";
const WEB_TOML: &str = "[app]\nuses = [\"orders\"]\n\n[client]\nemitter = \"typescript\"\n";
const ORDERS: &str = "model Order {\n  id: uuid\n}\n\n@auth(none)\nservice Orders at \"/orders\" {\n  get \"/{id}\" getOrder(id: uuid) -> Order | NotFound\n}\n";
const PAYMENTS: &str = "model Payment {\n  id: uuid\n  amount: int\n}\n\n@auth(none)\nservice Payments at \"/payments\" {\n  get \"/{id}\" getPayment(id: uuid) -> Payment | NotFound\n}\n";
/// `Payment` now has a rule, so the `payments` server has the output mode.
const PAYMENTS_WITH_RULES: &str = "model Payment {\n  id: uuid\n  amount: int\n  rule positive: amount > 0 message \"positive\"\n}\n\n@auth(none)\nservice Payments at \"/payments\" {\n  get \"/{id}\" getPayment(id: uuid) -> Payment | NotFound\n}\n";

/// `orders` (a server that consumes `payments`), `payments` (a server with nothing to configure)
/// and `web` (a client of `orders`).
fn shop(dir: &Path) {
    write(dir, "veld.toml", ROOT);
    write(dir, "services/orders/veld.toml", ORDERS_TOML);
    write(dir, "services/payments/veld.toml", PAYMENTS_TOML);
    write(dir, "apps/web/veld.toml", WEB_TOML);
    write(dir, "services/orders/contracts/orders.veld", ORDERS);
    write(dir, "services/payments/contracts/payments.veld", PAYMENTS);
}

fn read(dir: &Path, rel: &str) -> String {
    std::fs::read_to_string(dir.join(rel)).unwrap()
}

fn url_object() -> Value {
    json!({
        "additionalProperties": false,
        "properties": { "url": {
            "format": "uri", "minLength": 1, "pattern": "^[Hh][Tt][Tt][Pp][Ss]?://", "type": "string"
        } },
        "required": ["url"],
        "type": "object"
    })
}

fn services_object(name: &str) -> Value {
    json!({
        "additionalProperties": false,
        "properties": { name: url_object() },
        "required": [name],
        "type": "object"
    })
}

fn stdout_json(dir: &Path, args: &[&str]) -> (String, Value) {
    let out = veld(dir).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    (text, value)
}

#[test]
fn a_server_that_consumes_a_service_has_its_url_in_the_schema() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    let (text, schema) = stdout_json(tmp.path(), &["export", "runtime-schema", "orders"]);
    let expected = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "additionalProperties": false,
        "properties": { "services": services_object("payments") },
        "required": ["services"],
        "title": "OrdersRuntime",
        "type": "object"
    });
    assert_eq!(schema, expected);
    assert!(text.ends_with("}\n"), "one trailing newline");
    // The keys are in sorted order in the text itself.
    assert!(text.starts_with("{\n  \"$schema\""), "{text}");
    assert_eq!(
        text,
        serde_json::to_string_pretty(&expected).unwrap() + "\n"
    );
}

#[test]
fn a_client_target_has_a_url_for_each_service_it_covers() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    let (_, schema) = stdout_json(tmp.path(), &["export", "runtime-schema", "web"]);
    assert_eq!(schema["title"], "WebRuntime");
    assert_eq!(schema["properties"]["services"], services_object("orders"));
}

#[test]
fn the_output_is_deterministic() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    let first = veld(tmp.path())
        .args(["export", "runtime-schema", "orders"])
        .output()
        .unwrap();
    let second = veld(tmp.path())
        .args(["export", "runtime-schema", "orders"])
        .output()
        .unwrap();
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn a_server_with_rules_has_the_output_mode_in_the_schema() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    write(
        tmp.path(),
        "services/payments/contracts/payments.veld",
        PAYMENTS_WITH_RULES,
    );
    let (_, schema) = stdout_json(tmp.path(), &["export", "runtime-schema", "payments"]);
    assert_eq!(schema["title"], "PaymentsRuntime");
    assert_eq!(
        schema["properties"]["rules"]["properties"]["output"],
        json!({ "default": "strict", "enum": ["strict", "audit", "off"], "type": "string" })
    );
    assert_eq!(schema["required"], json!([]));
}

#[test]
fn a_target_without_a_runtime_config_is_e0803() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(tmp.path())
        .args(["export", "runtime-schema", "payments"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0803"))
        .stderr(predicate::str::contains("`payments` has no runtime config"));
}

#[test]
fn without_a_target_the_workspace_must_have_exactly_one_and_the_message_lists_them() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(tmp.path())
        .args(["export", "runtime-schema"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0803"))
        .stderr(predicate::str::contains("2 targets have a runtime config"))
        .stderr(predicate::str::contains("orders"))
        .stderr(predicate::str::contains("web"));
    // One selected member with one target has exactly one.
    let (_, schema) = stdout_json(tmp.path(), &["export", "runtime-schema", "-p", "web"]);
    assert_eq!(schema["title"], "WebRuntime");
    // So does a run inside a member, which selects that member.
    let (_, schema) = stdout_json(
        &tmp.path().join("services/orders"),
        &["export", "runtime-schema"],
    );
    assert_eq!(schema["title"], "OrdersRuntime");
    // A selection with no runtime config at all.
    veld(tmp.path())
        .args(["export", "runtime-schema", "-p", "payments"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0803"))
        .stderr(predicate::str::contains(
            "no target of the selected members",
        ));
}

#[test]
fn the_selection_flags_work_as_they_do_for_generate() {
    let tmp = tempdir().unwrap();
    shop(tmp.path());
    veld(tmp.path())
        .args(["export", "runtime-schema", "orders", "-p", "web"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0803"))
        .stderr(predicate::str::contains("not selected"));
    veld(tmp.path())
        .args(["export", "runtime-schema", "-p", "nope"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0803"))
        .stderr(predicate::str::contains("web, orders, payments"));
    veld(tmp.path())
        .args(["export", "runtime-schema", "nope"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("no target named `nope`"));
    // From inside a member, `--workspace` selects every member again.
    veld(&tmp.path().join("services/orders"))
        .args(["export", "runtime-schema", "web", "--workspace"])
        .assert()
        .success();
}

#[test]
fn a_file_is_written_like_the_openapi_export_writes_its_file() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    shop(dir);
    let (text, _) = stdout_json(dir, &["export", "runtime-schema", "orders"]);
    veld(dir)
        .args([
            "export",
            "runtime-schema",
            "orders",
            "-o",
            "orders.schema.json",
        ])
        .assert()
        .success();
    assert_eq!(read(dir, "orders.schema.json"), text);
    // The same bytes again change nothing.
    veld(dir)
        .args([
            "export",
            "runtime-schema",
            "orders",
            "-o",
            "orders.schema.json",
        ])
        .assert()
        .success();
    // The file is the user's: no manifest lists it, and a different one is refused without --force.
    assert!(!dir.join(".veld-manifest.json").exists());
    write(dir, "other.json", "mine");
    veld(dir)
        .args(["export", "runtime-schema", "orders", "-o", "other.json"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("--force"));
    assert_eq!(read(dir, "other.json"), "mine");
    veld(dir)
        .args([
            "export",
            "runtime-schema",
            "orders",
            "-o",
            "other.json",
            "--force",
        ])
        .assert()
        .success();
    assert_eq!(read(dir, "other.json"), text);
}

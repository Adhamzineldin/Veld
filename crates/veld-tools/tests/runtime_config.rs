//! The runtime config model of a target as a generator receives it, and its JSON Schema
//! (`spec/runtime-config.md` sections 2, 3 and 6).
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use serde_json::json;
use veld_codegen::Role;
use veld_codegen::runtime_config::{
    RuntimeModel, Setting, SettingKind, contract_has_rules, settings_for,
};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind, ServiceId};
use veld_tools::runtime_schema;

const PLAIN: &str = "model Item { name: string }\nservice Plain at \"/p\" {\n post \"/\" create(body: Item) -> 201 Item\n}\n";
const WITH_RULES: &str = "model Booking {\n start: datetime\n end: datetime\n rule endAfterStart: end > start message \"later\"\n}\nservice Bookings at \"/b\" {\n post \"/\" create(body: Booking) -> 201 Booking\n}\n";
/// The rule is on a declaration no endpoint reaches.
const UNREACHED_RULE: &str = "model Item { name: string }\nmodel Unused {\n a: int\n rule positive: a > 0\n}\nservice Plain at \"/p\" {\n post \"/\" create(body: Item) -> 201 Item\n}\n";
const TWO_SERVICES: &str = "model Item { name: string }\nservice First at \"/f\" {\n post \"/\" create(body: Item) -> 201 Item\n}\nservice Second at \"/s\" {\n post \"/\" make(body: Item) -> 201 Item\n}\n";

/// Compiles one service package per `(name, source)`.
fn ir_of(packages: &[(&str, &str)]) -> Ir {
    let out = compile(&CompileInput {
        packages: packages
            .iter()
            .map(|(name, text)| PackageInput {
                name: (*name).to_string(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: "app.veld".into(),
                    text: (*text).to_string(),
                }],
                base_url: None,
            })
            .collect(),
        consumes: BTreeMap::new(),
    });
    let rendered = out.render();
    out.ir
        .unwrap_or_else(|| panic!("does not compile:\n{rendered}"))
}

fn ids(ir: &Ir, package: &str) -> Vec<ServiceId> {
    ir.services_of_package(package).map(|s| s.id).collect()
}

fn paths(settings: &[Setting]) -> Vec<&str> {
    settings.iter().map(|s| s.path.as_str()).collect()
}

fn world() -> Ir {
    ir_of(&[
        ("bookings", WITH_RULES),
        ("iam", PLAIN.replace("Plain", "Iam").as_str()),
        ("payments", PLAIN.replace("Plain", "Payments").as_str()),
    ])
}

#[test]
fn a_server_without_rules_that_consumes_nothing_has_no_runtime_config() {
    let ir = world();
    let iam = ids(&ir, "iam");
    assert!(RuntimeModel::new(&ir, "iam", Role::Server, &iam, &[]).is_none());
    assert!(settings_for(&ir, Role::Server, &iam, &[]).is_empty());
}

#[test]
fn a_server_that_consumes_has_a_url_for_each_consumed_service() {
    let ir = world();
    let mut consumes = ids(&ir, "payments");
    consumes.extend(ids(&ir, "iam"));
    let m = RuntimeModel::new(
        &ir,
        "payments-api",
        Role::Server,
        &ids(&ir, "iam"),
        &consumes,
    )
    .unwrap();
    assert_eq!(
        paths(m.settings()),
        ["services.iam.url", "services.payments.url"]
    );
    assert_eq!(m.type_name(), "PaymentsApiRuntime");
    assert!(
        m.settings()
            .iter()
            .all(|s| s.required && s.kind == SettingKind::Url)
    );
    let first = m.setting("services.iam.url").unwrap();
    assert_eq!(first.service.as_ref().unwrap().name, "iam");
    assert_eq!(first.service.as_ref().unwrap().ids, ids(&ir, "iam"));
}

#[test]
fn a_server_with_rules_has_the_output_mode_first_in_path_order() {
    let ir = world();
    let bookings = ids(&ir, "bookings");
    let mut consumes = ids(&ir, "payments");
    consumes.extend(ids(&ir, "iam"));
    let m = RuntimeModel::new(&ir, "bookings", Role::Server, &bookings, &consumes).unwrap();
    assert_eq!(
        paths(m.settings()),
        ["rules.output", "services.iam.url", "services.payments.url"]
    );
    let output = m.setting("rules.output").unwrap();
    assert!(!output.required);
    assert_eq!(output.default.as_deref(), Some("strict"));
}

#[test]
fn a_server_with_rules_and_no_clients_has_only_the_output_mode() {
    let ir = world();
    let m = RuntimeModel::new(&ir, "bookings", Role::Server, &ids(&ir, "bookings"), &[]).unwrap();
    assert_eq!(paths(m.settings()), ["rules.output"]);
    assert!(!m.has_required());
}

#[test]
fn rules_in_a_consumed_service_do_not_give_the_server_an_output_mode() {
    let ir = world();
    let m = RuntimeModel::new(
        &ir,
        "iam",
        Role::Server,
        &ids(&ir, "iam"),
        &ids(&ir, "bookings"),
    )
    .unwrap();
    assert_eq!(paths(m.settings()), ["services.bookings.url"]);
    assert!(!contract_has_rules(&ir, &ids(&ir, "iam")));
    assert!(contract_has_rules(&ir, &ids(&ir, "bookings")));
}

#[test]
fn a_rule_on_a_declaration_no_endpoint_reaches_still_counts() {
    let ir = ir_of(&[("plain", UNREACHED_RULE)]);
    assert!(contract_has_rules(&ir, &ids(&ir, "plain")));
    let m = RuntimeModel::new(&ir, "plain", Role::Server, &ids(&ir, "plain"), &[]).unwrap();
    assert_eq!(paths(m.settings()), ["rules.output"]);
}

#[test]
fn a_client_has_a_url_for_each_service_it_covers_and_never_an_output_mode() {
    let ir = world();
    let mut all = ids(&ir, "bookings");
    all.extend(ids(&ir, "iam"));
    let m = RuntimeModel::new(&ir, "web", Role::Client, &all, &[]).unwrap();
    assert_eq!(
        paths(m.settings()),
        ["services.bookings.url", "services.iam.url"]
    );
    assert_eq!(m.type_name(), "WebRuntime");
}

#[test]
fn a_client_of_no_service_has_no_runtime_config() {
    let ir = world();
    assert!(RuntimeModel::new(&ir, "web", Role::Client, &[], &[]).is_none());
}

#[test]
fn a_package_with_several_service_blocks_has_one_path() {
    let ir = ir_of(&[("shop", TWO_SERVICES)]);
    let both = ids(&ir, "shop");
    assert_eq!(both.len(), 2);
    let m = RuntimeModel::new(&ir, "web", Role::Client, &both, &[]).unwrap();
    assert_eq!(paths(m.settings()), ["services.shop.url"]);
    assert_eq!(m.settings()[0].service.as_ref().unwrap().ids, both);
}

#[test]
fn the_schema_of_a_server_with_rules_that_consumes_two_services() {
    let ir = world();
    let mut consumes = ids(&ir, "payments");
    consumes.extend(ids(&ir, "iam"));
    let m = RuntimeModel::new(
        &ir,
        "bookings",
        Role::Server,
        &ids(&ir, "bookings"),
        &consumes,
    )
    .unwrap();
    let url = json!({
        "format": "uri", "minLength": 1, "pattern": "^[Hh][Tt][Tt][Pp][Ss]?://", "type": "string"
    });
    let expected = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "additionalProperties": false,
        "properties": {
            "rules": {
                "additionalProperties": false,
                "properties": {
                    "output": { "default": "strict", "enum": ["strict", "audit", "off"], "type": "string" }
                },
                "required": [],
                "type": "object"
            },
            "services": {
                "additionalProperties": false,
                "properties": {
                    "iam": {
                        "additionalProperties": false,
                        "properties": { "url": url },
                        "required": ["url"],
                        "type": "object"
                    },
                    "payments": {
                        "additionalProperties": false,
                        "properties": { "url": url },
                        "required": ["url"],
                        "type": "object"
                    }
                },
                "required": ["iam", "payments"],
                "type": "object"
            }
        },
        "required": ["services"],
        "title": "BookingsRuntime",
        "type": "object"
    });
    let schema = runtime_schema(&m);
    assert_eq!(schema, expected);
    // The order is part of the output: keys are sorted at every level.
    assert_eq!(
        serde_json::to_string(&schema).unwrap(),
        serde_json::to_string(&expected).unwrap()
    );
    assert_eq!(schema, runtime_schema(&m), "deterministic");
}

#[test]
fn the_schema_keys_are_sorted_even_when_the_settings_are_not_added_in_order() {
    let m = RuntimeModel::from_settings(
        "web",
        vec![
            Setting::service_url("zeta", Vec::new()),
            Setting::service_url("a-b", Vec::new()),
            Setting::service_url("a", Vec::new()),
        ],
    );
    let schema = runtime_schema(&m);
    let services = schema["properties"]["services"]["properties"]
        .as_object()
        .unwrap();
    let keys: Vec<&str> = services.keys().map(String::as_str).collect();
    assert_eq!(keys, ["a", "a-b", "zeta"]);
    assert_eq!(
        schema["properties"]["services"]["required"],
        json!(["a", "a-b", "zeta"])
    );
    let top: Vec<&str> = schema
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top,
        [
            "$schema",
            "additionalProperties",
            "properties",
            "required",
            "title",
            "type"
        ]
    );
}

//! Derived fields in generated Rust (`spec/rules.md` section 17.2, `spec/targets/rust.md` section
//! 16): snapshots of a chain, the words a contract without them must not contain, the 22 vectors
//! of `tests/rules/core/derived.json.pending` through a generated crate (21 here: the partial
//! check of a draft is TypeScript and the reference only), the 9 examples of
//! `tests/rules/endpoints/derived.veld.pending` through the generated test handler, and the
//! production handler and the client over HTTP (`tests/toolchain/rust/derived-edges`).
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::PathBuf;

use common::*;
use serde_json::{Value, json};
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_rust::tree::Tree;
use veld_ir::{DeclKind, Ir, PackageKind};
use veld_rules::vectors::{Case, Produced, compare_produced, load_cases, value_at};

const ENDPOINTS: &str = "tests/rules/endpoints/derived.veld.pending";
const EDGES: &str = "tests/toolchain/rust/derived-edges/edges.veld";

/// A chain of derived fields, one a model reads through a list, and an error with one.
const CHAIN: &str = "model Line {
  price: decimal
  qty: int
  total: decimal = price * qty
}

model Order {
  items: Line[]
  discount: decimal = 0
  subtotal: decimal = sum(items, i => i.total)
  total: decimal = subtotal - discount
  count: int = len(items)
  rule notNegative: total >= 0
}

error OverLimit 422 {
  limit: int
  used: int
  left: int = limit - used
}

service Shop at \"/shop\" {
  post \"/\" place(body: Order) -> 201 Order | OverLimit
}
";

fn chain() -> Ir {
    compile_text(CHAIN, "chain")
}

fn server_of(ir: &Ir) -> FileSet {
    run(
        ir,
        Role::Server,
        vec![service_id(ir, "Shop")],
        vec![],
        None,
        json!({ "crate-name": "chain-server" }),
    )
}

#[test]
fn snapshots_of_a_chain_of_derived_fields() {
    let ir = chain();
    let server = server_of(&ir);
    insta::assert_snapshot!(
        "derived_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("derived_models", text(&server, "src/chain/chain.rs"));
    insta::assert_snapshot!(
        "derived_server",
        text(&server, "src/chain/shop_server.rs")
    );
    insta::assert_snapshot!("derived_rules_runtime", text(&server, "src/veld/rules/mod.rs"));
    let client = gen_client(&ir, "chain-client", None);
    insta::assert_snapshot!("derived_client_models", text(&client, "src/chain/chain.rs"));
}

#[test]
fn a_derived_field_gets_a_schema_flag_a_derive_function_and_a_public_function() {
    let ir = chain();
    let models = text(&server_of(&ir), "src/chain/chain.rs").to_string();
    // Only the declarations that have a derived field get `derive`; every field says whether it is one.
    assert!(models.contains("derive: Some(veld::rules::derive_object::<Order>)"), "{models}");
    assert!(models.contains("derive: Some(veld::rules::derive_object::<Line>)"));
    assert!(models.contains("derive: Some(veld::rules::derive_object::<OverLimit>)"));
    assert!(models.contains("derived: true,"));
    assert!(models.contains("pub fn derive(&mut self) -> Vec<veld::rules::Violation>"));
    assert!(models.contains("pub fn derive_request(&mut self)"));
    // The chain is computed in order, each field after the ones it reads.
    let order = &models[models.find("impl veld::rules::Derive for Order").unwrap()..];
    let at = |needle: &str| order.find(needle).unwrap_or_else(|| panic!("{needle}"));
    assert!(at("// subtotal") < at("// total"));
    assert!(at("// total") < at("// count"));
    assert!(order.contains("let ok_total = if ok_subtotal"), "{order}");
    // A rule that reads a derived field is skipped when the field has no value.
    assert!(models.contains("!absent.contains(&\"total\")"));
}

/// A contract without a derived field does not get one word of the feature: no flag in a schema,
/// no function in the runtime.
#[test]
fn a_contract_without_derived_fields_has_none_of_it() {
    let words = [
        "derive_object",
        "derive_walk",
        "fill_derived",
        "without_derived",
        "complete_with",
        "complete_wire",
        "check_computed",
        "derived: ",
        "derive: ",
        "check_rules_absent",
        "stand_in",
    ];
    for files in [server_only_standard(), gen_client(&standard_ir(), "o", None)] {
        for (path, bytes) in files.iter() {
            let s = String::from_utf8_lossy(bytes);
            for word in words {
                assert!(!s.contains(word), "{path} has `{word}`");
            }
        }
    }
}

fn server_only_standard() -> FileSet {
    gen_server(&standard_ir(), "Orders", &["Iam"], "orders-server")
}

/// The crates of the rules vectors and of the endpoint contract do not fail on a model that has
/// a derived field and no rule at all.
#[test]
fn a_derived_field_without_a_rule_still_brings_the_rules_runtime() {
    let ir = compile_text(
        "model Room {\n  seats: int\n  free: int = seats - 1\n}\n\nservice Rooms at \"/rooms\" {\n  get \"/\" room() -> Room\n}\n",
        "rooms",
    );
    let files = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Rooms")],
        vec![],
        None,
        json!({ "crate-name": "rooms-server" }),
    );
    assert!(files.get("src/veld/rules/mod.rs").is_some());
    let models = text(&files, "src/rooms/rooms.rs");
    assert!(models.contains("impl veld::rules::Derive for Room"), "{models}");
    assert!(!models.contains("impl veld::rules::Invariants for Room"));
}

// ---- the vectors -----------------------------------------------------------------------------------

fn core_file(name: &str) -> PathBuf {
    repo_root().join("tests/rules/core").join(name)
}

/// The vector contract with a stub service so that the generator has something to serve.
fn vector_ir() -> Ir {
    let mut text = std::fs::read_to_string(core_file("derived.veld.pending")).unwrap();
    text.push_str("\nservice Probe at \"/probe\" {\n  get \"/p\" probe() -> Line\n}\n");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "derived.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: std::collections::BTreeMap::new(),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    out.ir.unwrap()
}

/// The driver: one `match` arm per model and error. For each case it prints one JSON line with the
/// violations of the derived fields and of the invariants, and the value as completed (wire form).
fn driver(ir: &Ir) -> String {
    let tree = Tree::new(
        ir,
        "crate",
        "src",
        ir.services.iter().map(|s| s.id).collect(),
        true,
    );
    let mut arms = String::new();
    for id in tree.decls() {
        let d = ir.decl(*id);
        if matches!(d.kind, DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Union(_))
            && !d.name.is_empty()
            && tree.module_path(d) != "crate::prelude"
        {
            let path = tree.type_path(*id, "").replacen("crate", "vec_derived", 1);
            arms.push_str(&format!(
                "        {:?} => run::<{path}>(value, request),\n",
                d.name
            ));
        }
    }
    format!(
        r#"use serde_json::{{Value, json}};
use vec_derived::veld::{{self, VeldDecl}};

fn run<T: VeldDecl>(value: &Value, request: bool) -> Value {{
    let schema = T::schema();
    // What a caller sent for a derived field is dropped before the value is checked.
    let wire = schema.without_derived(value);
    let issues = schema.validate(&wire, "$", request);
    if !issues.is_empty() {{
        return json!({{ "invalid": issues.len() }});
    }}
    let mode = if request {{ veld::rules::Mode::Request }} else {{ veld::rules::Mode::Full }};
    let found = veld::rules::check_wire(schema, &wire, "$", mode);
    let mut typed = schema.from_wire(&wire, request);
    veld::rules::derive_walk(schema, &mut typed, "$", mode, &mut Vec::new());
    let completed = schema.to_wire(&typed, false, false);
    json!({{ "violations": found.iter().map(veld::rules::Violation::to_json).collect::<Vec<_>>(), "wire": completed }})
}}

fn main() {{
    let path = std::env::args().nth(1).expect("a case file");
    let text = std::fs::read_to_string(path).expect("readable");
    let cases: Vec<Value> = serde_json::from_str(&text).expect("json");
    for case in cases {{
        let value = &case["value"];
        let request = case["request"].as_bool().unwrap_or(false);
        let out = match case["type"].as_str().unwrap_or("") {{
{arms}            _ => json!({{ "unknown": 0 }}),
        }};
        println!("{{out}}");
    }}
}}
"#
    )
}

fn vector_crate(ir: &Ir) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    let mut files = run(
        ir,
        Role::Client,
        all,
        vec![],
        Some("ureq"),
        json!({ "crate-name": "vec-derived" }),
    );
    files.add("src/bin/vectors_derived.rs", driver(ir)).unwrap();
    files
}

/// The 22 cases of `derived.json.pending`: 21 run through generated Rust (the draft case is the
/// partial check, which only TypeScript and the reference evaluator have), comparing `expect`, the
/// violations and the `computed` values.
#[test]
fn the_derived_vectors_pass_through_generated_rust() {
    if !cargo_or_skip() {
        return;
    }
    let text = std::fs::read_to_string(core_file("derived.json.pending")).unwrap();
    let all = load_cases(&text).unwrap();
    assert_eq!(all.len(), 22, "the file has 22 cases");
    let cases: Vec<&Case> = all.iter().filter(|c| !c.partial).collect();
    assert_eq!(cases.len(), 21, "one case is the partial check, which Rust does not have");
    let ir = vector_ir();
    let ws = work_dir().join("derived-vectors-ws");
    let _ = std::fs::remove_dir_all(&ws);
    write_files(&ws.join("vec"), &vector_crate(&ir));
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"vec\"]\n",
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    run_ok(&mut cargo(&ws, &["build", "--bins"]), "cargo build");
    run_ok(
        &mut cargo(&ws, &["clippy", "--all-targets", "--", "-D", "warnings"]),
        "cargo clippy",
    );
    let input: Vec<Value> = cases
        .iter()
        .map(|c| json!({ "type": c.ty, "value": c.value, "request": c.request }))
        .collect();
    let file = ws.join("cases.json");
    std::fs::write(&file, serde_json::to_string(&input).unwrap()).unwrap();
    let out = run_ok(
        &mut cargo(
            &ws,
            &["run", "-q", "--bin", "vectors_derived", "--", &file.to_string_lossy()],
        ),
        "the vector driver",
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), cases.len(), "one line per case");
    let failures = compare_every(&cases, &lines);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn compare_every(cases: &[&Case], lines: &[&str]) -> Vec<String> {
    let mut failures = Vec::new();
    for (case, line) in cases.iter().zip(lines) {
        let Ok(doc) = serde_json::from_str::<Value>(line) else {
            failures.push(format!("case `{}` printed {line}", case.name));
            continue;
        };
        let produced: Vec<Produced> = doc["violations"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect())
            .unwrap_or_default();
        if let Err(e) = compare_produced(case, &produced) {
            failures.push(format!("{e}"));
        }
        for (path, want) in &case.computed {
            let got = value_at(&doc["wire"], path).unwrap_or(&Value::Null);
            if got != want {
                failures.push(format!("case `{}`: {path} is {got}, expected {want}", case.name));
            }
        }
    }
    failures
}

// ---- the endpoint contract: the examples through the generated test handler -------------------------

fn endpoint_crates(ir: &Ir) -> (FileSet, FileSet) {
    let server = run(
        ir,
        Role::Server,
        vec![service_id(ir, "Shop")],
        vec![],
        None,
        json!({ "crate-name": "derived-server", "testing": true }),
    );
    (server, gen_client(ir, "derived-client", None))
}

/// The server (with the test handler) and the client build and pass `clippy -D warnings`, and all
/// 9 examples of the endpoint contract pass against the test handler: a request whose derived
/// value decides a `require`, a fact, a result read by an `ensure`, an error with a derived field.
#[test]
fn the_endpoint_examples_pass_through_the_generated_test_handler() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(ENDPOINTS, "derived");
    let (server, client) = endpoint_crates(&ir);
    let ws = harness_workspace("derived-ws", &server, &client, "derived-harness");
    build_lint_and_run(
        &ws,
        ("derived-server", "derived-client", "derived-harness"),
        &[],
        "derived harness ok",
    );
    run_every_example(&ir, "derived-serve", 9);
}

// ---- the production handler and the client over HTTP -------------------------------------------------

/// `tests/toolchain/rust/derived-edges`: what a caller sent is replaced and a malformed value is
/// dropped, a failing expression rejects, a rule reads the computed value, a fact and the principal
/// are given theirs, a fact that cannot be computed is `500`, an error and a stream item are
/// computed, the output mode decides what a failing result does, the fingerprint covers the
/// computed values, the public function, the client, and an event published and consumed.
#[test]
fn derived_fields_over_http_in_the_client_and_in_events() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(EDGES, "edges");
    let ids = ir.services.iter().map(|s| s.id).collect();
    let server = run(
        &ir,
        Role::Server,
        ids,
        vec![],
        None,
        json!({ "crate-name": "edges-server" }),
    );
    let client = gen_client(&ir, "edges-client", None);
    // The client ignores events: its crate has no event.
    assert!(client.paths().all(|p| !p.contains("events")));
    let ws = harness_workspace("derived-edges-ws", &server, &client, "derived-edges");
    build_lint_and_run(
        &ws,
        ("edges-server", "edges-client", "derived-edges"),
        &[],
        "derived ok",
    );
}

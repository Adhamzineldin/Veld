//! Derived fields (`spec/rules.md` section 17.2, `spec/targets/java.md` section 15): the 22 shared
//! vectors of `tests/rules/core/derived.json.pending` through the generated schemas, the 9 examples
//! of `tests/rules/endpoints/derived.veld.pending` through the generated test handler on the JDK
//! server and on Spring, what the production handler does over HTTP, the generated client, events,
//! snapshots of a small contract with a chain of derived fields, the files of a contract without
//! derived fields, and `javac -Xlint:all -Werror` for every role and release.
//!
//! The vector files keep their `.pending` suffix until every generator implements the feature.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_java::tree::Tree;
use veld_emit_java::util::type_name;
use veld_ir::{DeclKind, Ir, PackageKind};
use veld_rules::vectors::{Produced, compare_produced, load_cases, run_case, value_at};

const CORE_CONTRACT: &str = "tests/rules/core/derived.veld.pending";
const CORE_CASES: &str = "tests/rules/core/derived.json.pending";
const ENDPOINTS: &str = "tests/rules/endpoints/derived.veld.pending";

/// A principal with a derived field, a result and a stream item with one, and an invariant that
/// reads the derived field of the result.
const EXTRA: &str = r#"
model Caller {
  id: uuid
  level: int
  cap: int = level * 10
}

principal Caller

model Echo {
  n: int
  sq: int = n * n
  rule small: sq < 1000
}

model Tick {
  n: int
  twice: int = n * 2
}

@auth(bearer)
service Probe at "/probe" {
  get "/echo" echo(@query n: int) -> Echo {
    require user.cap >= 10
  }

  get "/ticks" ticks(@query count: int) -> stream Tick
}
"#;

/// An event with a derived field, published by one service and consumed by another.
const SALES: &str = r#"
event Sold {
  sku: string
  qty: int
  price: int
  total: int = qty * price
  rule positive: total > 0
}

@topic("test.sales")
channel Sales {
  carries Sold
  key sku
}

service Shop {
  publishes Sales
}

service Ledger {
  subscribes Sales as "ledger" {
    on Sold -> record
  }
}
"#;

/// A small contract with a chain of derived fields: `total` reads `subtotal`, which reads the
/// derived `total` of every item.
const CHAIN: &str = r#"
model Line {
  price: decimal
  qty: int
  total: decimal = price * qty
}

model Order {
  items: Line[]
  discount: decimal = 0
  subtotal: decimal = sum(items, i => i.total)
  total: decimal = subtotal - discount
  rule notNegative: total >= 0
}

error OverLimit 422 {
  limit: decimal
  used: decimal
  left: decimal = limit - used
}

service Shop at "/shop" {
  post "/" add(body: Order) -> 201 Order | OverLimit
}
"#;

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .replace("\r\n", "\n")
}

fn driver(name: &str) -> PathBuf {
    repo_root().join("tests/toolchain/java/derived").join(name)
}

fn endpoints_ir() -> Ir {
    compile_source(read(ENDPOINTS), "shop", ENDPOINTS)
}

fn extra_ir() -> Ir {
    compile_source(EXTRA.to_string(), "extra", "the principal contract")
}

fn sales_ir() -> Ir {
    compile_source(SALES.to_string(), "sales", "the events contract")
}

fn chain_ir() -> Ir {
    compile_source(CHAIN.to_string(), "chain", "the chain contract")
}

fn generate(ir: &Ir, role: Role, framework: Option<&str>, testing: bool, package: &str) -> FileSet {
    let mut o = options(package, Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], framework, o)
}

fn compile_and_run(files: &FileSet, drivers: &[&str], class: &str, args: &[&str]) -> String {
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), files);
    for d in drivers {
        sources.push(driver(d));
    }
    for release in releases() {
        javac_ok(work.path(), &sources, release);
    }
    let out = java_main(work.path(), class, args);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{class} failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

// --- the shared vectors of the core ---------------------------------------------------------------

/// Every model and union of the vector contract in one package, and a service that makes each of
/// them reachable (a client is gated for what an endpoint reaches).
fn core_ir() -> Ir {
    let text = read(CORE_CONTRACT);
    let mut names = Vec::new();
    let mut probes = String::from("service Probe at \"/probe\" {\n");
    for line in text.lines() {
        let mut words = line.split_whitespace();
        if let (Some("model" | "union"), Some(name)) = (words.next(), words.next()) {
            names.push(name.to_string());
            probes.push_str(&format!(
                "  post \"/p{}\" probe{}(body: {name}) -> 204\n",
                names.len(),
                names.len()
            ));
        }
    }
    probes.push_str("}\n");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![
                SourceFile {
                    path: "derived.veld".into(),
                    text,
                },
                SourceFile {
                    path: "zz_probe.veld".into(),
                    text: format!("use derived.{{{}}}\n\n{probes}", names.join(", ")),
                },
            ],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "the vectors do not compile:\n{}",
        out.render()
    );
    out.ir.unwrap()
}

/// The 22 cases of `derived.json.pending` (the partial one is TypeScript only): the violations,
/// the values at the paths of `computed` and the same values through the generated public function,
/// for the client role and for both releases.
#[test]
fn the_core_vectors_pass_on_generated_java() {
    if !require_javac("the_core_vectors_pass_on_generated_java") {
        return;
    }
    let ir = core_ir();
    let all: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    let files = run(
        &ir,
        Role::Client,
        all.clone(),
        vec![],
        None,
        options("gen.v", None),
    );
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &files);
    sources.push(driver("DerivedVectors.java"));
    for release in releases() {
        javac_ok(work.path(), &sources, release);
    }
    let (mut cases, mut lines, mut total, mut partial) = (Vec::new(), String::new(), 0, 0);
    for case in load_cases(&read(CORE_CASES)).unwrap() {
        total += 1;
        if case.partial {
            partial += 1;
            continue;
        }
        lines.push_str(
            &serde_json::json!({
                "id": cases.len(), "type": case.ty, "value": case.value, "request": case.request
            })
            .to_string(),
        );
        lines.push('\n');
        cases.push(case);
    }
    std::fs::write(work.path().join("cases.jsonl"), lines).unwrap();
    let tree = Tree::new(&ir, "gen.v", "gen.v.runtime", all);
    let mut types = String::new();
    for id in tree.decls() {
        let d = ir.decl(*id);
        if matches!(d.kind, DeclKind::Model(_) | DeclKind::Union(_)) {
            types.push_str(&format!(
                "{}\t{}.{}\n",
                d.name,
                tree.decl_package(d),
                type_name(d)
            ));
        }
    }
    std::fs::write(work.path().join("types.tsv"), types).unwrap();
    let out = java_main(
        work.path(),
        "veldtest.DerivedVectors",
        &["cases.jsonl", "types.tsv"],
    );
    assert!(
        out.status.success(),
        "driver failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    for case in &cases {
        if let Err(m) = run_case(&ir, case) {
            panic!("the reference evaluator disagrees with the vector: {m}");
        }
    }
    let (mut failures, mut seen) = (Vec::new(), 0);
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let case = &cases[v["id"].as_u64().unwrap() as usize];
        seen += 1;
        if let Some(e) = v.get("error") {
            failures.push(format!("{}: driver error {e}", case.name));
            continue;
        }
        let produced: Vec<Produced> = serde_json::from_value(v["violations"].clone()).unwrap();
        if let Err(m) = compare_produced(case, &produced) {
            failures.push(m.to_string());
        }
        for (path, want) in &case.computed {
            for key in ["computed", "viaPublic"] {
                let Some(full) = v.get(key) else { continue };
                let got = value_at(full, path).unwrap_or(&serde_json::Value::Null);
                if got != want {
                    failures.push(format!(
                        "{}: {key} at {path} is {got}, the vector says {want}",
                        case.name
                    ));
                }
            }
        }
    }
    assert_eq!(seen, cases.len(), "the driver answered {seen} cases");
    assert_eq!(
        (total, partial),
        (22, 1),
        "the vector file has 22 cases, one of them partial"
    );
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

// --- the examples through the generated test handler -------------------------------------------------

#[test]
fn every_derived_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_derived_example_passes_on_the_jdk_server") {
        return;
    }
    let ir = endpoints_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(
        work.path(),
        &generate(&ir, Role::Server, None, true, "gen.b"),
    );
    sources.push(driver("DerivedStubs.java"));
    sources.push(driver("DerivedServe.java"));
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), "veldtest.DerivedServe", &[]);
    let passed = run_every_example(&ir, &server.url);
    assert_eq!(passed, 9, "every example of the derived contract");
}

#[test]
fn every_derived_example_passes_through_the_generated_test_handler_on_spring() {
    let name = "every_derived_example_passes_on_spring";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    let ir = endpoints_ir();
    let files = generate(&ir, Role::Server, Some("spring"), true, "gen.b");
    let app = start_spring(
        "spring-derived",
        &files,
        &[
            "derived/DerivedStubs.java",
            "derived/spring-derived/App.java",
        ],
        "/__veld/test",
        &[],
    );
    let passed = run_every_example(&ir, &format!("http://127.0.0.1:{}", app.port));
    assert_eq!(passed, 9, "every example of the derived contract");
}

// --- the production handler, the client and events ----------------------------------------------------

/// A value the caller sent is replaced or dropped, a `require` and an invariant read the derived
/// values, a fact is given its derived field and one that fails is 500, an expression that fails in
/// a request is a 422, the handler's value is overwritten, and the output modes.
#[test]
fn the_production_handler_computes_derived_fields_over_http() {
    if !require_javac("the_production_handler_computes_derived_fields") {
        return;
    }
    let files = generate(&endpoints_ir(), Role::Server, None, false, "gen.b");
    let out = compile_and_run(
        &files,
        &["DerivedShopHttp.java"],
        "veldtest.DerivedShopHttp",
        &[],
    );
    assert!(out.contains("OK"), "{out}");
}

/// The principal is given its derived field, the result is computed before the invariants and
/// under the output mode, and every item of a stream is computed.
#[test]
fn the_principal_the_output_mode_and_a_stream_get_their_derived_fields() {
    if !require_javac("the_principal_the_output_mode_and_a_stream") {
        return;
    }
    let files = generate(&extra_ir(), Role::Server, None, false, "gen.b");
    let out = compile_and_run(
        &files,
        &["DerivedExtraHttp.java"],
        "veldtest.DerivedExtraHttp",
        &[],
    );
    assert!(out.contains("OK"), "{out}");
}

/// The client's local check computes the derived fields before it checks, never sends them, keeps a
/// derived value of a response as it came, and has the public function.
#[test]
fn the_client_computes_before_it_checks_and_never_sends_a_derived_field() {
    if !require_javac("the_client_computes_before_it_checks") {
        return;
    }
    let ir = endpoints_ir();
    let digest = ir.services[0].rules_digest.clone().unwrap();
    let files = generate(&ir, Role::Client, None, false, "gen.c");
    let out = compile_and_run(
        &files,
        &["DerivedClient.java"],
        "veldtest.DerivedClient",
        &[&digest],
    );
    assert!(out.contains("OK"), "{out}");
}

/// The publisher computes the derived fields of an event, the subscriber computes them again.
#[test]
fn a_publisher_computes_and_a_subscriber_computes_again() {
    if !require_javac("a_publisher_computes_and_a_subscriber_computes_again") {
        return;
    }
    let files = generate(&sales_ir(), Role::Server, None, false, "gen.e");
    let out = compile_and_run(
        &files,
        &["DerivedEvents.java"],
        "veldtest.DerivedEvents",
        &[],
    );
    assert!(out.contains("derived events ok"), "{out}");
}

// --- what is generated ------------------------------------------------------------------------------------

fn text(files: &FileSet, path: &str) -> String {
    String::from_utf8_lossy(files.get(path).unwrap_or_else(|| panic!("no file {path}")))
        .into_owned()
}

#[test]
fn the_chain_contract_is_snapshotted() {
    let ir = chain_ir();
    let server = generate(&ir, Role::Server, None, true, "gen.b");
    let names: Vec<&str> = server
        .paths()
        .filter(|p| !p.contains("/runtime/"))
        .collect();
    insta::assert_snapshot!("derived_server_file_list", names.join("\n"));
    for name in ["Line", "Order", "OverLimit"] {
        let path = format!("gen/b/chain/chain/{name}.java");
        insta::assert_snapshot!(format!("derived_{name}"), text(&server, &path));
    }
    for name in ["Derive", "Schema"] {
        let path = format!("gen/b/runtime/{name}.java");
        insta::assert_snapshot!(format!("derived_runtime_{name}"), text(&server, &path));
    }
    let client = generate(&ir, Role::Client, None, false, "gen.b");
    insta::assert_snapshot!(
        "derived_client_Order",
        text(&client, "gen/b/chain/chain/Order.java")
    );
}

/// A derived field is a nullable component, so the handler leaves it out of what it returns and it
/// is present in what it receives (`spec/targets/java.md` section 15.2); the public function and
/// the schema are there, and the order of the fields is the order of the wire.
#[test]
fn a_derived_field_is_a_nullable_component_with_a_public_function() {
    let server = generate(&chain_ir(), Role::Server, None, false, "gen.b");
    let order = text(&server, "gen/b/chain/chain/Order.java");
    assert!(
        order.contains("BigDecimal subtotal,\n    BigDecimal total")
            && order.contains(
                "Schema.field(\"total\", \"total\", Schema.decimal(), true, null, false)"
            ),
        "{order}"
    );
    assert!(
        order.contains(".derived(Order::deriveWire, \"subtotal\", \"total\")"),
        "{order}"
    );
    assert!(
        order.contains("public static Order derive(Order value)"),
        "{order}"
    );
    let subtotal = order.find("deriveFieldSubtotal(v, path, out);").unwrap();
    let total = order.find("deriveFieldTotal(v, path, out);").unwrap();
    assert!(
        subtotal < total,
        "the chain is computed in dependency order"
    );
    let error = text(&server, "gen/b/chain/chain/OverLimit.java");
    assert!(
        error.contains("public static Data derive(Data value)"),
        "an error has the function on its payload:\n{error}"
    );
}

/// A contract without derived fields has no trace of the feature in its files.
#[test]
fn a_contract_without_derived_fields_has_no_trace_of_them() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"], "gen.a", None);
    let client = gen_client(&ir, "gen.c");
    for files in [&server, &client] {
        for path in files.paths() {
            assert!(!path.ends_with("/Derive.java"), "{path} exists");
        }
        for (path, bytes) in files.iter() {
            let text = String::from_utf8_lossy(bytes);
            for word in ["Derive", ".derived(", "deriveWire", "derivedWires"] {
                assert!(!text.contains(word), "{path} mentions `{word}`");
            }
        }
    }
}

/// A derived field only a fact has (no endpoint reaches it) is the server's concern, but the model
/// is emitted as the contract declares it in both roles, so both carry `Derive` and compile.
#[test]
fn a_derived_field_only_a_fact_has_is_generated_in_both_roles() {
    let (ir, _) = veld_emit_conformance::compiled_derived_contracts()
        .into_iter()
        .nth(1)
        .unwrap();
    for role in [Role::Server, Role::Client] {
        let files = generate(&ir, role, None, false, "gen.b");
        assert!(
            files.paths().any(|p| p.ends_with("/Derive.java")),
            "{}: Derive",
            role.name()
        );
    }
}

// --- the generated code compiles with every warning on ---------------------------------------------

fn conformance_irs() -> Vec<Ir> {
    veld_emit_conformance::compiled_derived_contracts()
        .into_iter()
        .map(|(ir, _)| ir)
        .collect()
}

#[test]
fn every_contract_with_derived_fields_compiles_with_every_warning_on() {
    if !require_javac("every_contract_with_derived_fields_compiles") {
        return;
    }
    let mut irs = vec![
        endpoints_ir(),
        extra_ir(),
        sales_ir(),
        chain_ir(),
        core_ir(),
    ];
    irs.extend(conformance_irs());
    for ir in &irs {
        for (role, framework, testing) in [
            (Role::Server, None, false),
            (Role::Server, None, true),
            (Role::Server, Some("spring"), false),
            (Role::Client, None, false),
        ] {
            // The Spring controllers need Spring on the class path: the Spring variant is built by
            // the Spring tests, so it is only generated here.
            let files = generate(ir, role, framework, testing, "gen.b");
            if framework.is_some() {
                assert!(
                    files.paths().any(|p| p.ends_with("/Derive.java")),
                    "spring: Derive"
                );
                continue;
            }
            let dir = tempfile::tempdir().unwrap();
            let sources = write_files(dir.path(), &files);
            for release in releases() {
                javac_ok(dir.path(), &sources, release);
            }
        }
    }
}

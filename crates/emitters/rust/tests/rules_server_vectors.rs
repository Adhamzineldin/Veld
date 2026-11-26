//! The shared rule vectors (`spec/rules.md` section 15, `tests/rules/core`) against the generated
//! Rust **server**: what reaches `on_rule_violation`.
//!
//! The client run (`rules.rs`) checks values with `check_wire`. A server also reports, so a generator
//! that never calls the hook for an audited rule would pass every vector there. Here each vector
//! file is a generated server crate with a service `Vectors` that has, for every model of the
//! file, an endpoint that takes it (`/v/in/<Type>`, step 5) and one that answers with it
//! (`/v/out/<Type>`, step 8). A driver binary, written by this test from the names in the IR
//! because Rust has no reflection, records the events of the hook and prints one JSON line per
//! case; this test compares the `Rejected` ones with `expect` and the `Audited` ones with `audited`
//! of the case, as sets, and the status of the answer with what `expect` says.
//!
//! Without `cargo` the test is skipped, unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role, naming};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_rust::tree::Tree;
use veld_ir::{DeclKind, Ir, PackageKind};
use veld_rules::vectors::{Case, Produced, compare_produced, compare_produced_audited, load_cases};

fn core_dir() -> PathBuf {
    repo_root().join("tests/rules/core")
}

fn model_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let mut words = line.split_whitespace();
        if let (Some("model" | "union"), Some(name)) = (words.next(), words.next()) {
            names.push(name.trim_end_matches('{').to_owned());
        }
    }
    names
}

/// The contract with the service that gives every model an endpoint that takes it and one that
/// answers with it.
fn vector_ir(stem: &str, source: &str) -> Ir {
    let mut text = source.replace("\r\n", "\n");
    text.push_str("\nservice Vectors at \"/v\" {\n");
    for m in model_names(source) {
        text.push_str(&format!("  post \"/in/{m}\" in{m}(body: {m}) -> 204\n"));
        text.push_str(&format!("  get \"/out/{m}\" out{m}() -> {m}\n"));
    }
    text.push_str("}\n");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: format!("{stem}.veld"),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(out.ir.is_some(), "{stem}: {}", out.render());
    out.ir.unwrap_or_else(|| unreachable!())
}

fn crate_ident(stem: &str) -> String {
    format!("svec_{stem}")
}

/// The driver: a service that answers with the case, hooks that record, and one request per case.
fn driver(ir: &Ir, stem: &str) -> String {
    let tree = Tree::new(
        ir,
        "crate",
        "src",
        ir.services.iter().map(|s| s.id).collect(),
        true,
    );
    let krate = crate_ident(stem);
    // Every contract of vectors has rules, so its server has a runtime config with the rule output
    // mode, and the target is named after the crate.
    let rt = veld_codegen::runtime_config::runtime_type_name(&format!("svec-{stem}"));
    let (mut methods, mut arms) = (String::new(), String::new());
    for id in tree.decls() {
        let d = ir.decl(*id);
        if !matches!(d.kind, DeclKind::Model(_) | DeclKind::Union(_))
            || d.name.is_empty()
            || tree.module_path(d) == "crate::prelude"
        {
            continue;
        }
        let path = tree.type_path(*id, "").replacen("crate", &krate, 1);
        let (inn, out) = (
            naming::snake(&format!("in{}", d.name)),
            naming::snake(&format!("out{}", d.name)),
        );
        let (inr, outr) = (
            naming::pascal(&format!("in{}", d.name)),
            naming::pascal(&format!("out{}", d.name)),
        );
        methods.push_str(&format!(
            "    async fn {inn}(&self, _r: {inr}Request, _c: &RequestContext) -> Result<(), Error> {{ Ok(()) }}\n\
             \x20   async fn {out}(&self, _r: {outr}Request, _c: &RequestContext) -> Result<{path}, Error> {{\n\
             \x20       let wire = CURRENT.lock().unwrap().clone();\n\
             \x20       serde_json::from_value({path}::schema().from_wire(&wire, false)).map_err(|e| Error::other(e.to_string()))\n\
             \x20   }}\n"
        ));
        arms.push_str(&format!("            {:?} => 0,\n", d.name));
    }
    let runtime_use =
        format!("use {krate}::runtime_config::{{{rt}, RuleOutputMode, RulesSettings}};");
    let mode = "    let mode = if off { RuleOutputMode::Off } else { RuleOutputMode::Strict };";
    let create = format!(
        "{krate}::t::vectors_server::VectorsHandler::create(Impl, Hooks {{ events: events.clone() }}, Some(&{rt} {{ rules: RulesSettings {{ output: mode }} }})).unwrap()"
    );
    format!(
        r#"use std::sync::{{Arc, Mutex}};

use {krate}::t::vectors_server::*;
use {krate}::veld::rules::{{Outcome, RuleEvent}};
{runtime_use}
use {krate}::veld::server::{{Body, Request, RequestContext}};
use {krate}::veld::{{Error, VeldDecl}};
use serde_json::{{Value, json}};

static CURRENT: Mutex<Value> = Mutex::new(Value::Null);

struct Impl;

impl VectorsService for Impl {{
{methods}}}

struct Hooks {{
    events: Arc<Mutex<Vec<RuleEvent>>>,
}}

impl VectorsHooks for Hooks {{
    fn on_rule_violation(&self, event: &RuleEvent) {{
        self.events.lock().unwrap().push(event.clone());
    }}
}}

/// The path of the value that carries the rule: `body.rows[1]` and `result.rows[1]` are `$.rows[1]`.
fn value_path(path: &str) -> String {{
    for root in ["body", "result"] {{
        if let Some(rest) = path.strip_prefix(root) {{
            if rest.is_empty() || rest.starts_with('.') || rest.starts_with('[') {{
                return format!("${{rest}}");
            }}
        }}
    }}
    path.to_string()
}}

fn reported(events: &[RuleEvent], outcome: Outcome) -> Vec<Value> {{
    events
        .iter()
        .filter(|e| e.outcome == outcome)
        .map(|e| {{
            let mut v = e.violation.clone();
            v.path = value_path(&v.path);
            v.to_json()
        }})
        .collect()
}}

fn run(case: &Value) -> Value {{
    let events = Arc::new(Mutex::new(Vec::new()));
    let off = case["output"].as_str() == Some("off");
{mode}
    let handler = {create};
    let name = case["type"].as_str().unwrap_or("");
    let request = case["request"].as_bool().unwrap_or(false);
    *CURRENT.lock().unwrap() = case["value"].clone();
    let req = if request {{
        Request {{
            method: "POST".into(),
            path: format!("/v/in/{{name}}"),
            headers: vec![("content-type".into(), "application/json".into())],
            body: case["value"].to_string().into_bytes(),
            ..Request::default()
        }}
    }} else {{
        Request {{ method: "GET".into(), path: format!("/v/out/{{name}}"), ..Request::default() }}
    }};
    let _ = match name {{
{arms}        _ => 0,
    }};
    let Some(response) = {krate}::veld::block_on(handler.handle(req)) else {{
        return json!({{ "error": "no route" }});
    }};
    let body = match &response.body {{
        Body::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        _ => String::new(),
    }};
    let events = events.lock().unwrap();
    json!({{
        "status": response.status,
        "body": body,
        "violations": reported(&events, Outcome::Rejected),
        "audited": reported(&events, Outcome::Audited),
    }})
}}

fn main() {{
    let path = std::env::args().nth(1).expect("a case file");
    let text = std::fs::read_to_string(path).expect("readable");
    let cases: Vec<Value> = serde_json::from_str(&text).expect("json");
    for case in cases {{
        println!("{{}}", run(&case));
    }}
}}
"#
    )
}

fn crate_files(ir: &Ir, stem: &str) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    let mut files = run(
        ir,
        Role::Server,
        all,
        vec![],
        None,
        json!({ "crate-name": format!("svec-{stem}") }),
    );
    files
        .add(format!("src/bin/svectors_{stem}.rs"), driver(ir, stem))
        .unwrap();
    files
}

fn wanted_status(case: &Case) -> u64 {
    match (case.request, case.expect.is_empty()) {
        (true, false) => 422,
        (true, true) => 204,
        (false, false) => 500,
        (false, true) => 200,
    }
}

fn judge(stem: &str, line: &serde_json::Value, case: &Case) -> Result<(), String> {
    if let Some(e) = line.get("error") {
        return Err(format!("the driver said {e}"));
    }
    let produced = |key: &str| -> Vec<Produced> {
        serde_json::from_value(line[key].clone()).unwrap_or_default()
    };
    compare_produced(case, &produced("violations")).map_err(|m| m.to_string())?;
    compare_produced_audited(case, &produced("audited")).map_err(|m| m.to_string())?;
    let status = line["status"].as_u64().unwrap_or(0);
    if status != wanted_status(case) {
        return Err(format!(
            "case `{}` of {stem}: the answer is HTTP {status} {}, wanted {}",
            case.name,
            line["body"].as_str().unwrap_or(""),
            wanted_status(case)
        ));
    }
    Ok(())
}

#[test]
fn every_core_vector_gives_the_reference_result_on_the_rust_server() {
    if !cargo_or_skip() {
        return;
    }
    let mut stems: Vec<String> = std::fs::read_dir(core_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "veld"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    stems.sort();
    let ws = work_dir().join("rules-server-ws");
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let mut members = Vec::new();
    let mut irs = BTreeMap::new();
    for stem in &stems {
        let source = std::fs::read_to_string(core_dir().join(format!("{stem}.veld"))).unwrap();
        let ir = vector_ir(stem, &source);
        write_files(&ws.join(stem), &crate_files(&ir, stem));
        members.push(format!("\"{stem}\""));
        irs.insert(stem.clone(), ir);
    }
    std::fs::write(
        ws.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
            members.join(", ")
        ),
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    run_ok(
        &mut cargo(&ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    let (mut passed, mut audited, mut failures) = (0, 0, Vec::new());
    for stem in &stems {
        let cases: Vec<Case> =
            load_cases(&std::fs::read_to_string(core_dir().join(format!("{stem}.json"))).unwrap())
                .unwrap()
                .into_iter()
                .filter(|c| !c.partial)
                .collect();
        let input: Vec<serde_json::Value> = cases
            .iter()
            .map(|c| json!({ "type": c.ty, "value": c.value, "request": c.request, "output": c.output }))
            .collect();
        let file = ws.join(format!("{stem}.cases.json"));
        std::fs::write(&file, serde_json::to_string(&input).unwrap()).unwrap();
        let out = run_ok(
            &mut cargo(
                &ws,
                &[
                    "run",
                    "-q",
                    "-p",
                    &format!("svec-{stem}"),
                    "--bin",
                    &format!("svectors_{stem}"),
                    "--",
                    &file.to_string_lossy(),
                ],
            ),
            &format!("server vectors of {stem}"),
        );
        let lines: Vec<serde_json::Value> = out
            .lines()
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
            .collect();
        assert_eq!(lines.len(), cases.len(), "{stem}: one line per case");
        for (case, line) in cases.iter().zip(&lines) {
            audited += usize::from(!case.audited.is_empty());
            match judge(stem, line, case) {
                Ok(()) => passed += 1,
                Err(m) => failures.push(format!("{stem} / {m}")),
            }
        }
    }
    eprintln!("server vectors: {passed} passed, {audited} with audited reports");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(
        audited >= 8,
        "the vectors of audited rules did not run: {audited}"
    );
    let _ = irs;
}

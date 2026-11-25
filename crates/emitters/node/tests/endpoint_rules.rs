//! Endpoint rules in the generated TypeScript (`spec/targets/typescript.md` section 5c): snapshots
//! of the server module, the test handler and the client; `tsc --strict` on all of them; an HTTP
//! round trip of the production handler and the client; and every example of
//! `tests/rules/endpoints` sent to the generated test handler.
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::*;
use serde_json::{Value as Json, json};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Binding, DeclId, Endpoint, Ir, PackageKind, Service, Type};
use veld_project::output::{WriteOptions, write};
use veld_rules::{Expected, FilledExample, diff_pattern, fill_example};

fn endpoint_contracts() -> Vec<PathBuf> {
    let dir = repo_root().join("tests/rules/endpoints");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "veld"))
        .collect();
    files.sort();
    files
}

fn compile_text(text: String) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "bookings.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "the contract failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

/// `tests/rules/endpoints/bookings.veld`: the examples are the vectors.
fn endpoints_ir() -> Ir {
    let path = repo_root().join("tests/rules/endpoints/bookings.veld");
    compile_text(std::fs::read_to_string(path).unwrap())
}

fn testing() -> serde_json::Value {
    json!({ "testing": true })
}

// ---- snapshots and the shape of the output ------------------------------------------------

#[test]
fn snapshots_of_endpoint_rules() {
    let ir = endpoints_ir();
    let server = gen_server_with(&ir, "Bookings", &[], &testing());
    insta::assert_snapshot!(
        "endpoint_rules_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "endpoint_rules_server_bookings",
        text(&server, "server/bookings.ts")
    );
    insta::assert_snapshot!(
        "endpoint_rules_test_handler",
        text(&server, "server/bookings-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!("endpoint_rules_client", text(&client, "bookings/client.ts"));
}

#[test]
fn the_production_entry_point_does_not_change_with_testing() {
    let ir = endpoints_ir();
    let off = gen_server(&ir, "Bookings", &[]);
    let on = gen_server_with(&ir, "Bookings", &[], &testing());
    assert!(off.get("server/bookings-test-handler.ts").is_none());
    assert!(on.get("server/bookings-test-handler.ts").is_some());
    for (path, bytes) in off.iter() {
        assert_eq!(
            Some(bytes),
            on.get(path),
            "{path} changed when `testing` was switched on"
        );
    }
    assert_eq!(off.len() + 1, on.len(), "only the test handler is new");
    // Nothing exports the test handler, and nothing in the production module reads a test header.
    assert!(!text(&on, "index.ts").contains("test-handler"));
    assert!(!text(&on, "server/bookings.ts").contains("veld-test"));
    assert!(
        !text(&on, "runtime/veld.ts")
            .to_lowercase()
            .contains("veld-test")
    );
}

#[test]
fn the_test_handler_says_at_its_top_that_it_must_never_be_mounted_in_production() {
    let ir = endpoints_ir();
    let on = gen_server_with(&ir, "Bookings", &[], &testing());
    let file = text(&on, "server/bookings-test-handler.ts");
    let top: String = file.lines().take(8).collect::<Vec<_>>().join("\n");
    assert!(top.contains("NEVER MOUNT IT IN PRODUCTION"), "{top}");
    assert!(file.contains("/__veld/test"));
    assert!(
        file.contains("Never mount it in production"),
        "reported on creation"
    );
}

#[test]
fn a_contract_with_invariants_only_gets_none_of_it() {
    let ir = rules_ir();
    let server = gen_server_with(&ir, "Bookings", &[], &testing());
    assert!(
        !server.paths().any(|p| p.contains("test-handler")),
        "no endpoint rules, no test handler"
    );
    let runtime = text(&server, "runtime/veld.ts");
    for word in [
        "EndpointRuleSet",
        "onContractMismatch",
        "veld-rules",
        "EarlyResponse",
    ] {
        assert!(
            !runtime.contains(word),
            "`{word}` leaked into an invariants-only runtime"
        );
    }
    let client = gen_client(&ir);
    assert!(!text(&client, "bookings/client.ts").contains("digest"));
    assert!(!text(&server, "server/bookings.ts").contains("digest"));
}

#[test]
fn the_endpoint_rules_regions_come_with_endpoint_rules() {
    let ir = endpoints_ir();
    let client = gen_client(&ir);
    let server = gen_server(&ir, "Bookings", &[]);
    assert_eq!(
        text(&server, "runtime/veld.ts"),
        text(&client, "runtime/veld.ts"),
        "one runtime in both roles"
    );
    assert!(text(&client, "runtime/veld.ts").contains("EndpointRuleSet"));
    assert!(text(&client, "bookings/client.ts").contains("digest:"));
}

#[test]
fn the_testing_option_must_be_a_boolean() {
    let ir = endpoints_ir();
    let all = vec![service_id(&ir, "Bookings")];
    let req = veld_codegen::GenRequest {
        ir: &ir,
        role: veld_codegen::Role::Server,
        services: all,
        consumes: vec![],
        framework: None,
    };
    let bad = veld_codegen::DynGenerator::generate_dyn(
        &veld_emit_node::Node,
        &req,
        &json!({ "testing": "yes" }),
    );
    assert!(bad.is_err());
}

// ---- toolchain ------------------------------------------------------------------------------

fn toolchain_dir() -> PathBuf {
    repo_root().join("tests/toolchain/ts")
}

fn tsc_path() -> Option<PathBuf> {
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    let tsc = toolchain_dir().join("node_modules/typescript/bin/tsc");
    if node_ok && tsc.exists() {
        return Some(tsc);
    }
    assert!(
        std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
        "toolchain required but unavailable"
    );
    eprintln!("SKIPPED endpoint rules toolchain test");
    None
}

fn run(cmd: &mut Command, what: &str) {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{what} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write_config(work: &Path, name: &str, value: &serde_json::Value) {
    std::fs::write(
        work.join(name),
        serde_json::to_string_pretty(value).unwrap(),
    )
    .unwrap();
}

/// Generates the server (with the test handler) and the client, and compiles the drivers.
fn build(work: &Path, tsc: &Path, edition: &str, target: &str) {
    let ir = endpoints_ir();
    let opts = WriteOptions::default();
    let option = json!({ "typescript": edition, "testing": true });
    write(
        &work.join("server"),
        &gen_server_with(&ir, "Bookings", &[], &option),
        &opts,
    )
    .unwrap();
    write(
        &work.join("client"),
        &gen_client_with(&ir, &typescript_option(edition)),
        &opts,
    )
    .unwrap();
    for driver in ["endpoint_rules_http.ts", "endpoint_rules_serve.ts"] {
        std::fs::copy(toolchain_dir().join(driver), work.join(driver)).unwrap();
    }
    // 1. Each generated package compiles on its own under `--strict`, with no types packages.
    write_config(
        work,
        "tsconfig.generated.json",
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target], "types": [],
                "module": "commonjs", "moduleResolution": "node10", "noEmit": true,
                "skipLibCheck": false, "forceConsistentCasingInFileNames": true
            },
            "include": ["server/**/*.ts", "client/**/*.ts"]
        }),
    );
    run(
        Command::new("node")
            .arg(tsc)
            .args(["-p", "tsconfig.generated.json"])
            .current_dir(work),
        "tsc generated",
    );
    // 2. The drivers compile with the Node types.
    write_config(
        work,
        "tsconfig.json",
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target, "dom"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "server/**/*.ts", "client/**/*.ts"]
        }),
    );
    run(
        Command::new("node")
            .arg(tsc)
            .args(["-p", "tsconfig.json"])
            .current_dir(work),
        "tsc drivers",
    );
}

#[test]
fn endpoint_rules_compile_strict_and_behave_over_http() {
    let Some(tsc) = tsc_path() else { return };
    for (edition, target) in TARGETS {
        std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
        let guard = tempfile::Builder::new()
            .prefix("endpoint-rules-")
            .tempdir_in(toolchain_dir().join(".work"))
            .unwrap();
        let work = guard.path();
        build(work, &tsc, edition, target);
        run(
            Command::new("node")
                .arg("out/endpoint_rules_http.js")
                .current_dir(work),
            "node endpoint_rules_http.js",
        );
        run_examples(work, &endpoints_ir_all());
    }
}

// ---- the examples are the vectors -------------------------------------------------------------

/// Every contract of `tests/rules/endpoints`, compiled.
fn endpoints_ir_all() -> Vec<Ir> {
    endpoint_contracts()
        .into_iter()
        .map(|p| compile_text(std::fs::read_to_string(p).unwrap()))
        .collect()
}

/// A running test handler; killed when dropped.
struct Live {
    tree: veld_test_support::ProcessTree,
    url: String,
}

impl Live {
    fn start(work: &Path) -> Live {
        let mut cmd = Command::new("node");
        cmd.arg("out/endpoint_rules_serve.js")
            .current_dir(work)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut tree =
            veld_test_support::ProcessTree::spawn(&mut cmd).expect("cannot start the server");
        let mut line = String::new();
        BufReader::new(tree.child_mut().stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port = line
            .trim()
            .strip_prefix("PORT=")
            .unwrap_or_else(|| panic!("the server did not report a port: {line:?}"));
        Live {
            url: format!("http://127.0.0.1:{port}"),
            tree,
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.tree.kill_tree();
    }
}

/// What came back.
struct Reply {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Json,
}

fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn scalar_text(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The request an example stands for: the parameters bound the way the generated client binds
/// them (path, query, header, body), and the `Veld-Test-*` headers.
struct Call {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Json>,
}

fn bind(svc: &Service, ep: &Endpoint, filled: &FilledExample) -> Call {
    let mut path = ep.path.clone();
    let mut query: Vec<(String, String)> = Vec::new();
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body = None;
    for p in ep.params.iter().filter(|p| !p.stream) {
        let Some(value) = filled.request.params.get(&p.name) else {
            continue;
        };
        match p.binding {
            Binding::Path => {
                path = path.replace(
                    &format!("{{{}}}", p.name),
                    &percent_encode(&scalar_text(value)),
                );
            }
            Binding::Query => match value {
                Json::Array(items) => {
                    query.extend(items.iter().map(|i| (p.name.clone(), scalar_text(i))));
                }
                other => query.push((p.name.clone(), scalar_text(other))),
            },
            Binding::QueryObject => {
                if let Json::Object(map) = value {
                    query.extend(map.iter().map(|(k, v)| (k.clone(), scalar_text(v))));
                }
            }
            Binding::Header => headers.push((
                p.header.clone().unwrap_or_else(|| p.name.clone()),
                scalar_text(value),
            )),
            Binding::Body => body = Some(value.clone()),
        }
    }
    query.sort();
    if !query.is_empty() {
        let pairs: Vec<String> = query
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect();
        path = format!("{path}?{}", pairs.join("&"));
    }
    let facts: serde_json::Map<String, Json> = filled
        .request
        .facts
        .iter()
        .map(|(k, v)| (k.clone(), v.clone().unwrap_or(Json::Null)))
        .collect();
    let test = |h: &str, v: &Json| (h.to_string(), base64url(v.to_string().as_bytes()));
    headers.push(test("Veld-Test-Facts", &Json::Object(facts)));
    if let Some(principal) = &filled.request.principal {
        headers.push(test("Veld-Test-Principal", principal));
    }
    headers.push(("Veld-Test-Now".to_string(), filled.clock.clone()));
    match &filled.request.result {
        Some(result) => headers.push(test("Veld-Test-Result", result)),
        None => headers.push(("Veld-Test-Dry-Run".to_string(), "1".to_string())),
    }
    let prefix = svc.prefix.trim_end_matches('/');
    assert!(path.starts_with(prefix), "{path} is outside {prefix}");
    Call {
        method: ep.method.as_str().to_string(),
        path,
        headers,
        body,
    }
}

fn send(live: &Live, call: &Call) -> Reply {
    let mut req = ureq::request(&call.method, &format!("{}{}", live.url, call.path));
    for (k, v) in &call.headers {
        req = req.set(k, v);
    }
    let res = match &call.body {
        Some(b) => req
            .set("content-type", "application/json")
            .send_string(&b.to_string()),
        None => req.call(),
    };
    let res = match res {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => panic!("{} {}: {e}", call.method, call.path),
    };
    let status = res.status();
    let headers = res
        .headers_names()
        .into_iter()
        .filter_map(|n| res.header(&n).map(|v| (n.to_lowercase(), v.to_string())))
        .collect();
    let text = res.into_string().unwrap();
    let body = serde_json::from_str(&text).unwrap_or(Json::Null);
    Reply {
        status,
        headers,
        body,
    }
}

fn decl_name(ir: &Ir, decl: DeclId) -> String {
    ir.decl(decl).name.clone()
}

/// Checks the reply against what the example states; `Err` describes the difference.
fn check(ir: &Ir, ep: &Endpoint, filled: &FilledExample, reply: &Reply) -> Result<(), String> {
    match &filled.expected {
        Expected::Ok(pattern) => {
            if filled.request.result.is_none() {
                if reply.status != 204
                    || reply.headers.get("veld-test-passed").map(String::as_str) != Some("1")
                {
                    return Err(format!(
                        "expected a passed dry run, got {} {}",
                        reply.status, reply.body
                    ));
                }
                return Ok(());
            }
            if !(200..300).contains(&reply.status) {
                return Err(format!(
                    "expected success, got {} {}",
                    reply.status, reply.body
                ));
            }
            let (Some(pattern), Some(ty)) = (pattern, &ep.success.ty) else {
                return Ok(());
            };
            diff(ir, ty, pattern, &reply.body)
        }
        Expected::Error { decl, pattern } => {
            let name = decl_name(ir, *decl);
            let status = ep
                .errors
                .iter()
                .find(|e| e.decl == *decl)
                .map(|e| e.status)
                .ok_or_else(|| format!("{name} is not an error of the endpoint"))?;
            if reply.status != status || reply.body["title"] != json!(name) {
                return Err(format!(
                    "expected {status} {name}, got {} {}",
                    reply.status, reply.body
                ));
            }
            match pattern {
                Some(p) => diff(ir, &Type::Decl(*decl), p, &reply.body["data"]),
                None => Ok(()),
            }
        }
    }
}

fn diff(ir: &Ir, ty: &Type, pattern: &Json, actual: &Json) -> Result<(), String> {
    let found = diff_pattern(ir, ty, pattern, actual);
    if found.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the pattern {pattern} does not match {actual}: {found:?}"
        ))
    }
}

/// Sends every example of every contract to the generated test handler.
fn run_examples(work: &Path, contracts: &[Ir]) {
    let live = Live::start(work);
    // It says what it is, so `veld test --live` can tell it from a production server.
    let who = ureq::get(&format!("{}/__veld/test", live.url))
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    let who: Json = serde_json::from_str(&who).unwrap();
    assert_eq!(who["veldTestHandler"], json!(1));
    let (mut passed, mut failures) = (0, Vec::new());
    for ir in contracts {
        let digests = ir
            .services
            .iter()
            .filter_map(|s| s.rules_digest.as_ref().map(|d| (s.name.clone(), json!(d))))
            .collect::<serde_json::Map<_, _>>();
        assert_eq!(who["digests"], Json::Object(digests));
        for ex in &ir.examples {
            let filled = fill_example(ir, ex).unwrap_or_else(|e| panic!("{}: {e}", ex.name));
            let svc = ir.service(filled.service);
            let ep = svc
                .endpoints
                .iter()
                .find(|e| e.name == filled.endpoint)
                .unwrap();
            let reply = send(&live, &bind(svc, ep, &filled));
            match check(ir, ep, &filled, &reply) {
                Ok(()) => passed += 1,
                Err(why) => failures.push(format!("\"{}\": {why}", ex.name)),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} examples failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("endpoint examples passed through the generated test handler: {passed}");
    assert_eq!(passed, 20, "every example of tests/rules/endpoints");
}

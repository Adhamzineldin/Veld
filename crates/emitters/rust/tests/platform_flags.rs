//! Flags in generated Rust (`spec/platform.md` section 11, `spec/targets/rust.md` section 17.2):
//! what is generated for `tests/toolchain/contract/platform_p2/flags.veld`, the core vectors with
//! `flags` and `provider: "failed"` through generated code, the production handler over HTTP
//! (`tests/toolchain/rust/flags-harness`) and the 14 examples of
//! `tests/rules/endpoints/flags.veld.pending` through the generated test handler.
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeSet;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_emit_rust::tree::Tree;
use veld_ir::{DeclKind, Ir};
use veld_rules::vectors::{Case, Produced, compare_produced, load_cases};

const FLAGS: &str = "tests/toolchain/contract/platform_p2/flags.veld";
const EXAMPLES: &str = "tests/rules/endpoints/flags.veld.pending";
const STREAMS: &str = "tests/toolchain/contract/platform_p2/flags_streams.veld";

fn flags_ir() -> Ir {
    compile_path(FLAGS, "rooms")
}

fn server_with(ir: &Ir, options: serde_json::Value) -> FileSet {
    let mut o = json!({ "crate-name": "rooms-server" });
    o.as_object_mut()
        .unwrap()
        .extend(options.as_object().cloned().unwrap_or_default());
    run(
        ir,
        Role::Server,
        vec![service_id(ir, "Rooms")],
        vec![],
        None,
        o,
    )
}

#[test]
fn the_server_and_client_snapshots() {
    let ir = flags_ir();
    let server = server_with(&ir, json!({ "testing": true }));
    insta::assert_snapshot!("flags_server", text(&server, "src/rooms/rooms_server.rs"));
    insta::assert_snapshot!(
        "flags_policies",
        text(&server, "src/rooms/rooms_policies.rs")
    );
    insta::assert_snapshot!(
        "flags_test_handler",
        text(&server, "src/rooms/rooms_test_server.rs")
    );
    let client = gen_client(&ir, "rooms-client", None);
    insta::assert_snapshot!("flags_client", text(&client, "src/rooms/rooms_client.rs"));
    insta::assert_snapshot!(
        "flags_client_policies",
        text(&client, "src/rooms/rooms_policies.rs")
    );
}

#[test]
fn the_provider_the_reading_and_the_statements_are_generated() {
    let ir = flags_ir();
    let server = server_with(&ir, json!({}));
    let body = text(&server, "src/rooms/rooms_server.rs");
    for word in [
        "type Flags: veld::flags::FlagProvider;",
        "fn flags(&self) -> &Self::Flags;",
        "pub fn flag_names(endpoint: usize) -> &'static [&'static str]",
        "async fn read_flags(&self, endpoint: usize, ctx: &veld::server::RequestContext)",
        "veld::flags::FlagProvider::is_on(self.hooks.flags(), name, &context).await",
        "!ctx.flags.statement_on(\"openOnly\", true)",
        "vr::with_view(Some(&ctx.flags), ||",
        "ctx.flags.statement_on(\"archiving\", false)",
        "!state.flags.statement_on(\"capacityGuard\", true)",
    ] {
        assert!(body.contains(word), "{word}\n{body}");
    }
    let policies = text(&server, "src/rooms/rooms_policies.rs");
    assert!(
        policies.contains("vr::flag_on(\"betaDesk\", false)"),
        "{policies}"
    );
    assert!(
        policies.contains("vr::flag_on(\"lockSuspended\", true)"),
        "{policies}"
    );
    // NoHooks and `plain` need no hooks: with a provider to name they are not generated.
    assert!(!body.contains("pub struct NoHooks") && !body.contains("pub fn plain"));
    // The client has the function argument and no provider, no reading, no window.
    let client = gen_client(&ir, "rooms-client", None);
    let client_body = text(&client, "src/rooms/rooms_client.rs");
    assert!(client_body.contains("flag_answers: Option<&dyn Fn(&str) -> bool>"));
    for (path, bytes) in client.iter() {
        let body = String::from_utf8_lossy(bytes);
        assert!(
            !body.contains("FlagProvider")
                && !body.contains("read_flags")
                && !body.contains("flags.statement_on"),
            "{path}"
        );
    }
}

/// A contract without a flag has none of the words of the feature, in the runtime either.
#[test]
fn a_contract_without_flags_has_none_of_the_words() {
    let ir = compile_path("tests/rules/endpoints/transitions.veld", "bookings");
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Bookings")],
        vec![],
        None,
        json!({ "crate-name": "bookings-server" }),
    );
    for (path, bytes) in server.iter() {
        let body = String::from_utf8_lossy(bytes);
        for word in [
            "FlagView",
            "FlagProvider",
            "read_flags",
            "flag_on",
            "invariant_on",
            "flag_names",
            "statement_on",
        ] {
            assert!(!body.contains(word), "{path}: {word}");
        }
    }
    assert!(!server.paths().any(|p| p.ends_with("flags.rs")));
}

// ---- the core vectors ---------------------------------------------------------------------------------

/// The contract of the vectors with a stub service, as `rules.rs` does for the other contracts.
fn vector_ir() -> Ir {
    let mut text =
        std::fs::read_to_string(repo_root().join("tests/rules/core/flags.veld.pending")).unwrap();
    let first = text
        .lines()
        .find_map(|l| l.strip_prefix("model "))
        .and_then(|l| l.split_whitespace().next())
        .unwrap()
        .to_string();
    text.push_str(&format!(
        "\nservice Probe at \"/probe\" {{\n  get \"/p\" probe() -> {first}\n}}\n"
    ));
    compile_text(&text, "t")
}

/// Every flag the contract names, sorted.
fn flag_names(ir: &Ir) -> Vec<String> {
    let set: BTreeSet<String> = ir
        .decls
        .iter()
        .flat_map(|d| d.rules().iter().filter_map(|r| r.flag.clone()))
        .collect();
    set.into_iter().collect()
}

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
        if matches!(
            d.kind,
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Union(_)
        ) && !d.name.is_empty()
            && tree.module_path(d) != "crate::prelude"
        {
            let path = tree.type_path(*id, "").replacen("crate", "vec_flags", 1);
            arms.push_str(&format!(
                "        {:?} => run::<{path}>(value, request, &view),\n",
                d.name
            ));
        }
    }
    let all: Vec<String> = flag_names(ir).iter().map(|f| format!("{f:?}")).collect();
    format!(
        r#"use vec_flags::veld::{{self, VeldDecl}};
use serde_json::Value;

const ALL: [&str; {n}] = [{all}];

fn run<T: VeldDecl>(value: &Value, request: bool, view: &veld::rules::FlagView) -> String {{
    let issues = T::schema().validate(value, "$", request);
    if !issues.is_empty() {{
        return format!("invalid {{}}", issues.len());
    }}
    let mode = if request {{ veld::rules::Mode::Request }} else {{ veld::rules::Mode::Full }};
    let found = veld::rules::with_view(Some(view), || veld::rules::check_wire(T::schema(), value, "$", mode));
    let items: Vec<String> = found.iter().map(veld::rules::Violation::to_text).collect();
    format!("[{{}}]", items.join(","))
}}

fn main() {{
    let path = std::env::args().nth(1).expect("a case file");
    let text = std::fs::read_to_string(path).expect("readable");
    let cases: Vec<Value> = serde_json::from_str(&text).expect("json");
    for case in cases {{
        let value = &case["value"];
        let request = case["request"].as_bool().unwrap_or(false);
        let failed = case["provider"].as_str() == Some("failed");
        let on: Vec<&str> = case["flags"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let mut view = veld::rules::FlagView::new();
        for name in ALL {{
            let reading = if failed {{ veld::rules::Reading::Failed }} else if on.contains(&name) {{ veld::rules::Reading::On }} else {{ veld::rules::Reading::Off }};
            view.insert(name, reading);
        }}
        let out = match case["type"].as_str().unwrap_or("") {{
{arms}            _ => "{{\"unknown\":0}}".to_string(),
        }};
        println!("{{out}}");
    }}
}}
"#,
        n = all.len(),
        all = all.join(", ")
    )
}

/// The cases of `tests/rules/core/flags.json.pending` (`flags` and `provider: "failed"`) through a
/// generated client crate whose driver puts the readings of the case in force the way the server
/// does: a flag the case names is on, every other flag the contract has is off, and a failed
/// provider fails every reading.
#[test]
fn the_flag_vectors_pass_through_generated_rust() {
    if !cargo_or_skip() {
        return;
    }
    let ir = vector_ir();
    let all = ir.services.iter().map(|s| s.id).collect();
    let mut files = run(
        &ir,
        Role::Client,
        all,
        vec![],
        Some("ureq"),
        json!({ "crate-name": "vec-flags" }),
    );
    files.add("src/bin/vectors_flags.rs", driver(&ir)).unwrap();
    let ws = work_dir().join("flags-vectors-ws");
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    write_files(&ws.join("vec-flags"), &files);
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"vec-flags\"]\n",
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
    run_ok(
        &mut cargo(
            &ws,
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
    let text =
        std::fs::read_to_string(repo_root().join("tests/rules/core/flags.json.pending")).unwrap();
    let cases: Vec<Case> = load_cases(&text)
        .unwrap()
        .into_iter()
        .filter(|c| !c.partial)
        .collect();
    let input: Vec<serde_json::Value> = cases
        .iter()
        .map(|c| json!({ "type": c.ty, "value": c.value, "request": c.request, "flags": c.flags, "provider": c.provider }))
        .collect();
    let file = ws.join("flags.cases.json");
    std::fs::write(&file, serde_json::to_string(&input).unwrap()).unwrap();
    let out = run_ok(
        &mut cargo(
            &ws,
            &[
                "run",
                "-q",
                "-p",
                "vec-flags",
                "--bin",
                "vectors_flags",
                "--",
                &file.to_string_lossy(),
            ],
        ),
        "the flag vectors",
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), cases.len(), "one line per case");
    let mut failures = Vec::new();
    for (case, line) in cases.iter().zip(&lines) {
        match serde_json::from_str::<Vec<Produced>>(line) {
            Ok(produced) => {
                if let Err(e) = compare_produced(case, &produced) {
                    failures.push(format!("{e}"));
                }
            }
            Err(_) => failures.push(format!("case `{}` produced {line}", case.name)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(
        cases
            .iter()
            .any(|c| c.provider.as_deref() == Some("failed")),
        "the failed-provider cases ran"
    );
}

// ---- the production handler and the examples -------------------------------------------------------------

#[test]
fn flags_build_behave_over_http_and_pass_every_example() {
    if !cargo_or_skip() {
        return;
    }
    let ir = flags_ir();
    let server = server_with(&ir, json!({}));
    let client = gen_client(&ir, "rooms-client", None);
    let ws = harness_workspace("flags-ws", &server, &client, "flags-harness");
    build_lint_and_run(
        &ws,
        ("rooms-server", "rooms-client", "flags-harness"),
        &[],
        "flags ok",
    );
    let ir = compile_path(EXAMPLES, "rooms");
    let server = server_with(&ir, json!({ "testing": true }));
    let client = gen_client(&ir, "rooms-client", None);
    let ws = harness_workspace("flags-examples-ws", &server, &client, "flags-examples");
    run_ok(
        &mut cargo(&ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    run_ok(
        &mut cargo(
            &ws,
            &[
                "clippy",
                "--all-targets",
                "-p",
                "rooms-server",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
    run_every_example(&ir, "flags-serve", 14);
}

// ---- flagged invariants on stream items and WebSocket messages ---------------------------------------

/// The items of an SSE stream and the messages of a WebSocket, in both directions, are checked with
/// the flags the request read (`tests/toolchain/rust/flags-streams-harness`, over real sockets).
#[test]
fn a_flagged_invariant_of_a_stream_item_and_a_message_is_evaluated() {
    let ir = compile_path(STREAMS, "chat");
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Chat")],
        vec![],
        None,
        json!({ "crate-name": "chat-server" }),
    );
    let words = text(&server, "src/veld/server.rs");
    for word in [
        "fn with_flags(mut self",
        "let _window = self.io.window();",
        "with_view(Some(&st.ctx.flags)",
    ] {
        assert!(words.contains(word), "missing `{word}`");
    }
    if !cargo_or_skip() {
        return;
    }
    let client = gen_client(&ir, "chat-client", None);
    let ws = harness_workspace(
        "flags-streams-ws",
        &server,
        &client,
        "flags-streams-harness",
    );
    build_lint_and_run(
        &ws,
        ("chat-server", "chat-client", "flags-streams-harness"),
        &[],
        "flags streams ok",
    );
}

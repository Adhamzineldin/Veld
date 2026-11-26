//! The outbox of events phase E3 in the generated Rust server (`spec/events.md` section 11,
//! `spec/targets/rust.md` section 13): the `entries` of `tests/wire/events/outbox.json` through the
//! generated `emit`, its `relay` and `written` cases through the generated relay and the generated
//! check, the moves of `tests/events/outbox.veld.pending` over the production handler, the examples
//! with `expect emitted` through the generated test handler and `veld_testing::Live`, snapshots, the
//! gate (a contract without `emits` has none of the words), `E0801` for an `emit` parameter, a server
//! without an outbox that does not compile, the options of a channel at the relay, and the combination
//! with the features of platform phase P2.
//!
//! The programs are in `tests/toolchain/rust/outbox`; `entries` has none of its own, because Rust
//! has no reflection: its driver is written here from the names of the golden. Without `cargo` the
//! toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::*;
use serde_json::{Value, json};
use veld_codegen::naming::{pascal, snake};
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

const OUTBOX: &str = "tests/events/outbox.veld.pending";
const GOLDEN: &str = "tests/wire/events/outbox.json";
const FOUR: &str = "tests/toolchain/contract/platform_p2/outbox_four.veld";

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap()
        .replace("\r\n", "\n")
}

/// The program `name` of `tests/toolchain/rust/outbox`.
fn program(name: &str) -> String {
    read(&format!("tests/toolchain/rust/outbox/{name}"))
}

/// Compiles `text` as the package `bookings` from the file `t.veld`: the goldens were made so, and
/// the module name is part of the signatures.
fn compile_text(text: String) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
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
    out.ir.unwrap()
}

fn outbox_ir() -> Ir {
    compile_text(read(OUTBOX))
}

/// The server crate `events-server` of every service of the contract.
fn server_crate(ir: &Ir, options: &Value) -> FileSet {
    let mut o = json!({ "crate-name": "events-server" });
    o.as_object_mut()
        .unwrap()
        .extend(options.as_object().cloned().unwrap_or_default());
    let ids = ir.services.iter().map(|s| s.id).collect();
    run(ir, Role::Server, ids, vec![], None, o)
}

/// A workspace of the generated crate `events-server` and the harness crate `events-harness`, whose
/// binaries are `bins` (name and source); `common.rs` of the events drivers is in its `src`. `files`
/// are written next to the workspace file.
fn workspace(
    name: &str,
    server: &FileSet,
    bins: &[(&str, String)],
    files: &[(&str, String)],
) -> PathBuf {
    let ws = work_dir().join(name);
    let _ = std::fs::remove_dir_all(&ws);
    write_files(&ws.join("server"), server);
    let harness = ws.join("harness");
    std::fs::create_dir_all(harness.join("src/bin")).unwrap();
    std::fs::write(
        harness.join("Cargo.toml"),
        "[package]\nname = \"events-harness\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\nevents-server = { path = \"../server\" }\nserde_json = \"1\"\naxum = \"0.8\"\ntokio = { version = \"1\", features = [\"rt-multi-thread\", \"macros\", \"net\", \"time\"] }\n",
    )
    .unwrap();
    std::fs::write(
        harness.join("src/common.rs"),
        read("tests/toolchain/rust/events/common.rs"),
    )
    .unwrap();
    for (bin, source) in bins {
        std::fs::write(harness.join(format!("src/bin/{bin}.rs")), source).unwrap();
    }
    for (file, text) in files {
        std::fs::write(ws.join(file), text).unwrap();
    }
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"server\", \"harness\"]\n",
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    ws
}

/// Builds the workspace and runs clippy with `-D warnings` on the generated crate and the programs.
fn build_and_lint(ws: &std::path::Path) {
    run_ok(
        &mut cargo(ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    run_ok(
        &mut cargo(
            ws,
            &[
                "clippy",
                "--all-targets",
                "-p",
                "events-server",
                "-p",
                "events-harness",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
}

fn run_program(ws: &std::path::Path, bin: &str) -> String {
    run_ok(
        &mut cargo(ws, &["run", "-q", "-p", "events-harness", "--bin", bin]),
        &format!("the program {bin}"),
    )
}

// ---- `entries`: what `emit` saves ----------------------------------------------------------------

/// The cases of `entries` in `outbox.json`, grouped by contract, service and channel: the `source` of
/// a message is the service that owns the move, and the event goes to the first channel of that
/// service that carries it, so a group is one generated crate.
fn entry_groups() -> BTreeMap<(String, String, String), Vec<Value>> {
    let golden: Value = serde_json::from_str(&read(GOLDEN)).unwrap();
    let mut groups: BTreeMap<(String, String, String), Vec<Value>> = BTreeMap::new();
    for case in golden["entries"].as_array().unwrap() {
        let key = |k: &str| case[k].as_str().unwrap().to_owned();
        groups
            .entry((key("contract"), key("service"), key("channel")))
            .or_default()
            .push(case.clone());
    }
    groups
}

fn find_from(text: &str, pattern: &str, from: usize) -> Option<usize> {
    text[from..].find(pattern).map(|i| from + i)
}

/// `contract` with, for each of `events`, a machine `Zz<Event>State` with one edge that emits the
/// event, a model, and a move `zz<Event>` that the driver writes the event from, added to `service`.
/// `channel` is moved to the front of the `publishes` lines of the service, because the event goes
/// to the first channel of the service that carries it, and a service without `at` gets one (a copy
/// of `probe_contract` in `crates/emitters/node/tests/events.rs`).
fn probe_contract(contract: &str, service: &str, channel: &str, events: &[&str]) -> String {
    let at = contract
        .find(&format!("service {service}"))
        .expect("the service is in the contract");
    let open = find_from(contract, "{", at).expect("the service has a body");
    let line_end = contract[at..open].to_string();
    let head = if line_end.contains(" at \"") {
        line_end.clone()
    } else {
        format!("{} at \"/zz\" ", line_end.trim_end())
    };
    // The end of the body: the brace that closes the one at `open`.
    let (mut depth, mut close) = (0usize, open);
    for (i, ch) in contract[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &contract[open + 1..close];
    let mut channels = vec![channel.to_string()];
    let mut kept = Vec::new();
    for line in body.lines() {
        if let Some(list) = line.trim().strip_prefix("publishes ") {
            for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                if !channels.iter().any(|c| c == name) {
                    channels.push(name.to_string());
                }
            }
        } else {
            kept.push(line.to_string());
        }
    }
    let (mut moves, mut machines) = (String::new(), String::new());
    for e in events {
        moves.push_str(&format!(
            "  post \"/zz/{{id}}/{e}\" zz{e}(id: uuid) -> Zz{e}Box {{\n    fact zz{e}: Zz{e}Box = id\n    transition zz{e}.state: a -> b\n  }}\n"
        ));
        machines.push_str(&format!(
            "\nenum Zz{e}State {{\n  a, b\n  flow a -> b emits {e}\n  final b\n}}\n\nmodel Zz{e}Box {{\n  id: uuid\n  state: Zz{e}State\n}}\n"
        ));
    }
    format!(
        "{}{head}{{\n  publishes {}\n{}\n{moves}}}{}{machines}",
        &contract[..at],
        channels.join(", "),
        kept.join("\n"),
        &contract[close + 1..],
    )
}

/// The driver of the `entries` cases of one group: for each case, the `emit` of the probe move
/// saves the message of the golden in a recording outbox (the id and the clock are fixed), and the
/// driver prints what the outbox received.
fn entries_driver(service: &str, events: &[String]) -> String {
    let mut s = String::from(
        r#"#[path = "../common.rs"]
mod common;

use std::sync::{Arc, Mutex};

use common::*;
use events_server::veld::events::{BoxFuture, EmitLog, Outbox, OutboxEntry, PublisherOptions};
use events_server::veld::{Error, VeldDecl, block_on};
use serde_json::{Value, json};

/// What `save` was given: the topic, the key and the bytes.
type Saved = (String, Option<String>, Vec<u8>);

/// An outbox that keeps what `emit` saves.
#[derive(Default)]
struct Recording(Mutex<Vec<Saved>>);

impl Outbox for Recording {
    fn save<'a>(&'a self, topic: &'a str, key: Option<&'a str>, bytes: &'a [u8]) -> BoxFuture<'a, Result<(), Error>> {
        self.0.lock().unwrap().push((topic.to_string(), key.map(str::to_string), bytes.to_vec()));
        Box::pin(std::future::ready(Ok(())))
    }

    fn pending(&self, _limit: usize) -> BoxFuture<'_, Result<Vec<OutboxEntry>, Error>> {
        Box::pin(std::future::ready(Ok(Vec::new())))
    }

    fn done<'a>(&'a self, _entry: &'a OutboxEntry) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(std::future::ready(Ok(())))
    }
}

async fn entry_case(i: usize, case: &Value) -> Value {
    let outbox = Arc::new(Recording::default());
    let id = case["id"].as_str().unwrap().to_string();
    let at = time_from(case["time"].as_str().unwrap());
    let options = PublisherOptions::new().new_id(move || id.clone()).clock(move || at);
    let log = EmitLog::default();
    let result: Result<(), Error> = match i {
"#,
    );
    let module = format!("events_server::bookings::{}_events", snake(service));
    for (i, event) in events.iter().enumerate() {
        let emit = format!("Zz{}Emit", pascal(event));
        s += &format!(
            "        {i} => {{\n            let emit = {module}::{emit}::begin(&*outbox, options, &log);\n            let event = <events_server::bookings::t::{event} as VeldDecl>::from_wire(&case[\"value\"]).unwrap();\n            emit.{}(&event).await.map(drop)\n        }}\n",
            snake(event)
        );
    }
    s += r#"        _ => panic!("no such case"),
    };
    let saved: Vec<Value> = outbox.0.lock().unwrap().iter().map(|(t, k, b)| json!({ "topic": t, "key": k, "hex": hex(b) })).collect();
    json!({ "saved": saved, "error": result.err().map(|e| e.to_string()), "names": log.names() })
}

fn main() {
    let text = std::fs::read_to_string("cases.json").unwrap();
    let cases: Value = serde_json::from_str(&text).unwrap();
    let out = block_on(async {
        let mut all = Vec::new();
        for (i, case) in cases.as_array().unwrap().iter().enumerate() {
            all.push(entry_case(i, case).await);
        }
        Value::Array(all)
    });
    println!("{out}");
}
"#;
    s
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Every `entries` case of the golden through the generated `emit`: the topic, the key and the
/// bytes the outbox received are the golden's (so the `source` is the service that owns the move and
/// the channel is the case's), the event is named in the log of the request, and nothing reached a
/// transport (an outbox has none).
#[test]
fn the_entries_of_the_golden_are_what_emit_saves() {
    if !cargo_or_skip() {
        return;
    }
    for (n, ((contract, service, channel), cases)) in entry_groups().iter().enumerate() {
        let events: Vec<String> = cases
            .iter()
            .map(|c| c["event"].as_str().unwrap().to_owned())
            .collect();
        let mut distinct: Vec<&str> = events.iter().map(String::as_str).collect();
        distinct.sort_unstable();
        distinct.dedup();
        let ir = compile_text(probe_contract(&read(contract), service, channel, &distinct));
        // The event goes where the case says: the channel of each probe move is the case's.
        let owner = ir.services.iter().find(|s| &s.name == service).unwrap();
        for event in &distinct {
            let ep = owner
                .endpoints
                .iter()
                .find(|e| e.name == format!("zz{event}"))
                .unwrap();
            let emits = &ep.rules.transition.as_ref().unwrap().emits;
            assert_eq!(emits.len(), 1, "{contract}: {service}.zz{event}");
            assert_eq!(
                &ir.channel(emits[0].channel).name,
                channel,
                "{contract}: {service}.zz{event}"
            );
        }
        let ws = workspace(
            &format!("outbox-entries-{n}"),
            &server_crate(&ir, &json!({})),
            &[("entries", entries_driver(service, &events))],
            &[(
                "cases.json",
                serde_json::to_string(
                    &cases
                        .iter()
                        .map(|c| json!({ "id": c["id"], "time": c["time"], "value": c["value"] }))
                        .collect::<Vec<_>>(),
                )
                .unwrap(),
            )],
        );
        build_and_lint(&ws);
        let out: Value = serde_json::from_str(&run_program(&ws, "entries")).unwrap();
        let results = out.as_array().unwrap();
        assert_eq!(results.len(), cases.len(), "{contract} {service} {channel}");
        for (case, got) in cases.iter().zip(results) {
            let name = case["name"].as_str().unwrap();
            assert_eq!(got["error"], Value::Null, "{name}: {got}");
            let saved = got["saved"].as_array().unwrap();
            assert_eq!(saved.len(), 1, "{name}: one message saved");
            let want = case["canonical"].as_str().unwrap();
            assert_eq!(
                saved[0]["hex"].as_str().unwrap(),
                hex(want.as_bytes()),
                "{name}: the bytes differ from the golden\nwanted: {want}"
            );
            assert_eq!(saved[0]["topic"], case["topic"], "{name}: the topic");
            assert_eq!(saved[0]["key"], case["key"], "{name}: the key");
            assert_eq!(
                got["names"],
                json!([case["event"]]),
                "{name}: the event written"
            );
        }
    }
}

// ---- the relay, the check, the moves over the real entry point ----------------------------------------

/// The `relay` and `written` goldens through the generated relay and `check_written`
/// (`relay.rs`), and the moves of the contract over the production handler followed by the relay with
/// the memory transport and the subscriber of `Notifications` (`http.rs`).
#[test]
fn the_relay_the_check_and_the_moves_behave_as_the_goldens_and_the_spec_say() {
    if !cargo_or_skip() {
        return;
    }
    let ws = workspace(
        "outbox-moves",
        &server_crate(&outbox_ir(), &json!({})),
        &[("relay", program("relay.rs")), ("http", program("http.rs"))],
        &[("outbox.json", read(GOLDEN))],
    );
    build_and_lint(&ws);
    assert!(run_program(&ws, "relay").contains("outbox relay ok"));
    assert!(run_program(&ws, "http").contains("outbox http ok"));
}

/// The call examples with `expect emitted` and `expect not emitted` and the event examples of the
/// contract through the generated test handlers and the shared live run (`Veld-Test-Emitted`).
#[test]
fn the_examples_of_the_outbox_contract_pass_live_against_the_generated_test_handler() {
    if !cargo_or_skip() {
        return;
    }
    let ir = outbox_ir();
    let ws = workspace(
        "outbox-serve",
        &server_crate(&ir, &json!({ "testing": true })),
        &[("outbox-serve", program("serve.rs"))],
        &[],
    );
    build_and_lint(&ws);
    assert_eq!(ir.examples.len(), 10, "every example of the contract");
    let emitted = ir.examples.iter().filter(|e| e.emitted.is_some()).count();
    assert_eq!(emitted, 5, "the examples with an `expect emitted` line");
    run_every_example(&ir, "outbox-serve", ir.examples.len());
}

// ---- the options of a channel at the relay ---------------------------------------------------------------

const OPTIONS: &str = "event Ping {
  id: uuid
}

@topic(\"o.v1\")
@ordered
@deadLetter
channel Pings {
  carries Ping
  key id
}

enum State {
  a, b
  flow a -> b emits Ping
  final b
}

model Box {
  id: uuid
  state: State
}

service Pinger at \"/p\" {
  publishes Pings

  post \"/{id}\" go(id: uuid) -> Box {
    fact box: Box = id
    transition box.state: a -> b
  }
}
";

const OPTIONS_PROGRAM: &str = r#"#[path = "../common.rs"]
mod common;

use common::*;
use events_server::bookings::pinger_events::run_pinger_outbox_relay;
use events_server::veld::block_on;
use events_server::veld::events::{ChannelOptions, MemoryOutbox, Outbox, RelayOptions};

fn main() {
    block_on(async {
        let (outbox, recorder) = (MemoryOutbox::new(), Recorder::new());
        outbox.save("o.v1", Some("k"), b"x").await.unwrap();
        outbox.save("somewhere.else", None, b"y").await.unwrap();
        let sent = run_pinger_outbox_relay(&outbox, &*recorder, &RelayOptions::new()).await;
        assert_eq!(sent, 2);
        let seen = recorder.options.lock().unwrap().clone();
        assert_eq!(seen, vec![ChannelOptions { ordered: true, dead_letter: true }, ChannelOptions::NONE], "{seen:?}");
    });
    println!("outbox options ok");
}
"#;

/// The relay hands the transport the options of the channel of each entry's topic (section 10.6), and a
/// topic of no channel the options that are all off.
#[test]
fn the_relay_passes_the_options_of_the_channel_to_the_transport() {
    let files = server_crate(&compile_text(OPTIONS.to_owned()), &json!({}));
    let events = text(&files, "src/bookings/pinger_events.rs");
    assert!(events.contains(
        "(\"o.v1\", veld::events::ChannelOptions { ordered: true, dead_letter: true }),"
    ));
    if !cargo_or_skip() {
        return;
    }
    let ws = workspace(
        "outbox-options",
        &files,
        &[("options", OPTIONS_PROGRAM.to_owned())],
        &[],
    );
    build_and_lint(&ws);
    assert!(run_program(&ws, "options").contains("outbox options ok"));
}

// ---- the gates and the refusals ----------------------------------------------------------------------------

/// The same contract without `emits`: none of the words of the outbox in any file.
#[test]
fn a_contract_without_an_emitting_move_has_none_of_the_words_of_the_outbox() {
    // The examples that say what a call writes (`expect emitted`) need the move to emit.
    let text: String = read(OUTBOX)
        .replace(" emits BookingPaid", "")
        .replace(" emits BookingCancelled", "")
        .replace(" emits RefundDue", "")
        .lines()
        .filter(|l| {
            !l.trim_start().starts_with("expect emitted")
                && !l.trim_start().starts_with("expect not emitted")
        })
        .map(|l| format!("{l}\n"))
        .collect();
    let files = server_crate(&compile_text(text), &json!({ "testing": true }));
    for (path, bytes) in files.iter() {
        let body = String::from_utf8_lossy(bytes);
        for word in [
            "Outbox",
            "outbox",
            "Emission",
            "EmitLog",
            "emit",
            "RelayOptions",
            "test_emitted",
            "veld-test-emitted",
        ] {
            // The spec of a runtime comment is the only place the plain word may appear: none does.
            assert!(!body.contains(word), "`{word}` in {path}");
        }
    }
}

/// An emitting move gets the outbox, the check, the relay and the hooks.
#[test]
fn an_emitting_move_has_the_outbox_the_check_and_the_relay() {
    let files = server_crate(&outbox_ir(), &json!({ "testing": true }));
    let server = text(&files, "src/bookings/bookings_server.rs");
    for word in [
        "type Outbox: veld::events::Outbox;",
        "fn outbox(&self) -> &Self::Outbox;",
        "fn events(&self) -> veld::events::PublisherOptions",
        "emit: crate::bookings::bookings_events::PayEmit<'_>, ctx:",
        "transition: veld::rules::Transition<crate::bookings::t::Status>, emit: crate::bookings::bookings_events::PayEmit<'_>",
        "veld::events::check_emitted(\"pay\", emits, &ctx.emitted)?;",
        "PayEmit::begin(self.hooks.outbox(), self.hooks.events(), &ctx.emitted)",
    ] {
        assert!(server.contains(word), "missing `{word}`");
    }
    assert!(
        !server.contains("pub struct NoHooks") && !server.contains("pub fn plain"),
        "hooks that need an outbox have no default"
    );
    // The move that emits nothing has no `emit` argument.
    assert!(server.contains("fn refund(&self, request: RefundRequest, facts: RefundFacts, transition: veld::rules::Transition<crate::bookings::t::Status>, ctx:"));
    let events = text(&files, "src/bookings/bookings_events.rs");
    for word in [
        "pub struct PayEmit<'a>",
        "pub async fn booking_paid(&self, event: &crate::bookings::t::BookingPaid)",
        "pub struct CancelEmit<'a>",
        "pub async fn refund_due(&self, event: &crate::bookings::t::RefundDue)",
        "pub async fn run_bookings_outbox_relay(outbox: &dyn veld::events::Outbox, transport: &dyn veld::events::Transport, options: &veld::events::RelayOptions) -> usize",
        "\"pending\" => &[\"BookingPaid\"],",
    ] {
        assert!(events.contains(word), "missing `{word}`");
    }
    assert!(text(&files, "src/veld/events.rs").contains("pub struct MemoryOutbox"));
    let handler = text(&files, "src/bookings/bookings_test_server.rs");
    assert!(handler.contains("fn test_emitted(&self) -> bool"));
    assert!(handler.contains("veld::events::transition_from(&args)"));
    assert!(text(&files, "src/veld/server.rs").contains("veld-test-emitted"));
    // No other file than the runtime and the test handler names the header.
    for (path, bytes) in files.iter() {
        if path != "src/veld/server.rs" && path != "src/bookings/bookings_test_server.rs" {
            assert!(
                !String::from_utf8_lossy(bytes).contains("veld-test-emitted"),
                "{path}"
            );
        }
    }
}

/// A small contract with one emitting move: `@PARAM@` is added to its parameters and `@FACT@` is
/// the name of its fact.
const SMALL: &str = "event Paid { orderId: uuid }

channel PaymentEvents {
  carries Paid
  key orderId
}

enum Status {
  pending, paid
  flow pending -> paid emits Paid
  final paid
}

model Order { id: uuid, status: Status }

service Orders at \"/orders\" {
  publishes PaymentEvents

  post \"/{id}/pay\" pay(id: uuid@PARAM@) -> Order {
    fact @FACT@: Order = id
    transition @FACT@.status: pending -> paid
  }
}
";

/// `emit` is the name of the handler's argument: a parameter or a fact of that name on an emitting
/// move is `E0801`; another name generates.
#[test]
fn a_parameter_or_a_fact_called_emit_on_an_emitting_move_is_refused() {
    use veld_codegen::DynGenerator;
    let small = |param: &str, fact: &str| SMALL.replace("@PARAM@", param).replace("@FACT@", fact);
    let generate = |source: String| {
        let ir = compile_text(source);
        let req = veld_codegen::GenRequest {
            ir: &ir,
            role: Role::Server,
            services: ir.services.iter().map(|s| s.id).collect(),
            consumes: vec![],
            framework: None,
        };
        let options = veld_codegen::with_target_name(
            &json!({ "crate-name": "events-server" }),
            "events-server",
        );
        veld_emit_rust::Rust.generate_dyn(&req, &options)
    };
    assert!(generate(small("", "order")).is_ok());
    for (what, source) in [
        ("a parameter", small(", @query emit: bool", "order")),
        ("a fact", small("", "emit")),
    ] {
        let err = generate(source).expect_err(what);
        assert!(
            err.iter().all(|d| d.code == "E0801")
                && err.iter().any(|d| d.message.contains("`emit`")),
            "{what}: {err:?}"
        );
    }
}

/// A server whose hooks name no outbox does not compile: there is no handler to create, and no
/// request that finds out (`spec/events.md` section 11.1: "creating the handler fails").
#[test]
fn a_server_whose_hooks_name_no_outbox_does_not_compile() {
    if !cargo_or_skip() {
        return;
    }
    let bare = "use events_server::bookings::bookings_server::BookingsHooks;\n\nstruct Hooks;\n\nimpl BookingsHooks for Hooks {}\n\nfn main() {}\n";
    let ws = workspace(
        "outbox-no-outbox",
        &server_crate(&outbox_ir(), &json!({})),
        &[("bare", bare.to_owned())],
        &[],
    );
    let mut cmd = cargo(&ws, &["build", "-p", "events-harness", "--bin", "bare"]);
    let output = cmd.output().unwrap();
    assert!(
        !output.status.success(),
        "a server without an outbox compiled"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("E0046") && stderr.contains("Outbox"),
        "{stderr}"
    );
}

/// An emitting move next to a timed edge, a deadline, a service level, an idempotency key, a flagged
/// statement and a flagged, derived event: it generates in both modes and builds with
/// `clippy -D warnings`.
#[test]
fn an_emitting_move_next_to_the_features_of_platform_p2_builds() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_text(read(FOUR));
    for (n, testing) in [false, true].into_iter().enumerate() {
        let ws = workspace(
            &format!("outbox-four-{n}"),
            &server_crate(&ir, &json!({ "testing": testing })),
            &[("noop", "fn main() {}\n".to_owned())],
            &[],
        );
        build_and_lint(&ws);
    }
}

// ---- snapshots -----------------------------------------------------------------------------------------------

fn file_list(files: &FileSet) -> String {
    files
        .paths()
        .filter(|p| !p.starts_with("src/veld/") || p.ends_with("events.rs"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn snapshot_of_the_files_of_the_outbox_contract() {
    let files = server_crate(&outbox_ir(), &json!({ "testing": true }));
    insta::assert_snapshot!("events_outbox_file_list", file_list(&files));
    insta::assert_snapshot!(
        "events_outbox_events",
        text(&files, "src/bookings/bookings_events.rs")
    );
    insta::assert_snapshot!(
        "events_outbox_server",
        text(&files, "src/bookings/bookings_server.rs")
    );
    insta::assert_snapshot!(
        "events_outbox_test_handler",
        text(&files, "src/bookings/bookings_test_server.rs")
    );
    insta::assert_snapshot!("events_outbox_runtime", text(&files, "src/veld/events.rs"));
}

#[test]
fn the_rust_server_declares_the_outbox_and_the_client_ignores_it() {
    veld_emit_conformance::check_events_outbox_is_generated(&veld_emit_rust::Rust).unwrap();
    veld_emit_conformance::check_events_outbox_is_ignored_by_clients(&veld_emit_rust::Rust)
        .unwrap();
}

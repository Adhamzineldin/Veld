//! Events in the generated Rust server (`spec/events.md` sections 6 to 9, 12 and 14,
//! `spec/targets/rust.md` section 13): the wire goldens of `tests/wire/events`, the event examples
//! of `tests/events/bookings.veld`, two services in one process, audited event rules, snapshots of
//! the generated files and the capability checks.
//!
//! The toolchain tests build real crates in one scratch workspace per contract (`cargo` and the
//! pinned `Cargo.lock` of `tests/toolchain/rust`) and run drivers from `tests/toolchain/rust/events`.
//! Rust has no reflection, so the driver of the wire goldens is written by this test from the
//! names in the IR. Without `cargo` the tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::*;
use serde_json::{Value, json};
use veld_codegen::naming::{camel, kebab, snake};
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

const BOOKINGS: &str = "tests/events/bookings.veld";
const EDGES: &str = "tests/wire/events/edges.veld";

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap()
        .replace("\r\n", "\n")
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

/// The server crate `events-server` of `services` (every service of the contract when empty).
fn server_crate(ir: &Ir, services: &[&str]) -> FileSet {
    let ids = if services.is_empty() {
        ir.services.iter().map(|s| s.id).collect()
    } else {
        services.iter().map(|n| service_id(ir, n)).collect()
    };
    run(
        ir,
        Role::Server,
        ids,
        vec![],
        None,
        serde_json::json!({ "crate-name": "events-server" }),
    )
}

/// The driver sources of this test, written below `tests/toolchain/rust/events`.
fn driver(name: &str) -> String {
    read(&format!("tests/toolchain/rust/events/{name}"))
}

/// A workspace of the generated crate `events-server` and the harness crate `events-harness`,
/// whose binaries are `bins` (name and source). `files` are written next to the workspace file.
fn events_workspace(
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
        "[package]\nname = \"events-harness\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\nevents-server = { path = \"../server\" }\nserde_json = \"1\"\n",
    )
    .unwrap();
    std::fs::write(harness.join("src/common.rs"), driver("common.rs")).unwrap();
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

/// Builds the workspace, runs the unit tests of the generated crate and clippy on it with
/// `-D warnings`.
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
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
}

/// Runs the driver `bin` and returns what it printed.
fn run_driver(ws: &std::path::Path, bin: &str) -> String {
    run_ok(
        &mut cargo(ws, &["run", "-q", "-p", "events-harness", "--bin", bin]),
        &format!("the driver {bin}"),
    )
}

fn write_json(path: &std::path::Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

// ---- the wire goldens ------------------------------------------------------------------------

fn read_json(path: &str) -> Vec<Value> {
    serde_json::from_str(&read(path)).unwrap()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect()
}

/// The hex of UTF-8 text.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A consumer the test adds to a contract: one service that subscribes to one channel and handles
/// the listed events (`on <Event> -> on<Event>`).
struct Probe {
    service: String,
    channel: String,
    events: Vec<String>,
}

impl Probe {
    fn text(&self) -> String {
        let on: Vec<String> = self
            .events
            .iter()
            .map(|e| format!("    on {e} -> on{e}"))
            .collect();
        format!(
            "\nservice {} {{\n  subscribes {} as \"{}\" {{\n{}\n  }}\n}}\n",
            self.service,
            self.channel,
            kebab(&self.service),
            on.join("\n")
        )
    }
}

/// The probe for `events` on `channel`, added to `probes` when no equal one is there; its index.
fn probe_for(probes: &mut Vec<Probe>, channel: &str, events: &[String]) -> usize {
    let at = probes
        .iter()
        .position(|p| p.channel == channel && p.events == events);
    at.unwrap_or_else(|| {
        probes.push(Probe {
            service: format!("Probe{}", probes.len()),
            channel: channel.to_owned(),
            events: events.to_vec(),
        });
        probes.len() - 1
    })
}

/// The channel with the most events among those that carry all of `events`.
fn widest_channel(ir: &Ir, events: &[String]) -> String {
    let carried = |c: &veld_ir::Channel| {
        events
            .iter()
            .all(|e| c.events.iter().any(|id| &ir.decl(*id).name == e))
    };
    ir.channels
        .iter()
        .filter(|c| carried(c))
        .max_by_key(|c| (c.events.len(), std::cmp::Reverse(c.name.clone())))
        .map(|c| c.name.clone())
        .unwrap()
}

fn channel_named<'a>(ir: &'a Ir, name: &str) -> &'a veld_ir::Channel {
    ir.channels.iter().find(|c| c.name == name).unwrap()
}

/// One contract of the goldens: the publish cases and the consume cases that name it.
struct Golden {
    contract: String,
    publish: Vec<Value>,
    consume: Vec<Value>,
}

fn golden_at(out: &mut Vec<Golden>, contract: &str) -> usize {
    out.iter()
        .position(|g| g.contract == contract)
        .unwrap_or_else(|| {
            out.push(Golden {
                contract: contract.to_owned(),
                publish: vec![],
                consume: vec![],
            });
            out.len() - 1
        })
}

fn goldens() -> Vec<Golden> {
    let mut out: Vec<Golden> = Vec::new();
    for case in read_json("tests/wire/events/envelopes.json") {
        let i = golden_at(&mut out, case["contract"].as_str().unwrap());
        out[i].publish.push(case);
    }
    for case in read_json("tests/wire/events/consume.json") {
        let i = golden_at(&mut out, case["contract"].as_str().unwrap());
        out[i].consume.push(case);
    }
    out
}

/// The contract of a golden with its probes, the cases for the driver and what to check.
struct Plan {
    ir: Ir,
    probes: Vec<Probe>,
    /// The cases of the driver: first one per publish case, then one per consume case.
    cases: Value,
    /// For each publish case: the service, the channel and the event.
    publishing: Vec<(String, String, String)>,
    /// For each consume case (the reverse ones first): the probe that subscribes.
    consuming: Vec<usize>,
}

fn plan(g: &Golden, extra: Option<Probe>) -> Plan {
    let text = read(&g.contract);
    let base = compile_text(text.clone());
    let mut probes: Vec<Probe> = Vec::new();
    let mut slots: Vec<(usize, &Value, bool)> = Vec::new();
    for case in &g.publish {
        let channel = case["channel"].as_str().unwrap().to_owned();
        let event = vec![case["event"].as_str().unwrap().to_owned()];
        slots.push((probe_for(&mut probes, &channel, &event), case, true));
    }
    for case in &g.consume {
        let handled = strings(&case["handled"]);
        let channel = widest_channel(&base, &handled);
        slots.push((probe_for(&mut probes, &channel, &handled), case, false));
    }
    probes.extend(extra);
    let full: String = std::iter::once(text)
        .chain(probes.iter().map(Probe::text))
        .collect();
    let ir = compile_text(full);
    let (mut publish, mut reverse, mut consume) = (Vec::new(), Vec::new(), Vec::new());
    let (mut publishing, mut consuming, mut real) = (Vec::new(), Vec::new(), Vec::new());
    for (i, case, is_publish) in slots {
        if is_publish {
            publish.push(json!({
                "name": case["name"], "id": case["id"], "time": case["time"], "value": case["value"],
            }));
            publishing.push((
                case["service"].as_str().unwrap().to_owned(),
                case["channel"].as_str().unwrap().to_owned(),
                case["event"].as_str().unwrap().to_owned(),
            ));
            let canonical: Value =
                serde_json::from_str(case["canonical"].as_str().unwrap()).unwrap();
            let channel = channel_named(&ir, case["channel"].as_str().unwrap());
            reverse.push(json!({
                "name": format!("reverse: {}", case["name"].as_str().unwrap()),
                "topic": channel.topic,
                "key": canonical.get("subject").cloned().unwrap_or(Value::Null),
                "message": case["canonical"],
            }));
            consuming.push(i);
        } else {
            consume.push(json!({
                "name": case["name"],
                "topic": channel_named(&ir, &probes[i].channel).topic,
                "key": Value::Null,
                "message": case["message"],
            }));
            real.push(i);
        }
    }
    reverse.extend(consume);
    consuming.extend(real);
    Plan {
        ir,
        probes,
        cases: json!({ "publish": publish, "consume": reverse }),
        publishing,
        consuming,
    }
}

/// The module and the name of the generated items of a service.
fn module_of(service: &str) -> String {
    format!("events_server::bookings::{}_events", snake(service))
}

const WIRE_HEAD: &str = r#"#[path = "../common.rs"]
mod common;

use std::future::Future;
use std::sync::{Arc, Mutex};

use common::*;
use events_server::veld::events::{ConsumerOptions, EventMeta, MemoryTransport, PublisherOptions, Transport};
use events_server::veld::{Error, VeldDecl, block_on};
use serde_json::{Value, json};

#[derive(Default)]
struct Seen {
    delivered: Mutex<Vec<Value>>,
    rejected: Mutex<Vec<Value>>,
    mismatches: Mutex<Vec<Value>>,
}

impl Seen {
    fn deliver(&self, event: &str, data: Value, meta: &EventMeta) {
        self.delivered.lock().unwrap().push(json!({
            "event": event, "data": data,
            "meta": { "id": meta.id, "source": meta.source, "type": meta.r#type,
                      "subject": meta.subject, "time": meta.time, "attempt": meta.attempt },
        }));
    }
}

"#;

const WIRE_TAIL: &str = r#"
fn main() {
    let text = std::fs::read_to_string("cases.json").unwrap();
    let cases: Value = serde_json::from_str(&text).unwrap();
    let out = block_on(async {
        let mut publish = Vec::new();
        for (i, case) in cases["publish"].as_array().unwrap().iter().enumerate() {
            publish.push(publish_case(i, case).await);
        }
        let mut consume = Vec::new();
        for (i, case) in cases["consume"].as_array().unwrap().iter().enumerate() {
            consume.push(consume_case(i, case).await);
        }
        json!({ "publish": publish, "consume": consume })
    });
    println!("{out}");
}
"#;

/// The driver of the wire goldens: Rust has no reflection, so the calls are written from the plan.
fn wire_driver(plan: &Plan) -> String {
    let mut s = String::from(WIRE_HEAD);
    for (i, p) in plan.probes.iter().enumerate() {
        s += &format!(
            "#[allow(dead_code)]\nstruct H{i}(Arc<Seen>);\n\nimpl {}::{}EventHandlers for H{i} {{\n",
            module_of(&p.service),
            p.service
        );
        for e in &p.events {
            s += &format!(
                "    fn on_{}(&self, event: events_server::bookings::t::{e}, meta: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {{\n        self.0.deliver({e:?}, event.to_wire(), &meta);\n        std::future::ready(Ok(()))\n    }}\n",
                snake(e)
            );
        }
        s += "}\n\n";
    }
    s += "async fn publish_case(i: usize, case: &Value) -> Value {\n    let rec = Recorder::new();\n    let id = case[\"id\"].as_str().unwrap().to_string();\n    let at = time_from(case[\"time\"].as_str().unwrap());\n    let options = PublisherOptions::new().new_id(move || id.clone()).clock(move || at);\n    let result: Result<(), Error> = match i {\n";
    for (i, (service, channel, event)) in plan.publishing.iter().enumerate() {
        s += &format!(
            "        {i} => {{\n            let p = {}::create_{}_publisher(rec.clone(), options);\n            let event = <events_server::bookings::t::{event} as VeldDecl>::from_wire(&case[\"value\"]).unwrap();\n            p.{}().{}(&event).await.map(drop)\n        }}\n",
            module_of(service),
            snake(service),
            snake(channel),
            snake(event)
        );
    }
    s += "        _ => panic!(\"no such case\"),\n    };\n    let sent: Vec<Value> = rec.messages().iter().map(|(t, k, b)| json!({ \"topic\": t, \"key\": k, \"hex\": hex(b) })).collect();\n    json!({ \"sent\": sent, \"error\": result.err().map(|e| e.to_string()) })\n}\n\n";
    s += "async fn consume_case(i: usize, case: &Value) -> Value {\n    let transport = Arc::new(MemoryTransport::new());\n    let seen = Arc::new(Seen::default());\n    let (r, m) = (seen.clone(), seen.clone());\n    let options = ConsumerOptions::new()\n        .on_event_rejected(move |e| r.rejected.lock().unwrap().push(json!({ \"reason\": e.reason.as_str(), \"violations\": e.violations.len(), \"hex\": hex(&e.bytes) })))\n        .on_contract_mismatch(move |e| m.mismatches.lock().unwrap().push(json!({ \"signature\": e.signature, \"ownSignature\": e.own_signature, \"digest\": e.digest, \"ownDigest\": e.own_digest })));\n    let t: Arc<dyn Transport> = transport.clone();\n    match i {\n";
    for (i, probe) in plan.consuming.iter().enumerate() {
        let p = &plan.probes[*probe];
        s += &format!(
            "        {i} => {{\n            {}::subscribe_{}_events(t.clone(), Arc::new(H{probe}(seen.clone())), options).await.unwrap();\n        }}\n",
            module_of(&p.service),
            snake(&p.service)
        );
    }
    s += "        _ => panic!(\"no such case\"),\n    }\n    t.publish(case[\"topic\"].as_str().unwrap(), case[\"key\"].as_str(), case[\"message\"].as_str().unwrap().as_bytes()).await.unwrap();\n    transport.idle().await;\n    json!({\n        \"delivered\": seen.delivered.lock().unwrap().clone(),\n        \"rejected\": seen.rejected.lock().unwrap().clone(),\n        \"mismatches\": seen.mismatches.lock().unwrap().clone(),\n        \"dead\": transport.dead_letters().len(),\n    })\n}\n";
    s + WIRE_TAIL
}

fn check_publish(plan: &Plan, g: &Golden, out: &Value) {
    let results = out["publish"].as_array().unwrap();
    assert_eq!(results.len(), g.publish.len());
    for (case, got) in g.publish.iter().zip(results) {
        let name = case["name"].as_str().unwrap();
        assert_eq!(got["error"], Value::Null, "{name}: {got}");
        let sent = got["sent"].as_array().unwrap();
        assert_eq!(sent.len(), 1, "{name}: one message");
        let want = case["canonical"].as_str().unwrap();
        assert_eq!(
            sent[0]["hex"].as_str().unwrap(),
            hex(want.as_bytes()),
            "{name}: the bytes differ from the golden\nwanted: {want}"
        );
        let channel = channel_named(&plan.ir, case["channel"].as_str().unwrap());
        assert_eq!(sent[0]["topic"], json!(channel.topic), "{name}: the topic");
        let canonical: Value = serde_json::from_str(want).unwrap();
        assert_eq!(
            sent[0]["key"],
            canonical.get("subject").cloned().unwrap_or(Value::Null),
            "{name}: the key of the transport is the subject"
        );
    }
}

fn check_reverse(g: &Golden, out: &Value) {
    let results = out["consume"].as_array().unwrap();
    for (case, got) in g.publish.iter().zip(results) {
        let name = case["name"].as_str().unwrap();
        let canonical: Value = serde_json::from_str(case["canonical"].as_str().unwrap()).unwrap();
        let delivered = got["delivered"].as_array().unwrap();
        assert_eq!(delivered.len(), 1, "{name}: delivered once: {got}");
        assert!(
            got["rejected"].as_array().unwrap().is_empty(),
            "{name}: {got}"
        );
        assert!(
            got["mismatches"].as_array().unwrap().is_empty(),
            "{name}: {got}"
        );
        let d = &delivered[0];
        assert_eq!(d["event"], case["event"], "{name}");
        assert_eq!(d["data"], canonical["data"], "{name}: the typed event");
        for attr in ["id", "source", "type", "subject", "time"] {
            assert_eq!(
                d["meta"].get(attr).cloned().unwrap_or(Value::Null),
                canonical.get(attr).cloned().unwrap_or(Value::Null),
                "{name}: meta.{attr}"
            );
        }
        assert_eq!(d["meta"]["attempt"], json!(1), "{name}");
    }
}

fn check_consume(g: &Golden, out: &Value) {
    let results = &out["consume"].as_array().unwrap()[g.publish.len()..];
    assert_eq!(results.len(), g.consume.len());
    for (case, got) in g.consume.iter().zip(results) {
        let name = case["name"].as_str().unwrap();
        let outcome = case["outcome"].as_str().unwrap();
        let count = |k: &str| got[k].as_array().unwrap().len();
        let (delivered, rejected, mismatches) =
            (count("delivered"), count("rejected"), count("mismatches"));
        let ctx = format!("{name}: {outcome}: {got}");
        assert_eq!(
            got["dead"],
            json!(0),
            "{ctx}: every message is acknowledged"
        );
        match outcome {
            "malformed" | "shape" | "rule" => {
                assert_eq!((delivered, rejected, mismatches), (0, 1, 0), "{ctx}");
                assert_eq!(got["rejected"][0]["reason"], json!(outcome), "{ctx}");
                let violations = got["rejected"][0]["violations"].as_u64().unwrap();
                assert_eq!(violations > 0, outcome == "rule", "{ctx}");
                assert_eq!(
                    got["rejected"][0]["hex"].as_str().unwrap(),
                    hex(case["message"].as_str().unwrap().as_bytes()),
                    "{ctx}: the hook gets the raw bytes"
                );
            }
            "ignored" => assert_eq!((delivered, rejected, mismatches), (0, 0, 0), "{ctx}"),
            "delivered" => assert_eq!((delivered, rejected, mismatches), (1, 0, 0), "{ctx}"),
            "delivered, rules not checked" | "delivered, other type hash" => {
                assert_eq!((delivered, rejected, mismatches), (1, 0, 1), "{ctx}");
                let m = &got["mismatches"][0];
                if outcome == "delivered, other type hash" {
                    assert_ne!(m["signature"], m["ownSignature"], "{ctx}");
                } else {
                    assert_eq!(m["signature"], m["ownSignature"], "{ctx}");
                    assert_ne!(m["digest"], m["ownDigest"], "{ctx}");
                }
            }
            other => panic!("{name}: the golden names an outcome this test does not know: {other}"),
        }
    }
}

fn golden_for<'a>(all: &'a [Golden], contract: &str) -> &'a Golden {
    all.iter().find(|g| g.contract == contract).unwrap()
}

/// The three directions of the goldens against the output of one run of `wire`: what the
/// publisher writes, what the consumer makes of that, and what it does with the consume cases.
/// The byte comparison is also the check against the Node and Java consumers: the goldens are the
/// bytes all three targets agree on.
fn check_goldens(plan: &Plan, g: &Golden, out: &Value) {
    check_publish(plan, g, out);
    check_reverse(g, out);
    check_consume(g, out);
}

// ---- the event examples ----------------------------------------------------------------------

/// The `Service.handler` names the reference evaluator says are called, sorted.
fn reference_called(ir: &Ir, delivery: &veld_rules::example::Delivery) -> Vec<String> {
    let mut called: Vec<String> = match delivery {
        veld_rules::example::Delivery::Rejected { .. } => vec![],
        veld_rules::example::Delivery::Handled { called } => called
            .iter()
            .map(|(s, h)| format!("{}.{h}", ir.service(*s).name))
            .collect(),
    };
    called.sort();
    called
}

/// The message the transport carries for a filled-in event, as the consumer's own digest says.
fn envelope_of(ir: &Ir, filled: &veld_rules::example::FilledEvent) -> (Value, Value) {
    let decl = ir.decl(filled.event);
    let channel = ir.channel(filled.channel);
    let digest = if let veld_ir::DeclKind::Event(e) = &decl.kind {
        e.rules_digest.clone()
    } else {
        None
    };
    let key = channel.key.as_ref().map(|k| {
        let v = &filled.value[&k.wire];
        v.as_str().map_or_else(|| v.to_string(), str::to_owned)
    });
    let mut message = json!({
        "specversion": "1.0", "id": "example-1", "source": "veld:bookings/Bookings",
        "type": decl.sig, "time": "2026-06-10T12:00:00Z",
        "datacontenttype": "application/json", "data": filled.value,
    });
    if let Some(d) = digest {
        message["veldrules"] = json!(d);
    }
    if let Some(k) = &key {
        message["subject"] = json!(k);
    }
    (message, json!(key))
}

fn example_cases(ir: &Ir) -> Value {
    let examples: Vec<Value> = ir
        .examples
        .iter()
        .filter(|e| e.event.is_some())
        .map(|ex| {
            let filled = veld_rules::example::fill_event_example(ir, ex).unwrap();
            let (message, key) = envelope_of(ir, &filled);
            let channel = ir.channel(filled.channel);
            let event = &ir.decl(filled.event).name;
            json!({
                "name": ex.name, "topic": channel.topic, "key": key,
                "message": message.to_string(), "value": filled.value,
                "publisher": { "channel": camel(&channel.name), "companion": event },
            })
        })
        .collect();
    json!({ "examples": examples })
}

fn check_examples(ir: &Ir, out: &[Value]) {
    let examples: Vec<&veld_ir::Example> =
        ir.examples.iter().filter(|e| e.event.is_some()).collect();
    assert_eq!(examples.len(), 11, "the 11 examples of the contract");
    assert_eq!(out.len(), 11);
    for (ex, got) in examples.iter().zip(out) {
        let name = &ex.name;
        let report = veld_rules::example::run_event_example(ir, ex).unwrap();
        assert!(
            report.passed(),
            "the reference evaluator fails \"{name}\": {report}"
        );
        let filled = veld_rules::example::fill_event_example(ir, ex).unwrap();
        let delivery =
            veld_rules::example::deliver(ir, filled.channel, filled.event, &filled.value);
        let called = json!(reference_called(ir, &delivery));
        assert_eq!(
            got["raw"], called,
            "\"{name}\": the generated consumers against the reference"
        );
        let via = &got["viaPublisher"];
        if matches!(delivery, veld_rules::example::Delivery::Rejected { .. }) {
            assert_eq!(
                via["refused"],
                json!("ValidationError"),
                "\"{name}\": the publisher refuses it"
            );
            assert_eq!(via["called"], json!([]), "\"{name}\": and nothing is sent");
        } else {
            assert_eq!(via["refused"], Value::Null, "\"{name}\": {via}");
            assert_eq!(
                via["called"], called,
                "\"{name}\": through the generated publisher"
            );
        }
    }
}

// ---- the toolchain tests ---------------------------------------------------------------------

/// `tests/events/bookings.veld` with the probes of its goldens: the wire goldens, the 11 event
/// examples and two services in one process, in one crate that also passes `clippy -D warnings`.
#[test]
fn the_bookings_crate_reproduces_the_goldens_runs_the_examples_and_two_services_talk() {
    if !cargo_or_skip() {
        return;
    }
    let all = goldens();
    let g = golden_for(&all, BOOKINGS);
    let plan = plan(g, None);
    // The examples are judged on the contract itself: the probes of the goldens are not subscribed.
    let own = compile_text(read(BOOKINGS));
    let ir = &plan.ir;
    let examples = example_cases(&own);
    let ws = events_workspace(
        "events-bookings",
        &server_crate(ir, &[]),
        &[
            ("wire", wire_driver(&plan)),
            ("process", driver("process.rs")),
            ("event_examples", driver("examples.rs")),
        ],
        &[],
    );
    write_json(&ws.join("cases.json"), &merge(&plan.cases, &examples));
    build_and_lint(&ws);
    let out: Value = serde_json::from_str(&run_driver(&ws, "wire")).unwrap();
    check_goldens(&plan, g, &out);
    let out: Vec<Value> = serde_json::from_str(&run_driver(&ws, "event_examples")).unwrap();
    check_examples(&own, &out);
    assert!(run_driver(&ws, "process").contains("events process ok"));
}

fn merge(a: &Value, b: &Value) -> Value {
    let mut out = a.clone();
    for (k, v) in b.as_object().unwrap() {
        out[k] = v.clone();
    }
    out
}

/// `tests/wire/events/edges.veld`: the wire goldens, and an audited event rule that is reported
/// on both sides and never rejects.
#[test]
fn the_edges_crate_reproduces_the_goldens_and_reports_an_audited_rule_without_rejecting() {
    if !cargo_or_skip() {
        return;
    }
    let all = goldens();
    let g = golden_for(&all, EDGES);
    let extra = Probe {
        service: "AuditProbe".into(),
        channel: "InvoiceEvents".into(),
        events: vec!["InvoiceNudged".into()],
    };
    let plan = plan(g, Some(extra));
    let ws = events_workspace(
        "events-edges",
        &server_crate(&plan.ir, &[]),
        &[("wire", wire_driver(&plan)), ("audit", driver("audit.rs"))],
        &[],
    );
    write_json(&ws.join("cases.json"), &plan.cases);
    build_and_lint(&ws);
    let out: Value = serde_json::from_str(&run_driver(&ws, "wire")).unwrap();
    check_goldens(&plan, g, &out);
    let out: Value = serde_json::from_str(&run_driver(&ws, "audit")).unwrap();
    let nudged = plan
        .ir
        .decls
        .iter()
        .find(|d| d.name == "InvoiceNudged")
        .unwrap();
    let broken = &out["breaking"];
    for side in ["published", "consumed"] {
        let heard = broken[side].as_array().unwrap();
        assert_eq!(
            heard.len(),
            1,
            "{side}: the audited rule is reported once: {broken}"
        );
        assert!(
            heard[0]["rule"]
                .as_str()
                .unwrap()
                .contains("InvoiceNudged.soon@")
        );
        assert_eq!(heard[0]["outcome"], json!("audited"), "{side}");
        assert_eq!(heard[0]["operation"], json!(nudged.sig), "{side}");
    }
    assert_eq!(broken["published"][0]["position"], json!("output"));
    assert_eq!(broken["consumed"][0]["position"], json!("input"));
    // It never rejects: the message is sent without a digest and delivered.
    assert_eq!(broken["delivered"], json!([45]));
    assert_eq!(broken["rejected"], json!([]));
    let sent = broken["sent"].as_array().unwrap();
    assert_eq!(sent.len(), 1);
    assert!(!sent[0].as_str().unwrap().contains("veldrules"));
    let fine = &out["fine"];
    assert_eq!(fine["published"], json!([]));
    assert_eq!(fine["consumed"], json!([]));
    assert_eq!(fine["delivered"], json!([10]));
}

/// Events and no rule at all: the crate has no rules runtime, a publisher-only service and a
/// subscriber-only service build with `clippy -D warnings` and talk.
#[test]
fn events_without_any_rule_compile_run_and_have_no_rules_runtime() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_text(PLAIN.to_owned());
    let files = server_crate(&ir, &[]);
    assert!(files.get("src/veld/rules/mod.rs").is_none());
    let ws = events_workspace(
        "events-plain",
        &files,
        &[("plain", driver("plain.rs"))],
        &[],
    );
    build_and_lint(&ws);
    assert!(run_driver(&ws, "plain").contains("events plain ok"));
}

// ---- the files: snapshots, no events no files ------------------------------------------------

const SERVICES: [&str; 3] = ["Bookings", "Notifications", "Ledger"];

fn bookings_files() -> FileSet {
    server_crate(&compile_text(read(BOOKINGS)), &SERVICES)
}

fn file_list(files: &FileSet) -> String {
    files.paths().collect::<Vec<_>>().join("\n")
}

/// Events and no rule at all.
const PLAIN: &str = "event Pinged {
  when: datetime
}

channel Pings {
  carries Pinged
}

service Pinger {
  publishes Pings
}

service Listener {
  subscribes Pings as \"l\" {
    on Pinged -> heard
  }
}
";

#[test]
fn snapshot_of_the_files_of_a_server_with_events() {
    insta::assert_snapshot!("events_file_list", file_list(&bookings_files()));
}

#[test]
fn snapshot_of_the_publisher_module() {
    insta::assert_snapshot!(
        "events_publisher",
        text(&bookings_files(), "src/bookings/bookings_events.rs")
    );
}

#[test]
fn snapshot_of_the_subscriber_module() {
    insta::assert_snapshot!(
        "events_subscriber",
        text(&bookings_files(), "src/bookings/ledger_events.rs")
    );
}

#[test]
fn snapshot_of_the_event_types() {
    insta::assert_snapshot!("events_types", text(&bookings_files(), "src/bookings/t.rs"));
}

#[test]
fn snapshot_of_the_events_runtime() {
    insta::assert_snapshot!(
        "events_runtime",
        text(&bookings_files(), "src/veld/events.rs")
    );
}

#[test]
fn snapshot_of_the_events_runtime_of_a_contract_without_rules() {
    let files = server_crate(&compile_text(PLAIN.to_owned()), &[]);
    insta::assert_snapshot!(
        "events_runtime_without_rules",
        text(&files, "src/veld/events.rs")
    );
}

#[test]
fn a_service_that_only_publishes_or_subscribes_has_no_server_module_and_no_framework() {
    let files = server_crate(&compile_text(PLAIN.to_owned()), &[]);
    for gone in [
        "src/bookings/pinger_server.rs",
        "src/bookings/listener_server.rs",
        "src/adapters/axum.rs",
        "src/veld/server.rs",
    ] {
        assert!(files.get(gone).is_none(), "{gone}");
    }
    assert!(files.get("src/bookings/pinger_events.rs").is_some());
    assert!(files.get("src/bookings/listener_events.rs").is_some());
    let cargo = text(&files, "Cargo.toml");
    assert!(
        !cargo.contains("axum") && !cargo.contains("tokio"),
        "{cargo}"
    );
    let lib = text(&files, "src/lib.rs");
    assert!(!lib.contains("adapters"), "{lib}");
    assert!(text(&files, "src/bookings/mod.rs").contains("pub mod pinger_events;"));
    let bookings = bookings_files();
    assert!(
        bookings.get("src/bookings/bookings_server.rs").is_some(),
        "it has endpoints"
    );
    assert!(
        bookings
            .get("src/bookings/notifications_server.rs")
            .is_none()
    );
    assert!(bookings.get("src/bookings/ledger_server.rs").is_none());
}

#[test]
fn a_contract_without_events_gets_no_event_files() {
    let files = gen_server(&standard_ir(), "Orders", &["Iam"], "orders-server");
    for (path, bytes) in files.iter() {
        assert!(!path.contains("events"), "{path}");
        let body = String::from_utf8_lossy(bytes);
        assert!(!body.contains("veld::events"), "{path}");
    }
}

#[test]
fn an_event_that_a_client_cannot_see_is_in_the_server_and_not_in_the_client() {
    let ir = compile_text(read(BOOKINGS));
    let client = gen_client(&ir, "events-client", None);
    for path in client.paths() {
        assert!(!path.contains("events"), "the client has {path}");
    }
}

#[test]
fn the_rust_server_declares_events() {
    use veld_codegen::DynGenerator;
    assert!(
        veld_emit_rust::Rust
            .capabilities(Role::Server)
            .supports(veld_ir::Feature::Events)
    );
    veld_emit_conformance::check_events_are_generated(&veld_emit_rust::Rust).unwrap();
}

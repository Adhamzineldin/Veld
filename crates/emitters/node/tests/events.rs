//! Events in the generated Node server (`spec/events.md` sections 6 to 9, 12 and 14,
//! `spec/targets/typescript.md` section 5f): the wire goldens of `tests/wire/events`, the event
//! examples of `tests/events/bookings.veld`, two services in one process, audited event rules,
//! snapshots of the generated files and the capability checks.
//!
//! The toolchain tests need `node` and `tests/toolchain/ts/node_modules`; without them they are
//! skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use common::toolchain::{run, scratch, toolchain_dir, tsc_path};
use common::*;
use serde_json::{Value, json};
use veld_codegen::naming::{camel, kebab, pascal};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_project::output::{WriteOptions, write};

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap()
        .replace("\r\n", "\n")
}

/// Compiles `text` as the package `bookings` from the file `t.veld`: the goldens were made so, and the
/// module name is part of the signatures.
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
        "the contract failed to compile:
{}",
        out.render()
    );
    out.ir.unwrap()
}

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

// ---- building and running generated packages -------------------------------------------------

fn matrix(body: fn(&Path, &str, &str)) {
    let Some(tsc) = tsc_path() else {
        return;
    };
    for (edition, target) in TARGETS {
        body(&tsc, edition, target);
    }
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

fn tsc(tsc_path: &Path, work: &Path, project: &str) {
    run(
        Command::new("node")
            .arg(tsc_path)
            .args(["-p", project])
            .current_dir(work),
        &format!("tsc -p {project}"),
    );
}

/// Writes the server of `services` into `work/server`, compiles it alone with `tsc --strict` and no
/// types packages, copies `drivers` and compiles them with the Node types.
fn build(
    work: &Path,
    tsc_path: &Path,
    ir: &Ir,
    services: &[&str],
    drivers: &[&str],
    edition: &str,
) {
    build_with(
        work,
        tsc_path,
        ir,
        services,
        drivers,
        edition,
        &typescript_option(edition),
    );
}

/// [`build`] with the target options `options` (`testing`, `messaging`).
fn build_with(
    work: &Path,
    tsc_path: &Path,
    ir: &Ir,
    services: &[&str],
    drivers: &[&str],
    edition: &str,
    options: &Value,
) {
    let target = TARGETS
        .iter()
        .find(|(e, _)| *e == edition)
        .map_or("ES2020", |(_, t)| *t);
    let files = gen_server_services(ir, services, options);
    write(&work.join("server"), &files, &WriteOptions::default()).unwrap();
    write_json(
        &work.join("tsconfig.generated.json"),
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target], "types": [],
                "module": "commonjs", "moduleResolution": "node10", "noEmit": true,
                "skipLibCheck": false, "forceConsistentCasingInFileNames": true
            },
            "include": ["server/**/*.ts"]
        }),
    );
    tsc(tsc_path, work, "tsconfig.generated.json");
    for driver in drivers {
        std::fs::copy(toolchain_dir().join(driver), work.join(driver)).unwrap();
    }
    write_json(
        &work.join("tsconfig.json"),
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target, "dom"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "server/**/*.ts"]
        }),
    );
    tsc(tsc_path, work, "tsconfig.json");
}

/// Runs `node out/<script>.js` in `work` and returns its standard output.
fn node(work: &Path, script: &str) -> String {
    let out = Command::new("node")
        .arg(format!("out/{script}.js"))
        .current_dir(work)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "node {script}.js failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// ---- the wire goldens ------------------------------------------------------------------------

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

/// The hex of UTF-8 text.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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

/// The consume case that hands a published message back to a consumer of the same event.
fn reverse(case: &Value, canonical: &Value, channel: &str, probe: &Probe, ir: &Ir) -> Value {
    let key = canonical.get("subject").cloned().unwrap_or(Value::Null);
    json!({
        "name": format!("reverse: {}", case["name"].as_str().unwrap()),
        "module": format!("server/server/{}-events", kebab(&probe.service)),
        "subscribe": format!("subscribe{}Events", pascal(&probe.service)),
        "topic": channel_named(ir, channel).topic,
        "key": key,
        "message": case["canonical"],
        "handlers": probe.events.iter()
            .map(|e| json!({ "method": format!("on{e}"), "event": e }))
            .collect::<Vec<_>>(),
    })
}

fn consume_case(case: &Value, probe: &Probe, ir: &Ir) -> Value {
    json!({
        "name": case["name"],
        "module": format!("server/server/{}-events", kebab(&probe.service)),
        "subscribe": format!("subscribe{}Events", pascal(&probe.service)),
        "topic": channel_named(ir, &probe.channel).topic,
        "key": Value::Null,
        "message": case["message"],
        "handlers": probe.events.iter()
            .map(|e| json!({ "method": format!("on{e}"), "event": e }))
            .collect::<Vec<_>>(),
    })
}

fn publish_case(case: &Value) -> Value {
    let service = case["service"].as_str().unwrap();
    let event = case["event"].as_str().unwrap();
    json!({
        "name": case["name"],
        "module": format!("server/server/{}-events", kebab(service)),
        "factory": format!("create{}Publisher", pascal(service)),
        "channel": camel(case["channel"].as_str().unwrap()),
        "method": camel(event),
        "companion": event,
        "id": case["id"],
        "time": case["time"],
        "value": case["value"],
    })
}

/// The contract of a golden with its probes, the cases for the driver and what to check.
struct Plan {
    ir: Ir,
    cases: Value,
    services: Vec<String>,
}

fn plan(g: &Golden) -> Plan {
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
    let full: String = std::iter::once(text)
        .chain(probes.iter().map(Probe::text))
        .collect();
    let ir = compile_text(full);
    let (mut publish, mut consume) = (Vec::new(), Vec::new());
    for (i, case, is_publish) in slots {
        let probe = &probes[i];
        if is_publish {
            publish.push(publish_case(case));
            let canonical: Value =
                serde_json::from_str(case["canonical"].as_str().unwrap()).unwrap();
            let channel = case["channel"].as_str().unwrap();
            consume.push(reverse(case, &canonical, channel, probe, &ir));
        } else {
            consume.push(consume_case(case, probe, &ir));
        }
    }
    let services = ir.services.iter().map(|s| s.name.clone()).collect();
    Plan {
        ir,
        cases: json!({ "publish": publish, "consume": consume }),
        services,
    }
}

fn check_publish(plan: &Plan, g: &Golden, out: &Value) {
    let results = out["publish"].as_array().unwrap();
    assert_eq!(results.len(), g.publish.len());
    for (case, got) in g.publish.iter().zip(results) {
        let name = case["name"].as_str().unwrap();
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

fn wire_goldens(tsc_path: &Path, edition: &str, _target: &str) {
    for g in goldens() {
        let plan = plan(&g);
        let guard = scratch("events-wire-");
        let work = guard.path();
        let names: Vec<&str> = plan.services.iter().map(String::as_str).collect();
        build(
            work,
            tsc_path,
            &plan.ir,
            &names,
            &["events_wire.ts"],
            edition,
        );
        write_json(&work.join("cases.json"), &plan.cases);
        let out: Value = serde_json::from_str(&node(work, "events_wire")).unwrap();
        check_publish(&plan, &g, &out);
        check_reverse(&g, &out);
        check_consume(&g, &out);
    }
}

#[test]
fn the_generated_publisher_and_subscriber_reproduce_the_wire_goldens() {
    matrix(wire_goldens);
}

// ---- the event examples ----------------------------------------------------------------------

const BOOKINGS: &str = "tests/events/bookings.veld";
const EDGES: &str = "tests/wire/events/edges.veld";

fn module_of(service: &str) -> String {
    format!("server/server/{}-events", kebab(service))
}

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
    let services: Vec<Value> = ir
        .services
        .iter()
        .filter(|s| !s.subscriptions.is_empty())
        .map(|s| {
            let handlers: Vec<&str> = s
                .subscriptions
                .iter()
                .flat_map(|sub| sub.handlers.iter().map(|h| h.name.as_str()))
                .collect();
            json!({
                "name": s.name, "module": module_of(&s.name),
                "subscribe": format!("subscribe{}Events", pascal(&s.name)), "handlers": handlers,
            })
        })
        .collect();
    let examples: Vec<Value> = ir
        .examples
        .iter()
        .filter(|e| e.event.is_some())
        .map(|ex| {
            let filled = veld_rules::example::fill_event_example(ir, ex).unwrap();
            let (message, key) = envelope_of(ir, &filled);
            let channel = ir.channel(filled.channel);
            let publisher = ir
                .services
                .iter()
                .find(|s| s.publishes.iter().any(|p| ir.channel(p.channel) == channel))
                .unwrap();
            let event = &ir.decl(filled.event).name;
            json!({
                "name": ex.name, "topic": channel.topic, "key": key,
                "message": message.to_string(), "value": filled.value,
                "publisher": {
                    "module": module_of(&publisher.name),
                    "factory": format!("create{}Publisher", pascal(&publisher.name)),
                    "channel": camel(&channel.name), "method": camel(event), "companion": event,
                },
            })
        })
        .collect();
    json!({ "services": services, "examples": examples })
}

fn examples_against_the_generated_code(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(read(BOOKINGS));
    let guard = scratch("events-examples-");
    let work = guard.path();
    let names: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
    build(
        work,
        tsc_path,
        &ir,
        &names,
        &["events_examples.ts"],
        edition,
    );
    let cases = example_cases(&ir);
    write_json(&work.join("cases.json"), &cases);
    let out: Vec<Value> = serde_json::from_str(&node(work, "events_examples")).unwrap();
    let examples: Vec<&veld_ir::Example> =
        ir.examples.iter().filter(|e| e.event.is_some()).collect();
    assert_eq!(examples.len(), 11, "the 11 examples of the contract");
    assert_eq!(out.len(), 11);
    for (ex, got) in examples.iter().zip(&out) {
        let name = &ex.name;
        let report = veld_rules::example::run_event_example(&ir, ex).unwrap();
        assert!(
            report.passed(),
            "the reference evaluator fails \"{name}\": {report}"
        );
        let filled = veld_rules::example::fill_event_example(&ir, ex).unwrap();
        let delivery =
            veld_rules::example::deliver(&ir, filled.channel, filled.event, &filled.value);
        let called = json!(reference_called(&ir, &delivery));
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

#[test]
fn the_event_examples_pass_against_the_generated_publisher_and_subscribers() {
    matrix(examples_against_the_generated_code);
}

#[test]
fn the_edges_contract_has_no_event_examples_to_run() {
    let ir = compile_text(read(EDGES));
    assert!(ir.examples.iter().all(|e| e.event.is_none()));
}

// ---- two services in one process -------------------------------------------------------------

fn process(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(read(BOOKINGS));
    let guard = scratch("events-process-");
    let work = guard.path();
    let names: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
    build(work, tsc_path, &ir, &names, &["events_process.ts"], edition);
    assert!(node(work, "events_process").contains("events process ok"));
}

#[test]
fn two_generated_services_talk_through_the_memory_transport_in_one_process() {
    matrix(process);
}

// ---- audited event rules ---------------------------------------------------------------------

fn audited(tsc_path: &Path, edition: &str, _target: &str) {
    let probe = Probe {
        service: "Probe0".into(),
        channel: "InvoiceEvents".into(),
        events: vec!["InvoiceNudged".into()],
    };
    let ir = compile_text(format!("{}{}", read(EDGES), probe.text()));
    let guard = scratch("events-audit-");
    let work = guard.path();
    build(
        work,
        tsc_path,
        &ir,
        &["Billing", "Probe0"],
        &["events_audit.ts"],
        edition,
    );
    let out: Value = serde_json::from_str(&node(work, "events_audit")).unwrap();
    let nudged = ir.decls.iter().find(|d| d.name == "InvoiceNudged").unwrap();
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

#[test]
fn an_audited_rule_of_an_event_is_reported_on_both_sides_and_never_rejects() {
    matrix(audited);
}

// ---- the files: snapshots, the same bytes for every adapter, no events no files ----------------

const SERVICES: [&str; 3] = ["Bookings", "Notifications", "Ledger"];

fn bookings_files(framework: &str) -> veld_codegen::FileSet {
    let ir = compile_text(read(BOOKINGS));
    gen_server_services_fw(&ir, &SERVICES, &serde_json::Value::Null, Some(framework))
}

fn file_list(files: &veld_codegen::FileSet) -> String {
    files.paths().collect::<Vec<_>>().join("\n")
}

#[test]
fn snapshot_of_the_files_of_a_server_with_events_on_a_plain_adapter() {
    insta::assert_snapshot!(
        "events_file_list_express",
        file_list(&bookings_files("express"))
    );
}

#[test]
fn snapshot_of_the_files_of_a_server_with_events_on_nest() {
    insta::assert_snapshot!("events_file_list_nest", file_list(&bookings_files("nest")));
}

#[test]
fn snapshot_of_the_publisher_module() {
    insta::assert_snapshot!(
        "events_publisher",
        text(&bookings_files("express"), "server/bookings-events.ts")
    );
}

#[test]
fn snapshot_of_the_subscriber_module() {
    insta::assert_snapshot!(
        "events_subscriber",
        text(&bookings_files("express"), "server/ledger-events.ts")
    );
}

#[test]
fn snapshot_of_the_event_types() {
    insta::assert_snapshot!(
        "events_types",
        text(&bookings_files("express"), "events/t.ts")
    );
}

#[test]
fn snapshot_of_the_events_runtime() {
    insta::assert_snapshot!(
        "events_runtime",
        text(&bookings_files("express"), "runtime/events.ts")
    );
}

/// Events and no rule at all: the runtime keeps neither the `rules` nor the `rollout` regions.
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
fn snapshot_of_the_events_runtime_of_a_contract_without_rules() {
    let ir = compile_text(PLAIN.to_owned());
    let files =
        gen_server_services_fw(&ir, &["Pinger", "Listener"], &serde_json::Value::Null, None);
    insta::assert_snapshot!(
        "events_runtime_without_rules",
        text(&files, "runtime/events.ts")
    );
    assert!(files.get("runtime/rules.ts").is_none());
}

fn plain(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(PLAIN.to_owned());
    let guard = scratch("events-plain-");
    let work = guard.path();
    build(
        work,
        tsc_path,
        &ir,
        &["Pinger", "Listener"],
        &["events_plain.ts"],
        edition,
    );
    assert!(!work.join("server/runtime/rules.ts").exists());
    assert!(node(work, "events_plain").contains("events plain ok"));
}

fn prelude_error(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = veld_emit_conformance::compiled_events_contract();
    let guard = scratch("events-prelude-");
    let work = guard.path();
    build(
        work,
        tsc_path,
        &ir,
        &["Hello", "Audit"],
        &["events_prelude.ts"],
        edition,
    );
    assert!(node(work, "events_prelude").contains("events prelude ok"));
}

#[test]
fn a_service_with_a_prelude_validation_error_throws_that_class() {
    matrix(prelude_error);
}

#[test]
fn events_without_any_rule_compile_and_run() {
    matrix(plain);
}

#[test]
fn the_event_files_are_the_same_for_every_adapter() {
    let reference = bookings_files("express");
    let event_file = |p: &str| {
        p == "runtime/events.ts"
            || p == "runtime/veld.ts"
            || p == "runtime/rules.ts"
            || p.starts_with("events/")
            || p.starts_with("models/")
            || p.ends_with("-events.ts")
    };
    for fw in ["fastify", "hono", "node-http", "nest"] {
        let files = bookings_files(fw);
        for path in reference.paths().filter(|p| event_file(p)) {
            assert_eq!(
                files.get(path),
                reference.get(path),
                "{path} differs with the adapter `{fw}`"
            );
        }
        assert!(
            files.paths().filter(|p| event_file(p)).count() >= 7,
            "{fw}: the event files are there"
        );
    }
}

#[test]
fn a_service_that_only_publishes_or_subscribes_has_no_http_module() {
    for fw in ["express", "nest"] {
        let files = bookings_files(fw);
        for gone in ["server/notifications.ts", "server/ledger.ts"] {
            assert!(files.get(gone).is_none(), "{fw}: {gone}");
        }
        assert!(
            files.get("server/bookings.ts").is_some(),
            "{fw}: it has endpoints"
        );
        assert!(files.get("server/notifications-events.ts").is_some());
        let barrel = text(&files, "index.ts");
        assert!(!barrel.contains("'./server/notifications'"), "{barrel}");
        assert!(barrel.contains("./server/notifications-events"));
        assert!(barrel.contains("export * as veldEvents from './runtime/events';"));
        let readme = text(&files, "README.md");
        assert!(readme.contains("createBookingsHandler"));
        assert!(!readme.contains("createNotificationsHandler"));
    }
    let nest = bookings_files("nest");
    assert!(nest.get("server/nest/bookings.ts").is_some());
    assert!(nest.get("server/nest/notifications.ts").is_none());
}

#[test]
fn a_contract_without_events_gets_no_event_files() {
    let files = gen_server(&standard_ir(), "Orders", &["Iam"]);
    for (path, bytes) in files.iter() {
        assert!(!path.contains("events"), "{path}");
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains("veldEvents"), "{path}");
    }
    assert!(files.paths().all(|p| !p.starts_with("events/")));
}

#[test]
fn an_event_that_a_client_cannot_see_is_in_the_server_and_not_in_the_client() {
    let ir = compile_text(read(BOOKINGS));
    let client = gen_client(&ir);
    for path in client.paths() {
        assert!(!path.contains("events"), "the client has {path}");
    }
    assert!(!text(&client, "index.ts").contains("veldEvents"));
}

// ---- capabilities ----------------------------------------------------------------------------

#[test]
fn the_node_server_declares_events_for_every_adapter_and_generates_them() {
    use veld_codegen::{DynGenerator, Role};
    assert!(
        veld_emit_node::Node
            .capabilities(Role::Server)
            .supports(veld_ir::Feature::Events)
    );
    veld_emit_conformance::check_events_are_generated(&veld_emit_node::Node).unwrap();
    for adapter in veld_emit_node::ADAPTERS {
        let info = veld_codegen::framework_info(*adapter);
        let ir = compile_text(read(BOOKINGS));
        let files =
            gen_server_services_fw(&ir, &SERVICES, &serde_json::Value::Null, Some(&info.id));
        assert!(files.get("runtime/events.ts").is_some(), "{}", info.id);
    }
}

// ---- phase E2: the dead-letter and retry goldens ---------------------------------------------

const DELIVERY: &str = "tests/events/delivery.veld";

/// A message the retry cases deliver: the first publish golden of `BookingPaid`, as it is, with
/// a number where step 3 wants a decimal, with a zero where step 4 wants a positive amount, with
/// a `type` no `on` line names, and as text that is not JSON.
fn retry_messages() -> Value {
    let cases = read_json("tests/wire/events/envelopes.json");
    let valid = cases[0]["canonical"].as_str().unwrap().to_owned();
    let with = |change: &dyn Fn(&mut Value)| {
        let mut m: Value = serde_json::from_str(&valid).unwrap();
        change(&mut m);
        m.to_string()
    };
    json!({
        "valid": valid,
        "shape": with(&|m| m["data"]["amount"] = json!(120)),
        "rule": with(&|m| m["data"]["amount"] = json!("0.00")),
        "ignored": with(&|m| m["type"] = json!("veld:bookings/SomethingElse@0000000000000000")),
        "malformed": "this is not json",
    })
}

fn delivery_goldens(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(read(BOOKINGS));
    let paid = ir.decls.iter().find(|d| d.name == "BookingPaid").unwrap();
    let digest = if let veld_ir::DeclKind::Event(e) = &paid.kind {
        e.rules_digest.clone().unwrap()
    } else {
        panic!("BookingPaid is not an event")
    };
    let guard = scratch("events-delivery-wire-");
    let work = guard.path();
    let names: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
    build(
        work,
        tsc_path,
        &ir,
        &names,
        &["events_delivery_wire.ts"],
        edition,
    );
    let dead = read_json("tests/wire/events/dead-letter.json");
    let retry = read_json("tests/wire/events/retry.json");
    write_json(
        &work.join("cases.json"),
        &json!({
            "digest": digest,
            "topic": channel_named(&ir, "BookingEvents").topic,
            "group": "ledger",
            "messages": retry_messages(),
            "deadLetter": dead,
            "retry": retry,
        }),
    );
    let out: Value = serde_json::from_str(&node(work, "events_delivery_wire")).unwrap();
    check_dead_letter(&dead, &out["deadLetter"]);
    check_retry(&retry, &out["retry"]);
}

/// The bytes a golden case names: `x` as text, or `xHex` as hex.
fn golden_hex(case: &Value, text: &str, hex_name: &str) -> String {
    match case.get(hex_name).and_then(Value::as_str) {
        Some(h) => h.to_owned(),
        None => hex(case[text].as_str().unwrap().as_bytes()),
    }
}

fn check_dead_letter(cases: &[Value], got: &Value) {
    let got = got.as_array().unwrap();
    assert_eq!(got.len(), cases.len());
    for (case, got) in cases.iter().zip(got) {
        let name = case["name"].as_str().unwrap();
        assert_eq!(got["name"], case["name"]);
        assert_eq!(
            got["hex"].as_str().unwrap(),
            golden_hex(case, "copy", "copyHex"),
            "{name}: the copy differs from the golden"
        );
    }
}

fn check_retry(cases: &[Value], got: &Value) {
    let got = got.as_array().unwrap();
    assert_eq!(got.len(), cases.len());
    for (case, got) in cases.iter().zip(got) {
        let name = case["name"].as_str().unwrap();
        let want = &case["expect"];
        assert_eq!(got["runs"], want["runs"], "{name}: runs");
        assert_eq!(got["failures"], want["failures"], "{name}: failures");
        assert_eq!(got["waitsMs"], want["waitsMs"], "{name}: waits");
        assert_eq!(got["answer"], want["answer"], "{name}: answer");
        assert_eq!(
            got["answerIfCopyFails"], want["answerIfCopyFails"],
            "{name}: answer when the copy cannot be published"
        );
        match want["copy"].as_str() {
            None => assert_eq!(got["copy"], Value::Null, "{name}: no copy"),
            Some(reason) => {
                assert_eq!(got["copy"]["reason"], json!(reason), "{name}: copy reason");
                assert_eq!(
                    got["copy"]["attempts"], want["copyAttempts"],
                    "{name}: veldattempts"
                );
                assert_eq!(
                    got["copy"]["topic"],
                    json!("shop.bookings.v1.dlq"),
                    "{name}: the copy goes to <topic>.dlq"
                );
                assert_eq!(got["copy"]["key"], json!("k1"), "{name}: the original key");
            }
        }
    }
}

#[test]
fn the_generated_consumer_reproduces_the_dead_letter_and_retry_goldens() {
    matrix(delivery_goldens);
}

#[test]
fn every_retry_case_of_the_golden_is_run_and_the_driver_covers_every_kind_of_case() {
    // A golden case the driver cannot run (a new kind of case) must fail here, not silently pass.
    for case in read_json("tests/wire/events/retry.json") {
        let keys: Vec<&str> = case
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for key in keys {
            assert!(
                [
                    "name",
                    "retry",
                    "deadLetter",
                    "delayMs",
                    "maxDelayMs",
                    "transportAttempt",
                    "results",
                    "rejected",
                    "ignored",
                    "expect"
                ]
                .contains(&key),
                "retry.json has a member the driver does not read: {key}"
            );
        }
    }
}

// ---- phase E2: two services in one process ---------------------------------------------------

fn delivery_process(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(read(DELIVERY));
    let guard = scratch("events-delivery-process-");
    let work = guard.path();
    let names: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
    build(
        work,
        tsc_path,
        &ir,
        &names,
        &["events_delivery_process.ts"],
        edition,
    );
    assert!(node(work, "events_delivery_process").contains("events delivery process ok"));
}

#[test]
fn retry_dead_letter_and_the_inbox_work_between_two_generated_services_in_one_process() {
    matrix(delivery_process);
}

// ---- phase E2: the event examples through the generated test handlers -------------------------

/// A running `events_serve` driver; killed when dropped.
struct Served {
    tree: veld_test_support::ProcessTree,
    url: String,
}

impl Served {
    /// `handlers` are `<module>#<factory>`: the test handlers the one server mounts.
    fn start(work: &Path, handlers: &[String]) -> Served {
        use std::io::{BufRead, BufReader};
        use std::process::Stdio;
        let mut cmd = Command::new("node");
        cmd.arg("out/events_serve.js")
            .args(handlers)
            .current_dir(work)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut tree =
            veld_test_support::ProcessTree::spawn(&mut cmd).expect("cannot start the handler");
        let mut line = String::new();
        BufReader::new(tree.child_mut().stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port = line
            .trim()
            .strip_prefix("PORT=")
            .unwrap_or_else(|| panic!("the handler did not report a port: {line:?}"));
        Served {
            url: format!("http://127.0.0.1:{port}"),
            tree,
        }
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        self.tree.kill_tree();
    }
}

/// Every event example of `path` through the generated test handlers of every service that
/// subscribes, mounted in one server (`events_serve.ts`), with the shared live runner. Every
/// example must pass: none may fail, and none may be skipped.
///
/// The handlers are mounted together because the runner compares the outcome of the whole
/// message with the reference's, which rejects a message at the first subscription that rejects it:
/// a service that has no `on` line for the event ignores it at step 2 of section 8, so alone it
/// answers `ignored` where the reference says `rejected` (the example "an optional field that is
/// present is checked" of `bookings.veld`, for `Ledger`).
fn live_event_examples(tsc_path: &Path, edition: &str, path: &str) -> usize {
    let ir = compile_text(read(path));
    let guard = scratch("events-live-");
    let work = guard.path();
    let names: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
    build_with(
        work,
        tsc_path,
        &ir,
        &names,
        &["events_serve.ts"],
        edition,
        &json!({ "typescript": edition, "testing": true }),
    );
    let handlers: Vec<String> = ir
        .services
        .iter()
        .filter(|s| !s.subscriptions.is_empty())
        .map(|s| {
            format!(
                "./server/server/{}-test-handler#create{}TestHandler",
                kebab(&s.name),
                pascal(&s.name)
            )
        })
        .collect();
    assert!(!handlers.is_empty(), "{path}: no service subscribes");
    let served = Served::start(work, &handlers);
    let live = veld_testing::Live::connect(&served.url, true).unwrap();
    let mut run = 0;
    for ex in ir.examples.iter().filter(|e| e.event.is_some()) {
        let report = live.run_example(&ir, ex).unwrap();
        assert!(
            report.passed(),
            "{path}: \"{}\" did not pass live: {report}",
            ex.name
        );
        run += 1;
    }
    run
}

fn live_examples_of_both_contracts(tsc_path: &Path, edition: &str, _target: &str) {
    assert_eq!(live_event_examples(tsc_path, edition, DELIVERY), 4);
    assert_eq!(live_event_examples(tsc_path, edition, BOOKINGS), 11);
}

#[test]
fn every_event_example_passes_live_against_the_generated_test_handlers() {
    matrix(live_examples_of_both_contracts);
}

/// The event endpoint answers `400` with `{ "error": ... }` for a missing or empty topic and for a
/// number of deliveries that is neither 1 nor 2.
fn test_handler_errors(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(read(DELIVERY));
    let guard = scratch("events-test-handler-errors-");
    let work = guard.path();
    let names: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
    build_with(
        work,
        tsc_path,
        &ir,
        &names,
        &["events_test_handler_errors.ts"],
        edition,
        &json!({ "typescript": edition, "testing": true }),
    );
    assert!(node(work, "events_test_handler_errors").contains("events test handler errors ok"));
}

#[test]
fn the_event_endpoint_of_a_test_handler_answers_400_with_an_error_object() {
    matrix(test_handler_errors);
}

// ---- phase E2: the generated files -----------------------------------------------------------

fn delivery_files(options: &Value) -> veld_codegen::FileSet {
    let ir = compile_text(read(DELIVERY));
    gen_server_services_fw(
        &ir,
        &["Orders", "Fulfilment", "Billing"],
        options,
        Some("express"),
    )
}

#[test]
fn snapshot_of_the_event_files_of_a_contract_with_all_three_annotations() {
    let files = delivery_files(&json!({ "testing": true }));
    insta::assert_snapshot!("events_delivery_file_list", file_list(&files));
    insta::assert_snapshot!(
        "events_delivery_publisher",
        text(&files, "server/orders-events.ts")
    );
    insta::assert_snapshot!(
        "events_delivery_subscriber",
        text(&files, "server/fulfilment-events.ts")
    );
    insta::assert_snapshot!(
        "events_delivery_subscriber_billing",
        text(&files, "server/billing-events.ts")
    );
    insta::assert_snapshot!(
        "events_delivery_test_handler",
        text(&files, "server/fulfilment-test-handler.ts")
    );
    insta::assert_snapshot!("events_delivery_runtime", text(&files, "runtime/events.ts"));
}

#[test]
fn snapshot_of_the_broker_adapters_and_the_manifest_they_change() {
    let files = delivery_files(&json!({ "messaging": ["rabbitmq", "kafka"] }));
    insta::assert_snapshot!("events_kafka_adapter", text(&files, "server/kafka.ts"));
    insta::assert_snapshot!(
        "events_rabbitmq_adapter",
        text(&files, "server/rabbitmq.ts")
    );
    insta::assert_snapshot!("events_adapters_package_json", text(&files, "package.json"));
    insta::assert_snapshot!("events_adapters_file_list", file_list(&files));
}

#[test]
fn the_annotations_of_a_channel_are_in_its_files_and_only_in_the_files_of_channels_that_have_them()
{
    let delivery = delivery_files(&json!({}));
    let publisher = text(&delivery, "server/orders-events.ts");
    assert!(
        publisher.contains("ordered: true, deadLetter: true }"),
        "{publisher}"
    );
    let subscriber = text(&delivery, "server/fulfilment-events.ts");
    assert!(subscriber.contains("retry: { max: 3, backoff: 'exponential' },"));
    assert!(subscriber.contains("deadLetter: true,") && subscriber.contains("ordered: true,"));
    // The bookings channels have none of the three: their files do not mention them.
    let bookings = bookings_files("express");
    for path in bookings.paths().filter(|p| p.ends_with("-events.ts")) {
        let source = text(&bookings, path);
        for word in ["retry:", "deadLetter", "ordered"] {
            assert!(!source.contains(word), "{path} mentions {word}");
        }
    }
}

#[test]
fn the_event_files_are_the_same_for_every_adapter_with_the_annotations_and_the_brokers() {
    let options = json!({ "testing": true, "messaging": ["kafka", "rabbitmq"] });
    let ir = compile_text(read(DELIVERY));
    let names = ["Orders", "Fulfilment", "Billing"];
    let reference = gen_server_services_fw(&ir, &names, &options, Some("express"));
    for fw in ["fastify", "hono", "node-http", "nest"] {
        let files = gen_server_services_fw(&ir, &names, &options, Some(fw));
        for path in reference.paths().filter(|p| {
            p.starts_with("runtime/")
                || p.starts_with("events/")
                || p.ends_with("-events.ts")
                || p.ends_with("-test-handler.ts")
                || *p == "server/kafka.ts"
                || *p == "server/rabbitmq.ts"
        }) {
            assert_eq!(files.get(path), reference.get(path), "{path} with `{fw}`");
        }
    }
}

// ---- phase E2: the option `messaging` ---------------------------------------------------------

fn generate_with(
    ir: &Ir,
    services: &[&str],
    options: &Value,
) -> Result<veld_codegen::FileSet, Vec<veld_codegen::GenDiagnostic>> {
    use veld_codegen::{DynGenerator, GenRequest, Role};
    let req = GenRequest {
        ir,
        role: Role::Server,
        services: services.iter().map(|n| service_id(ir, n)).collect(),
        consumes: vec![],
        framework: Some("express".into()),
    };
    // A target with settings must be named (`E0802`): the server target is `api`.
    let options = veld_codegen::with_target_name(options, "api");
    veld_emit_node::Node.generate_dyn(&req, &options)
}

#[test]
fn messaging_names_the_adapters_and_nothing_else_changes() {
    let ir = compile_text(read(DELIVERY));
    let names = ["Orders", "Fulfilment", "Billing"];
    let plain = generate_with(&ir, &names, &Value::Null).unwrap();
    for value in [
        json!("kafka"),
        json!("rabbitmq"),
        json!(["kafka", "rabbitmq"]),
        json!(["rabbitmq", "kafka", "kafka"]),
    ] {
        let files = generate_with(&ir, &names, &json!({ "messaging": value })).unwrap();
        let kafka = value.to_string().contains("kafka");
        let rabbit = value.to_string().contains("rabbitmq");
        assert_eq!(files.get("server/kafka.ts").is_some(), kafka, "{value}");
        assert_eq!(files.get("server/rabbitmq.ts").is_some(), rabbit, "{value}");
        let manifest = text(&files, "package.json");
        assert_eq!(
            manifest.contains("\"kafkajs\""),
            kafka,
            "{value}: {manifest}"
        );
        assert_eq!(
            manifest.contains("\"amqplib\""),
            rabbit,
            "{value}: {manifest}"
        );
        // Only the adapter's file imports its library.
        for (path, bytes) in files.iter() {
            let source = String::from_utf8_lossy(bytes);
            assert_eq!(
                source.contains("from 'kafkajs'"),
                path == "server/kafka.ts",
                "{path}"
            );
            assert!(!source.contains("require("), "{path}");
        }
        // Every other generated file is what it was without the option.
        for (path, bytes) in plain.iter() {
            if matches!(path, "package.json" | "index.ts") {
                continue;
            }
            assert_eq!(files.get(path), Some(bytes), "{path}");
        }
        let barrel = text(&files, "index.ts");
        assert_eq!(barrel.contains("./server/kafka'"), kafka, "{value}");
        assert_eq!(barrel.contains("./server/rabbitmq'"), rabbit, "{value}");
    }
    assert!(plain.get("server/kafka.ts").is_none() && plain.get("server/rabbitmq.ts").is_none());
    let manifest = text(&plain, "package.json");
    assert!(
        !manifest.contains("kafkajs") && !manifest.contains("amqplib"),
        "{manifest}"
    );
}

#[test]
fn an_unknown_broker_and_a_wrong_value_are_e0802() {
    let ir = compile_text(read(DELIVERY));
    let names = ["Orders", "Fulfilment", "Billing"];
    for value in [
        json!("nats"),
        json!(["kafka", "nats"]),
        json!(3),
        json!([1]),
    ] {
        let err = generate_with(&ir, &names, &json!({ "messaging": value })).unwrap_err();
        assert_eq!(err[0].code, "E0802", "{value}");
    }
    let err = generate_with(&ir, &names, &json!({ "messaging": "pulsar" })).unwrap_err();
    assert!(err[0].message.contains("pulsar"), "{}", err[0].message);
    // Like every other option error of this emitter.
    assert!(
        err[0]
            .message
            .starts_with("invalid options for generator `node`: "),
        "{}",
        err[0].message
    );
}

#[test]
fn messaging_on_a_target_without_events_is_e0802() {
    let ir = standard_ir();
    let err = generate_with(&ir, &["Orders"], &json!({ "messaging": "kafka" })).unwrap_err();
    assert_eq!(err[0].code, "E0802");
    assert!(
        err[0]
            .message
            .starts_with("invalid options for generator `node`: "),
        "{}",
        err[0].message
    );
    assert!(
        err[0].message.contains("publishes or subscribes"),
        "{}",
        err[0].message
    );
    // An empty list asks for nothing.
    assert!(generate_with(&ir, &["Orders"], &json!({ "messaging": [] })).is_ok());
}

#[test]
fn messaging_is_accepted_when_one_service_of_the_target_subscribes() {
    let ir = compile_text(read(BOOKINGS));
    assert!(generate_with(&ir, &["Notifications"], &json!({ "messaging": "kafka" })).is_ok());
}

// ---- phase E2: the test handler's files -------------------------------------------------------

#[test]
fn a_subscriber_gets_a_test_handler_with_the_event_endpoint_and_a_publisher_does_not() {
    let ir = compile_text(read(BOOKINGS));
    let options = json!({ "testing": true });
    let files = gen_server_services_fw(&ir, &SERVICES, &options, Some("express"));
    for service in ["notifications", "ledger"] {
        let handler = text(&files, &format!("server/{service}-test-handler.ts"));
        assert!(handler.contains("'/__veld/test/event'"), "{service}");
        assert!(handler.contains("subscribes: {"), "{service}");
        assert!(handler.contains("export function create"), "{service}");
    }
    assert!(
        text(&files, "server/ledger-test-handler.ts")
            .contains("subscribes: { 'Ledger': ['shop.bookings.v1'] }")
    );
    assert!(files.get("server/bookings-test-handler.ts").is_none());
    // Without the option no service gets one.
    let none = gen_server_services_fw(&ir, &SERVICES, &Value::Null, Some("express"));
    assert!(none.paths().all(|p| !p.contains("test-handler")));
}

#[test]
fn a_contract_without_subscriptions_keeps_its_test_handler_bytes() {
    let ir = rules_ir();
    let with = gen_server_with(&ir, "Bookings", &[], &json!({ "testing": true }));
    for (path, bytes) in with.iter().filter(|(p, _)| p.contains("test-handler")) {
        let source = String::from_utf8_lossy(bytes);
        assert!(!source.contains("/__veld/test/event"), "{path}");
        assert!(!source.contains("subscribes"), "{path}");
    }
}

const MIXED: &str = "event Pinged {
  when: datetime
}

channel Pings {
  carries Pinged
}

error TooSmall 422 {
  n: int
}

model Body {
  n: int
}

service Mixed at \"/m\" {
  publishes Pings
  subscribes Pings as \"m\" {
    on Pinged -> heard
  }

  post \"/x\" doit(body: Body) -> 204 | TooSmall {
    require body.n > 0 else TooSmall { n: body.n }
  }
}
";

fn mixed(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(MIXED.to_owned());
    let guard = scratch("events-mixed-");
    let work = guard.path();
    build_with(
        work,
        tsc_path,
        &ir,
        &["Mixed"],
        &[],
        edition,
        &json!({ "typescript": edition, "testing": true }),
    );
    let handler =
        std::fs::read_to_string(work.join("server/server/mixed-test-handler.ts")).unwrap();
    // One file: the handler of the endpoint rules, which also answers the events.
    assert!(handler.contains("export function createMixedTestHandler(impl: MixedService"));
    assert!(handler.contains("return testEvent(req);"));
    assert!(handler.contains("subscribes: { 'Mixed': ['"));
    assert!(handler.contains("/__veld/test'"));
}

#[test]
fn a_service_with_endpoint_rules_and_subscriptions_has_one_test_handler_that_answers_both() {
    matrix(mixed);
}

// ---- phase E3: the outbox (`spec/events.md` section 11) ----------------------------------------

const OUTBOX: &str = "tests/events/outbox.veld.pending";
const OUTBOX_GOLDEN: &str = "tests/wire/events/outbox.json";

/// The first position of `needle` in `text` at or after `from`.
fn find_from(text: &str, needle: &str, from: usize) -> Option<usize> {
    text.get(from..)?.find(needle).map(|i| i + from)
}

/// The text of `contract` with, for each of `events`, a probe move added to `service`: a state
/// machine `Zz<Event>State` with one edge that emits the event, a model, and an endpoint
/// `zz<Event>` whose handler the driver writes the event from. `channel` is moved to the front of
/// the `publishes` lines of the service, because the event goes to the first channel of the
/// service that carries it, and a service without `at` gets one. Returns the text and the path
/// prefix of the service.
fn probe_contract(
    contract: &str,
    service: &str,
    channel: &str,
    events: &[&str],
) -> (String, String) {
    let header = format!("service {service}");
    let at = contract
        .find(&header)
        .expect("the service is in the contract");
    let open = find_from(contract, "{", at).expect("the service has a body");
    let line_end = contract[at..open].to_string();
    let (head, prefix) = match line_end.find(" at \"") {
        Some(i) => {
            let rest = &line_end[i + 5..];
            let prefix = rest[..rest.find('"').expect("a quoted path")].to_string();
            (line_end.clone(), prefix)
        }
        None => (
            format!("{} at \"/zz\" ", line_end.trim_end()),
            "/zz".to_string(),
        ),
    };
    // The end of the body: the brace that closes the one at `open`.
    let mut depth = 0usize;
    let mut close = open;
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
        let trimmed = line.trim();
        if let Some(list) = trimmed.strip_prefix("publishes ") {
            for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                if !channels.iter().any(|c| c == name) {
                    channels.push(name.to_string());
                }
            }
        } else {
            kept.push(line.to_string());
        }
    }
    let mut moves = String::new();
    let mut machines = String::new();
    for e in events {
        moves.push_str(&format!(
            "  post \"/zz/{{id}}/{e}\" zz{e}(id: uuid) -> Zz{e}Box {{\n    fact zz{e}: Zz{e}Box = id\n    transition zz{e}.state: a -> b\n  }}\n"
        ));
        machines.push_str(&format!(
            "\nenum Zz{e}State {{\n  a, b\n  flow a -> b emits {e}\n  final b\n}}\n\nmodel Zz{e}Box {{\n  id: uuid\n  state: Zz{e}State\n}}\n"
        ));
    }
    let text = format!(
        "{}{head}{{\n  publishes {}\n{}\n{moves}}}{}{machines}",
        &contract[..at],
        channels.join(", "),
        kept.join("\n"),
        &contract[close + 1..],
    );
    (text, prefix)
}

/// The cases of `entries` in `outbox.json`, grouped by contract, service and channel.
fn outbox_entry_groups() -> BTreeMap<(String, String, String), Vec<Value>> {
    let golden: Value = serde_json::from_str(&read(OUTBOX_GOLDEN)).unwrap();
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

/// Every `entries` case of the golden through the generated `emit`: a probe move per event is added
/// to the service of the case, and the driver checks the topic, the key and the bytes the outbox
/// received.
fn outbox_entries(tsc_path: &Path, edition: &str, _target: &str) {
    let groups = outbox_entry_groups();
    assert!(groups.len() >= 5, "{} groups", groups.len());
    for ((contract, service, channel), cases) in groups {
        let mut events: Vec<&str> = cases.iter().map(|c| c["event"].as_str().unwrap()).collect();
        events.sort_unstable();
        events.dedup();
        let (text, at) = probe_contract(&read(&contract), &service, &channel, &events);
        let ir = compile_text(text);
        let guard = scratch("events-outbox-wire-");
        let work = guard.path();
        build_with(
            work,
            tsc_path,
            &ir,
            &[service.as_str()],
            &["events_outbox_wire.ts"],
            edition,
            &typescript_option(edition),
        );
        let listed: Vec<Value> = cases
            .iter()
            .map(|c| {
                json!({
                    "name": c["name"], "event": c["event"],
                    "method": camel(c["event"].as_str().unwrap()),
                    "id": c["id"], "time": c["time"], "value": c["value"],
                    "topic": c["topic"], "key": c["key"], "canonical": c["canonical"],
                })
            })
            .collect();
        write_json(
            &work.join("cases.json"),
            &json!({ "service": service, "at": at, "cases": listed }),
        );
        assert!(
            node(work, "events_outbox_wire").contains("events outbox wire ok"),
            "{contract} / {service} / {channel}"
        );
    }
}

#[test]
fn the_entries_of_the_outbox_goldens_come_out_of_the_generated_emit() {
    matrix(outbox_entries);
}

/// The relay and `written` cases of the goldens through the generated relay and the generated
/// check.
fn outbox_relay(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(read(OUTBOX));
    let guard = scratch("events-outbox-relay-");
    let work = guard.path();
    build_with(
        work,
        tsc_path,
        &ir,
        &["Bookings", "Notifications"],
        &["events_outbox_relay.ts"],
        edition,
        &typescript_option(edition),
    );
    std::fs::copy(repo_root().join(OUTBOX_GOLDEN), work.join("outbox.json")).unwrap();
    assert!(node(work, "events_outbox_relay").contains("events outbox relay ok"));
}

#[test]
fn the_relay_and_written_goldens_hold_for_the_generated_relay_and_check() {
    matrix(outbox_relay);
}

/// The moves of the outbox contract over the production handler, and the relay with the memory
/// transport and a subscriber.
fn outbox_http(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(read(OUTBOX));
    let guard = scratch("events-outbox-http-");
    let work = guard.path();
    build_with(
        work,
        tsc_path,
        &ir,
        &["Bookings", "Notifications"],
        &["events_outbox_http.ts"],
        edition,
        &typescript_option(edition),
    );
    assert!(node(work, "events_outbox_http").contains("events outbox http ok"));
}

#[test]
fn an_emitting_move_writes_to_the_outbox_and_the_relay_publishes_it() {
    matrix(outbox_http);
}

/// The call examples of the outbox contract (`expect emitted`, `expect not emitted`) and its event
/// examples through the generated test handlers and the shared live run.
fn outbox_examples(tsc_path: &Path, edition: &str, _target: &str) {
    let ir = compile_text(read(OUTBOX));
    let guard = scratch("events-outbox-live-");
    let work = guard.path();
    build_with(
        work,
        tsc_path,
        &ir,
        &["Bookings", "Notifications"],
        &["events_outbox_serve.ts"],
        edition,
        &json!({ "typescript": edition, "testing": true }),
    );
    let served = common::toolchain::Live::start(work, "events_outbox_serve.js");
    let passed = common::toolchain::run_examples(&served.url, &ir);
    assert_eq!(passed, ir.examples.len());
    assert_eq!(ir.examples.len(), 10, "every example ran, none was skipped");
    let emitted = ir.examples.iter().filter(|e| e.emitted.is_some()).count();
    assert_eq!(emitted, 5, "the examples with an `expect emitted` line");
}

#[test]
fn the_examples_of_the_outbox_contract_pass_live_against_the_generated_test_handlers() {
    matrix(outbox_examples);
}

// ---- phase E3: the generated files ---------------------------------------------------------------

fn outbox_files(options: &Value) -> veld_codegen::FileSet {
    let ir = compile_text(read(OUTBOX));
    gen_server_services_fw(
        &ir,
        &["Bookings", "Notifications"],
        options,
        Some("express"),
    )
}

#[test]
fn snapshot_of_the_files_of_the_outbox_contract() {
    let files = outbox_files(&json!({ "testing": true }));
    insta::assert_snapshot!("events_outbox_file_list", file_list(&files));
    insta::assert_snapshot!(
        "events_outbox_events",
        text(&files, "server/bookings-events.ts")
    );
    insta::assert_snapshot!("events_outbox_server", text(&files, "server/bookings.ts"));
    insta::assert_snapshot!(
        "events_outbox_test_handler",
        text(&files, "server/bookings-test-handler.ts")
    );
    insta::assert_snapshot!("events_outbox_runtime", text(&files, "runtime/events.ts"));
}

#[test]
fn the_outbox_exists_only_for_a_contract_with_an_emitting_move() {
    let with = outbox_files(&json!({}));
    let runtime = text(&with, "runtime/events.ts");
    for word in ["prepareEvent", "emitEvent", "MemoryOutbox", "relayOutbox"] {
        assert!(runtime.contains(word), "{word}");
    }
    let server = text(&with, "server/bookings.ts");
    assert!(server.contains("outbox: veldEvents.Outbox;"));
    assert!(server.contains("pay(req: PayRequest & PayFacts & PayTransition & PayEmit"));
    // The move that emits nothing gets no `emit`.
    assert!(server.contains("refund(req: RefundRequest & RefundFacts & RefundTransition,"));
    assert!(server.contains("emits: { edges: { pending: ['BookingPaid'] }, make: makePayEmit },"));
    assert!(
        server.contains("emits: { edges: { pending: ['BookingCancelled'], paid: ['RefundDue'] }, make: makeCancelEmit },")
    );
    let events = text(&with, "server/bookings-events.ts");
    assert!(events.contains("export function runBookingsOutboxRelay("));
    assert!(events.contains("export function makePayEmit("));
    // The same contract without `emits` (and without the examples that expect an emitted event): no
    // word of the outbox anywhere. `outbox` alone is a local variable of the WebSocket code.
    let stripped = read(OUTBOX)
        .replace(" emits BookingPaid", "")
        .replace(" emits BookingCancelled", "")
        .replace(" emits RefundDue", "")
        .lines()
        .filter(|l| {
            !l.trim_start().starts_with("expect emitted")
                && !l.trim_start().starts_with("expect not emitted")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let ir = compile_text(stripped);
    let without = gen_server_services_fw(
        &ir,
        &["Bookings", "Notifications"],
        &json!({}),
        Some("express"),
    );
    for (path, bytes) in without.iter() {
        let source = String::from_utf8_lossy(bytes);
        for word in [
            "Outbox",
            "hooks.outbox",
            "emitEvent",
            "prepareEvent",
            "checkWritten",
            "EmitSession",
        ] {
            assert!(!source.contains(word), "`{word}` is in {path}");
        }
    }
    // The events runtime of a contract with events and no emit keeps `publishEvent` in one piece.
    assert!(text(&without, "runtime/events.ts").contains("export async function publishEvent("));
}

#[test]
fn the_relay_hands_the_transport_the_options_of_the_channel() {
    let annotated = read(OUTBOX).replace(
        "@topic(\"shop.bookings.v1\")\nchannel BookingEvents",
        "@topic(\"shop.bookings.v1\") @ordered @deadLetter\nchannel BookingEvents",
    );
    let ir = compile_text(annotated);
    let files = gen_server_services_fw(
        &ir,
        &["Bookings", "Notifications"],
        &json!({}),
        Some("express"),
    );
    let events = text(&files, "server/bookings-events.ts");
    assert!(
        events.contains("'shop.bookings.v1': { ordered: true, deadLetter: true },"),
        "{events}"
    );
    assert!(
        events.contains("'shop.refunds.v1': { ordered: false, deadLetter: false },"),
        "{events}"
    );
}

#[test]
fn a_parameter_called_emit_on_a_move_that_emits_is_e0801() {
    let text = read(OUTBOX).replace("pay(id: uuid)", "pay(id: uuid, @query emit: int)");
    let ir = compile_text(text);
    let err = generate_with(&ir, &["Bookings", "Notifications"], &Value::Null).unwrap_err();
    assert!(err.iter().all(|d| d.code == "E0801"), "{err:?}");
    assert!(err[0].message.contains("`emit`"), "{}", err[0].message);
}

/// The outbox next to a timed edge, a derived field, a flagged statement and an `@idempotent`
/// endpoint: the generated server compiles under `tsc --strict` for both editions.
#[test]
fn the_outbox_compiles_next_to_the_other_features() {
    let Some(tsc_path) = tsc_path() else {
        return;
    };
    let text = read(OUTBOX)
        .replace(
            "  flow pending -> cancelled emits BookingCancelled\n",
            "  flow pending -> cancelled after hours(24) emits BookingCancelled\n",
        )
        .replace(
            "  total: decimal\n}",
            "  total: decimal\n  qty: int\n  double: int = qty * 2\n  pendingSince: datetime\n}",
        )
        .replace(
            "    transition booking.status: pending -> paid\n  }",
            "    transition booking.status: pending -> paid\n    require booking.double > 0 when flag(\"positiveOnly\")\n  }",
        )
        .replace("  post \"/{id}/pay\" pay", "  @idempotent\n  post \"/{id}/pay\" pay")
        .replace(
            "transition booking.status: pending, paid -> cancelled",
            "transition booking.status: pending -> cancelled since booking.pendingSince",
        )
        .lines()
        .filter(|l| !l.trim_start().starts_with("expect emitted") && !l.trim_start().starts_with("expect not emitted"))
        .collect::<Vec<_>>()
        .join("\n");
    let ir = compile_text(text);
    for (edition, _) in TARGETS {
        let guard = scratch("events-outbox-combo-");
        build_with(
            guard.path(),
            &tsc_path,
            &ir,
            &["Bookings", "Notifications"],
            &[],
            edition,
            &json!({ "typescript": edition, "testing": true }),
        );
    }
}

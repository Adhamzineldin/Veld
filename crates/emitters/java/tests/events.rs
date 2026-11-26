//! Events in the generated Java server (`spec/events.md` sections 6 to 9, 12 and 14,
//! `spec/targets/java.md` section 12): the wire goldens of `tests/wire/events`, the event examples
//! of `tests/events/bookings.veld`, two services in one process, audited event rules, snapshots of
//! the generated files and the capability checks.
//!
//! The toolchain tests need a JDK; `VELD_REQUIRE_TOOLCHAINS=1` makes a missing one a failure. The
//! generated code is compiled with `javac -Xlint:all -Werror` for every release of `releases()`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::live::{java_main, javac_ok, require_javac, write_files};
use common::*;
use serde_json::{Value, json};
use veld_codegen::naming::{kebab, pascal};
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_java::tree::Tree;
use veld_emit_java::util::{member_ident, type_name};
use veld_ir::{Ir, PackageKind};

const BASE: &str = "gen.e";
const BOOKINGS: &str = "tests/events/bookings.veld";
const EDGES: &str = "tests/wire/events/edges.veld";

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
        "the contract failed to compile:\n{}",
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

// ---- generating, compiling and running -------------------------------------------------------

fn generate(ir: &Ir, services: &[&str], framework: Option<&str>, release: &str) -> FileSet {
    gen_server_services(ir, services, BASE, framework, Some(release))
}

fn all_services(ir: &Ir) -> Vec<String> {
    ir.services.iter().map(|s| s.name.clone()).collect()
}

fn driver_dir() -> PathBuf {
    repo_root().join("tests/toolchain/java/events")
}

/// Writes the server of `services` into `work/src`, compiles it with the drivers (all with
/// `-Xlint:all -Werror`).
fn build(work: &Path, ir: &Ir, services: &[String], drivers: &[&str], release: &str) {
    let names: Vec<&str> = services.iter().map(String::as_str).collect();
    let files = generate(ir, &names, None, release);
    let mut sources = write_files(work, &files);
    for d in drivers {
        sources.push(driver_dir().join(d));
    }
    javac_ok(work, &sources, release);
}

/// Runs `veldtest.<class>` in `work` and returns its standard output.
fn run_java(work: &Path, class: &str) -> String {
    let out = java_main(work, &format!("veldtest.{class}"), &[]);
    assert!(
        out.status.success(),
        "{class} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

fn each_release(test: &str, body: &dyn Fn(&str)) {
    if !require_javac(test) {
        return;
    }
    for release in releases() {
        body(release);
    }
}

// ---- names of generated classes --------------------------------------------------------------

fn tree(ir: &Ir) -> Tree<'_> {
    let all = ir.services.iter().map(|s| s.id).collect();
    Tree::new(ir, BASE, &format!("{BASE}.runtime"), all)
}

/// The Java class of the event, the enum or the model `name`.
fn decl_class(ir: &Ir, name: &str) -> String {
    let tree = tree(ir);
    let d = ir.decls.iter().find(|d| d.name == name).unwrap();
    format!("{}.{}", tree.decl_package(d), type_name(d))
}

/// `<package>.server.<Service><suffix>`.
fn server_class(ir: &Ir, service: &str, suffix: &str) -> String {
    let tree = tree(ir);
    let svc = ir.services.iter().find(|s| s.name == service).unwrap();
    format!(
        "{}.{}{suffix}",
        tree.api_package(svc.package, "server"),
        pascal(&svc.name)
    )
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

fn handler_list(probe: &Probe) -> Vec<Value> {
    probe
        .events
        .iter()
        .map(|e| json!({ "method": member_ident(&format!("on{e}")), "event": e }))
        .collect()
}

/// The consume case that hands a published message back to a consumer of the same event.
fn reverse(case: &Value, canonical: &Value, channel: &str, probe: &Probe, ir: &Ir) -> Value {
    let key = canonical.get("subject").cloned().unwrap_or(Value::Null);
    json!({
        "name": format!("reverse: {}", case["name"].as_str().unwrap()),
        "subscriber": server_class(ir, &probe.service, "Subscriber"),
        "handlers_class": server_class(ir, &probe.service, "EventHandlers"),
        "topic": channel_named(ir, channel).topic,
        "key": key,
        "message": case["canonical"],
        "handlers": handler_list(probe),
    })
}

fn consume_case(case: &Value, probe: &Probe, ir: &Ir) -> Value {
    json!({
        "name": case["name"],
        "subscriber": server_class(ir, &probe.service, "Subscriber"),
        "handlers_class": server_class(ir, &probe.service, "EventHandlers"),
        "topic": channel_named(ir, &probe.channel).topic,
        "key": Value::Null,
        "message": case["message"],
        "handlers": handler_list(probe),
    })
}

fn publish_case(case: &Value, ir: &Ir) -> Value {
    let service = case["service"].as_str().unwrap();
    let event = case["event"].as_str().unwrap();
    json!({
        "name": case["name"],
        "publisher": server_class(ir, service, "Publisher"),
        "channel": member_ident(case["channel"].as_str().unwrap()),
        "method": member_ident(event),
        "eventClass": decl_class(ir, event),
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
            publish.push(publish_case(case, &ir));
            let canonical: Value =
                serde_json::from_str(case["canonical"].as_str().unwrap()).unwrap();
            let channel = case["channel"].as_str().unwrap();
            consume.push(reverse(case, &canonical, channel, probe, &ir));
        } else {
            consume.push(consume_case(case, probe, &ir));
        }
    }
    let services = all_services(&ir);
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

fn wire_goldens(release: &str) {
    for g in goldens() {
        let plan = plan(&g);
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path();
        build(
            work,
            &plan.ir,
            &plan.services,
            &["EventsWire.java"],
            release,
        );
        write_json(&work.join("cases.json"), &plan.cases);
        let out: Value = serde_json::from_str(&run_java(work, "EventsWire")).unwrap();
        check_publish(&plan, &g, &out);
        check_reverse(&g, &out);
        check_consume(&g, &out);
    }
}

/// Every case of `envelopes.json` and `consume.json` through generated Java: the publisher's bytes
/// equal the golden (so a Node consumer takes them, and the other way round), and the consumer's
/// outcome is the golden's.
#[test]
fn the_generated_publisher_and_subscriber_reproduce_the_wire_goldens() {
    each_release(
        "the_generated_publisher_and_subscriber_reproduce_the_wire_goldens",
        &wire_goldens,
    );
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
    let services: Vec<Value> = ir
        .services
        .iter()
        .filter(|s| !s.subscriptions.is_empty())
        .map(|s| {
            json!({
                "name": s.name,
                "subscriber": server_class(ir, &s.name, "Subscriber"),
                "handlers_class": server_class(ir, &s.name, "EventHandlers"),
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
                    "class": server_class(ir, &publisher.name, "Publisher"),
                    "event_class": decl_class(ir, event),
                    "channel": member_ident(&channel.name),
                    "method": member_ident(event),
                },
            })
        })
        .collect();
    json!({ "services": services, "examples": examples })
}

fn examples_against_the_generated_code(release: &str) {
    let ir = compile_text(read(BOOKINGS));
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path();
    build(
        work,
        &ir,
        &all_services(&ir),
        &["EventsExamples.java"],
        release,
    );
    write_json(&work.join("cases.json"), &example_cases(&ir));
    let out: Vec<Value> = serde_json::from_str(&run_java(work, "EventsExamples")).unwrap();
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

/// The 11 event examples of `tests/events/bookings.veld` through the generated publisher and
/// subscribers, compared with the reference evaluator.
#[test]
fn the_event_examples_pass_against_the_generated_publisher_and_subscribers() {
    each_release(
        "the_event_examples_pass_against_the_generated_publisher_and_subscribers",
        &examples_against_the_generated_code,
    );
}

#[test]
fn the_edges_contract_has_no_event_examples_to_run() {
    let ir = compile_text(read(EDGES));
    assert!(ir.examples.iter().all(|e| e.event.is_none()));
}

// ---- two services in one process, audited rules, plain events, the prelude error ------------

fn process(release: &str) {
    let ir = compile_text(read(BOOKINGS));
    let dir = tempfile::tempdir().unwrap();
    build(
        dir.path(),
        &ir,
        &all_services(&ir),
        &["EventsProcess.java"],
        release,
    );
    assert!(run_java(dir.path(), "EventsProcess").contains("events process ok"));
}

#[test]
fn two_generated_services_talk_through_the_memory_transport_in_one_process() {
    each_release(
        "two_generated_services_talk_through_the_memory_transport_in_one_process",
        &process,
    );
}

fn audited(release: &str) {
    let probe = Probe {
        service: "Probe0".into(),
        channel: "InvoiceEvents".into(),
        events: vec!["InvoiceNudged".into()],
    };
    let ir = compile_text(format!("{}{}", read(EDGES), probe.text()));
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path();
    build(
        work,
        &ir,
        &all_services(&ir),
        &["EventsAudit.java"],
        release,
    );
    let out: Value = serde_json::from_str(&run_java(work, "EventsAudit")).unwrap();
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
    each_release(
        "an_audited_rule_of_an_event_is_reported_on_both_sides_and_never_rejects",
        &audited,
    );
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

fn plain(release: &str) {
    let ir = compile_text(PLAIN.to_owned());
    let dir = tempfile::tempdir().unwrap();
    build(
        dir.path(),
        &ir,
        &all_services(&ir),
        &["EventsPlain.java"],
        release,
    );
    assert!(run_java(dir.path(), "EventsPlain").contains("events plain ok"));
}

#[test]
fn events_without_any_rule_compile_and_run() {
    each_release("events_without_any_rule_compile_and_run", &plain);
}

fn prelude_error(release: &str) {
    let ir = veld_emit_conformance::compiled_events_contract();
    let dir = tempfile::tempdir().unwrap();
    build(
        dir.path(),
        &ir,
        &all_services(&ir),
        &["EventsPrelude.java"],
        release,
    );
    assert!(run_java(dir.path(), "EventsPrelude").contains("events prelude ok"));
}

#[test]
fn a_service_with_a_prelude_validation_error_throws_that_class() {
    each_release(
        "a_service_with_a_prelude_validation_error_throws_that_class",
        &prelude_error,
    );
}

// ---- the files: snapshots, the same bytes for every adapter, no events no files ----------------

const SERVICES: [&str; 3] = ["Bookings", "Notifications", "Ledger"];

fn bookings_files(framework: &str) -> FileSet {
    let ir = compile_text(read(BOOKINGS));
    generate(&ir, &SERVICES, Some(framework), "17")
}

fn file_list(files: &FileSet) -> String {
    files
        .paths()
        .filter(|p| !p.contains("/runtime/"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn snapshot_of_the_files_of_a_server_with_events_on_the_jdk_adapter() {
    insta::assert_snapshot!("events_file_list_jdk", file_list(&bookings_files("jdk")));
}

#[test]
fn snapshot_of_the_files_of_a_server_with_events_on_spring() {
    insta::assert_snapshot!(
        "events_file_list_spring",
        file_list(&bookings_files("spring"))
    );
}

#[test]
fn snapshot_of_the_publisher() {
    insta::assert_snapshot!(
        "events_publisher",
        text(
            &bookings_files("jdk"),
            "gen/e/bookings/server/BookingsPublisher.java"
        )
    );
}

#[test]
fn snapshot_of_the_subscriber() {
    insta::assert_snapshot!(
        "events_subscriber",
        text(
            &bookings_files("jdk"),
            "gen/e/bookings/server/LedgerSubscriber.java"
        )
    );
}

#[test]
fn snapshot_of_the_handler_interface() {
    insta::assert_snapshot!(
        "events_handlers",
        text(
            &bookings_files("jdk"),
            "gen/e/bookings/server/LedgerEventHandlers.java"
        )
    );
}

#[test]
fn snapshot_of_an_event_type() {
    insta::assert_snapshot!(
        "events_type",
        text(&bookings_files("jdk"), "gen/e/bookings/t/BookingPaid.java")
    );
}

#[test]
fn snapshot_of_the_events_runtime_files() {
    let files = bookings_files("jdk");
    insta::assert_snapshot!(
        "events_runtime_events",
        text(&files, "gen/e/runtime/Events.java")
    );
    insta::assert_snapshot!(
        "events_runtime_memory_transport",
        text(&files, "gen/e/runtime/MemoryTransport.java")
    );
    insta::assert_snapshot!(
        "events_runtime_transport",
        text(&files, "gen/e/runtime/Transport.java")
    );
}

#[test]
fn the_event_files_are_the_same_for_every_adapter() {
    let reference = bookings_files("jdk");
    let event_file = |p: &str| {
        p.ends_with("Publisher.java")
            || p.ends_with("Subscriber.java")
            || p.ends_with("EventHandlers.java")
            || p.contains("/runtime/")
            || p.contains("/bookings/t/")
    };
    let spring = bookings_files("spring");
    for path in reference.paths().filter(|p| event_file(p)) {
        assert_eq!(
            spring.get(path),
            reference.get(path),
            "{path} differs with the adapter `spring`"
        );
    }
    assert!(
        reference.paths().filter(|p| event_file(p)).count() >= 7,
        "the event files are there"
    );
}

#[test]
fn a_service_that_only_publishes_or_subscribes_has_no_http_classes() {
    for fw in ["jdk", "spring"] {
        let files = bookings_files(fw);
        for gone in ["Notifications", "Ledger"] {
            for suffix in ["Service", "Hooks", "Handler"] {
                let path = format!("gen/e/bookings/server/{gone}{suffix}.java");
                assert!(files.get(&path).is_none(), "{fw}: {path}");
            }
        }
        for kept in ["BookingsService", "BookingsHooks", "BookingsHandler"] {
            let path = format!("gen/e/bookings/server/{kept}.java");
            assert!(files.get(&path).is_some(), "{fw}: {path}");
        }
        for p in files.paths().filter(|p| p.contains("Notifications")) {
            assert!(
                p.ends_with("EventHandlers.java") || p.ends_with("Subscriber.java"),
                "{fw}: only the event classes of a service without endpoints, not {p}"
            );
        }
        let readme = text(&files, "README.md");
        assert!(readme.contains("BookingsHandler"));
        assert!(!readme.contains("NotificationsHandler"));
        assert!(readme.contains("NotificationsSubscriber"));
    }
}

#[test]
fn a_contract_without_events_gets_no_event_files() {
    let files = gen_server(&standard_ir(), "Orders", &["Iam"], "gen.e", None);
    for (path, bytes) in files.iter() {
        for word in [
            "Events.java",
            "MemoryTransport",
            "Transport.java",
            "Publisher",
            "Subscriber",
        ] {
            assert!(!path.contains(word), "{path}");
        }
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains("Events."), "{path}");
    }
}

#[test]
fn an_event_that_a_client_cannot_see_is_in_the_server_and_not_in_the_client() {
    let ir = compile_text(read(BOOKINGS));
    let all = ir.services.iter().map(|s| s.id).collect();
    let client = run(
        &ir,
        Role::Client,
        all,
        vec![],
        None,
        options(BASE, Some("17")),
    );
    for path in client.paths() {
        assert!(!path.contains("Event"), "the client has {path}");
        assert!(!path.contains("Publisher"), "the client has {path}");
    }
}

// ---- capabilities ----------------------------------------------------------------------------

#[test]
fn the_java_server_declares_events_for_every_framework_and_generates_them() {
    use veld_codegen::DynGenerator;
    assert!(
        veld_emit_java::Java
            .capabilities(Role::Server)
            .supports(veld_ir::Feature::Events)
    );
    veld_emit_conformance::check_events_are_generated(&veld_emit_java::Java).unwrap();
    for fw in ["jdk", "spring"] {
        let files = bookings_files(fw);
        assert!(files.get("gen/e/runtime/Events.java").is_some(), "{fw}");
        assert!(
            files
                .get("gen/e/bookings/server/BookingsPublisher.java")
                .is_some(),
            "{fw}"
        );
    }
}

// ---- phase E2: the dead-letter and retry goldens ---------------------------------------------

const DELIVERY: &str = "tests/events/delivery.veld";

/// The server of `services` with `extra` target options (`testing`, `messaging`).
fn generate_with(ir: &Ir, services: &[&str], extra: &Value, release: &str) -> FileSet {
    let mut opts = options(BASE, Some(release));
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        opts[k] = v;
    }
    run(
        ir,
        Role::Server,
        services.iter().map(|n| service_id(ir, n)).collect(),
        vec![],
        None,
        opts,
    )
}

/// Like `build`, with target options.
fn build_with(work: &Path, ir: &Ir, drivers: &[&str], release: &str, extra: &Value) {
    let names = all_services(ir);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let files = generate_with(ir, &names, extra, release);
    let mut sources = write_files(work, &files);
    for d in drivers {
        sources.push(driver_dir().join(d));
    }
    javac_ok(work, &sources, release);
}

/// The messages the retry cases deliver: the first publish golden of `BookingPaid` as it is, with a
/// number where step 3 wants a decimal, with a zero where step 4 wants a positive amount, with a
/// `type` no `on` line names, and as text that is not JSON.
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
        assert_eq!(got["name"], case["name"]);
        assert_eq!(
            got["hex"].as_str().unwrap(),
            golden_hex(case, "copy", "copyHex"),
            "{}: the copy differs from the golden",
            case["name"]
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

fn delivery_goldens(release: &str) {
    let ir = compile_text(read(BOOKINGS));
    let paid = ir.decls.iter().find(|d| d.name == "BookingPaid").unwrap();
    let digest = if let veld_ir::DeclKind::Event(e) = &paid.kind {
        e.rules_digest.clone().unwrap()
    } else {
        panic!("BookingPaid is not an event")
    };
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path();
    build(
        work,
        &ir,
        &all_services(&ir),
        &["DeliveryWire.java"],
        release,
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
    let out: Value = serde_json::from_str(&run_java(work, "DeliveryWire")).unwrap();
    check_dead_letter(&dead, &out["deadLetter"]);
    check_retry(&retry, &out["retry"]);
}

#[test]
fn the_generated_consumer_reproduces_the_dead_letter_and_retry_goldens() {
    each_release(
        "the_generated_consumer_reproduces_the_dead_letter_and_retry_goldens",
        &delivery_goldens,
    );
}

#[test]
fn every_member_of_the_retry_goldens_is_read_by_the_driver() {
    // A golden case the driver cannot run (a new kind of case) must fail here, not silently pass.
    for case in read_json("tests/wire/events/retry.json") {
        for key in case.as_object().unwrap().keys() {
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
                .contains(&key.as_str()),
                "retry.json has a member the driver does not read: {key}"
            );
        }
    }
}

// ---- phase E2: two services in one process ---------------------------------------------------

fn delivery_process(release: &str) {
    let ir = compile_text(read(DELIVERY));
    let dir = tempfile::tempdir().unwrap();
    build(
        dir.path(),
        &ir,
        &all_services(&ir),
        &["DeliveryProcess.java"],
        release,
    );
    assert!(run_java(dir.path(), "DeliveryProcess").contains("events delivery process ok"));
}

#[test]
fn retry_dead_letter_and_the_inbox_work_between_generated_services_in_one_process() {
    each_release(
        "retry_dead_letter_and_the_inbox_work_between_generated_services_in_one_process",
        &delivery_process,
    );
}

// ---- phase E2: the event examples through the generated test handlers -------------------------

/// Every event example of `path` through the generated test handlers of every service that
/// subscribes, mounted in one server (`EventsServe.java`), with the shared live runner. Every
/// example must pass.
fn live_event_examples(release: &str, path: &str) -> usize {
    let ir = compile_text(read(path));
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path();
    build_with(
        work,
        &ir,
        &["EventsServe.java"],
        release,
        &json!({ "testing": true }),
    );
    let tree = tree(&ir);
    let handlers: Vec<String> = ir
        .services
        .iter()
        .filter(|s| !s.subscriptions.is_empty())
        .map(|s| {
            format!(
                "{}.{}TestHandler",
                tree.api_package(s.package, "server"),
                pascal(&s.name)
            )
        })
        .collect();
    assert!(!handlers.is_empty(), "{path}: no service subscribes");
    let args: Vec<&str> = handlers.iter().map(String::as_str).collect();
    let served = common::live::Running::start(work, "veldtest.EventsServe", &args);
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

#[test]
fn every_event_example_passes_live_against_the_generated_test_handlers() {
    each_release(
        "every_event_example_passes_live_against_the_generated_test_handlers",
        &|release| {
            assert_eq!(live_event_examples(release, DELIVERY), 4);
            assert_eq!(live_event_examples(release, BOOKINGS), 11);
        },
    );
}

#[test]
fn a_service_that_subscribes_has_a_test_handler_only_with_the_option() {
    let ir = compile_text(read(BOOKINGS));
    let plain = generate_with(&ir, &SERVICES, &json!({}), "17");
    assert!(
        plain
            .paths()
            .all(|p| !p.contains("TestHandler") && !p.ends_with("/TestSupport.java"))
    );
    let testing = generate_with(&ir, &SERVICES, &json!({ "testing": true }), "17");
    for name in ["Notifications", "Ledger"] {
        let t = text(
            &testing,
            &format!("gen/e/bookings/server/{name}TestHandler.java"),
        );
        assert!(t.contains("public static VeldHandler create()"), "{t}");
        assert!(t.contains("TestSupport.events("), "{t}");
    }
    // Bookings only publishes and has no endpoint rule: no test handler.
    assert!(
        testing
            .get("gen/e/bookings/server/BookingsTestHandler.java")
            .is_none()
    );
    let support = text(&testing, "gen/e/runtime/TestSupport.java");
    assert!(support.contains("/__veld/test/event"), "{support}");
}

// ---- phase E2: the files, the option `messaging` ----------------------------------------------

fn delivery_files(extra: &Value) -> FileSet {
    let ir = compile_text(read(DELIVERY));
    generate_with(&ir, &["Orders", "Fulfilment", "Billing"], extra, "17")
}

#[test]
fn snapshot_of_the_event_files_of_a_contract_with_all_three_annotations() {
    let files = delivery_files(&json!({ "testing": true }));
    insta::assert_snapshot!("events_delivery_file_list", file_list(&files));
    insta::assert_snapshot!(
        "events_delivery_publisher",
        text(&files, "gen/e/bookings/server/OrdersPublisher.java")
    );
    insta::assert_snapshot!(
        "events_delivery_subscriber",
        text(&files, "gen/e/bookings/server/FulfilmentSubscriber.java")
    );
    insta::assert_snapshot!(
        "events_delivery_subscriber_billing",
        text(&files, "gen/e/bookings/server/BillingSubscriber.java")
    );
    insta::assert_snapshot!(
        "events_delivery_test_handler",
        text(&files, "gen/e/bookings/server/FulfilmentTestHandler.java")
    );
    insta::assert_snapshot!(
        "events_delivery_runtime",
        text(&files, "gen/e/runtime/Events.java")
    );
    insta::assert_snapshot!(
        "events_delivery_transport",
        text(&files, "gen/e/runtime/Transport.java")
    );
}

#[test]
fn a_channel_without_annotations_keeps_its_three_argument_literals() {
    let ir = compile_text(read(BOOKINGS));
    let files = generate_with(&ir, &SERVICES, &json!({}), "17");
    let publisher = text(&files, "gen/e/bookings/server/BookingsPublisher.java");
    assert!(!publisher.contains("true"), "{publisher}");
    let subscriber = text(&files, "gen/e/bookings/server/NotificationsSubscriber.java");
    assert!(!subscriber.contains("RetryPolicy"), "{subscriber}");
    let annotated = delivery_files(&json!({}));
    let publisher = text(&annotated, "gen/e/bookings/server/OrdersPublisher.java");
    assert!(
        publisher.contains("KeyKind.UUID, true, true)"),
        "{publisher}"
    );
    let subscriber = text(&annotated, "gen/e/bookings/server/BillingSubscriber.java");
    assert!(
        subscriber.contains("new Events.RetryPolicy(3, Events.Backoff.EXPONENTIAL), true, true)"),
        "{subscriber}"
    );
}

#[test]
fn snapshot_of_the_broker_adapters() {
    let files = delivery_files(&json!({ "messaging": ["rabbitmq", "kafka"] }));
    insta::assert_snapshot!(
        "events_kafka_adapter",
        text(&files, "gen/e/adapters/KafkaTransport.java")
    );
    insta::assert_snapshot!(
        "events_rabbitmq_adapter",
        text(&files, "gen/e/adapters/RabbitMqTransport.java")
    );
    insta::assert_snapshot!("events_adapters_readme", text(&files, "gen/e/README.md"));
}

#[test]
fn the_option_messaging_chooses_the_adapters_and_nothing_else_changes() {
    let base = delivery_files(&json!({}));
    assert!(
        base.paths()
            .all(|p| !p.contains("KafkaTransport") && !p.contains("RabbitMqTransport"))
    );
    assert!(!text(&base, "gen/e/README.md").contains("kafka-clients"));
    let kafka = delivery_files(&json!({ "messaging": "kafka" }));
    assert!(kafka.get("gen/e/adapters/KafkaTransport.java").is_some());
    assert!(kafka.get("gen/e/adapters/RabbitMqTransport.java").is_none());
    assert!(text(&kafka, "gen/e/README.md").contains("org.apache.kafka:kafka-clients"));
    assert!(!text(&kafka, "gen/e/README.md").contains("amqp-client"));
    let both = delivery_files(&json!({ "messaging": ["rabbitmq", "kafka", "kafka"] }));
    assert!(both.get("gen/e/adapters/KafkaTransport.java").is_some());
    assert!(both.get("gen/e/adapters/RabbitMqTransport.java").is_some());
    // Only the adapters and the README differ from the contract without the option.
    for (path, bytes) in base.iter() {
        if path != "gen/e/README.md" {
            assert_eq!(both.get(path), Some(bytes), "{path} changed");
        }
    }
    let none = delivery_files(&json!({ "messaging": [] }));
    assert_eq!(none, base, "an empty list asks for nothing");
    // Only the adapters import a library.
    for (path, bytes) in both.iter() {
        let text = String::from_utf8_lossy(bytes);
        let library = text.contains("org.apache.kafka") || text.contains("com.rabbitmq");
        assert_eq!(
            library,
            path.ends_with("KafkaTransport.java") || path.ends_with("RabbitMqTransport.java"),
            "{path}"
        );
    }
}

fn messaging_error(
    extra: Value,
    role: Role,
    services: &[&str],
) -> Vec<veld_codegen::GenDiagnostic> {
    use veld_codegen::{DynGenerator, GenRequest};
    let ir = compile_text(read(DELIVERY));
    let mut opts = options(BASE, Some("17"));
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        opts[k] = v;
    }
    let req = GenRequest {
        ir: &ir,
        role,
        services: services.iter().map(|n| service_id(&ir, n)).collect(),
        consumes: vec![],
        framework: None,
    };
    veld_emit_java::Java.generate_dyn(&req, &opts).unwrap_err()
}

#[test]
fn a_wrong_messaging_option_is_e0802() {
    let all = ["Orders", "Fulfilment", "Billing"];
    for bad in [json!("pulsar"), json!(["kafka", "nats"]), json!(3)] {
        let d = messaging_error(json!({ "messaging": bad }), Role::Server, &all);
        assert!(
            d.iter().all(|d| d.code == "E0802") && !d.is_empty(),
            "{bad}: {d:?}"
        );
    }
    // A client has no events: the option is wrong there.
    let d = messaging_error(json!({ "messaging": "kafka" }), Role::Client, &all);
    assert!(
        !d.is_empty() && d.iter().all(|d| d.code == "E0802"),
        "{d:?}"
    );
}

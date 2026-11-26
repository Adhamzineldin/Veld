//! The outbox of events phase E3 in the generated Java server (`spec/events.md` section 11,
//! `spec/targets/java.md` section 12.6): the `entries` of `tests/wire/events/outbox.json` through
//! the generated `emit`, its `relay` and `written` cases through the generated relay and check, the
//! moves of `tests/events/outbox.veld.pending` over the production handler, its examples through
//! the generated test handlers and the shared live runner, snapshots of the generated files, the
//! gate (a contract without an emitting move has no word of the outbox), `E0801` for a parameter
//! called `emit`, and `javac -Xlint:all -Werror` next to the other features.
//!
//! The toolchain tests need a JDK; `VELD_REQUIRE_TOOLCHAINS=1` makes a missing one a failure. The
//! contract file keeps its `.pending` suffix until every generator implements the feature.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::live::{Running, java_main, javac_ok, require_javac, write_files};
use common::*;
use serde_json::{Value, json};
use veld_codegen::naming::pascal;
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_java::tree::Tree;
use veld_emit_java::util::{member_ident, type_name};
use veld_ir::{Ir, PackageKind};

const BASE: &str = "gen.b";
const OUTBOX: &str = "tests/events/outbox.veld.pending";
const OUTBOX_GOLDEN: &str = "tests/wire/events/outbox.json";

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap()
        .replace("\r\n", "\n")
}

/// Compiles `text` as the package `bookings` from the file `t.veld`: the goldens were made so, and
/// the module name is part of the signatures.
fn compile_golden(text: String) -> Ir {
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

/// The outbox contract as the package `bookings` in the file `bookings.veld`: the module of its
/// declarations is `bookings` too (`gen.b.bookings.bookings.Booking`), which the drivers name.
fn outbox_ir() -> Ir {
    compile_source(read(OUTBOX), "bookings", OUTBOX)
}

fn driver_dir() -> std::path::PathBuf {
    repo_root().join("tests/toolchain/java/outbox")
}

/// The server of `services` with `extra` target options (`testing`).
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

/// Writes the server of `services` into `work/src` and compiles it with the drivers (all with
/// `-Xlint:all -Werror`).
fn build(work: &Path, ir: &Ir, services: &[&str], drivers: &[&str], release: &str, extra: &Value) {
    let files = generate_with(ir, services, extra, release);
    let mut sources = write_files(work, &files);
    for d in drivers {
        sources.push(driver_dir().join(d));
    }
    javac_ok(work, &sources, release);
}

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

fn each_release(test: &str, body: &dyn Fn(&str)) {
    if !require_javac(test) {
        return;
    }
    for release in releases() {
        body(release);
    }
}

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

// ---- the entries of the golden ------------------------------------------------------------------

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
fn entry_groups() -> BTreeMap<(String, String, String), Vec<Value>> {
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
fn entries(release: &str) {
    let groups = entry_groups();
    assert!(groups.len() >= 5, "{} groups", groups.len());
    for ((contract, service, channel), cases) in groups {
        let mut events: Vec<&str> = cases.iter().map(|c| c["event"].as_str().unwrap()).collect();
        events.sort_unstable();
        events.dedup();
        let (text, at) = probe_contract(&read(&contract), &service, &channel, &events);
        let ir = compile_golden(text);
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path();
        build(
            work,
            &ir,
            &[service.as_str()],
            &["OutboxWire.java"],
            release,
            &json!({}),
        );
        let listed: Vec<Value> = cases
            .iter()
            .map(|c| {
                let event = c["event"].as_str().unwrap();
                json!({
                    "name": c["name"], "event": event,
                    "method": member_ident(event),
                    "eventClass": decl_class(&ir, event),
                    "boxClass": decl_class(&ir, &format!("Zz{event}Box")),
                    "id": c["id"], "time": c["time"], "value": c["value"],
                    "topic": c["topic"], "key": c["key"], "canonical": c["canonical"],
                })
            })
            .collect();
        let group = json!({
            "service": server_class(&ir, &service, "Service"),
            "loaders": server_class(&ir, &service, "FactLoaders"),
            "hooks": server_class(&ir, &service, "Hooks"),
            "handler": server_class(&ir, &service, "Handler"),
            "at": at,
            "cases": listed,
        });
        std::fs::write(
            work.join("cases.json"),
            serde_json::to_string_pretty(&group).unwrap(),
        )
        .unwrap();
        assert!(
            run_java(work, "OutboxWire").contains("OK"),
            "{contract} / {service} / {channel}"
        );
    }
}

#[test]
fn the_entries_of_the_outbox_goldens_come_out_of_the_generated_emit() {
    each_release(
        "the_entries_of_the_outbox_goldens_come_out_of_the_generated_emit",
        &entries,
    );
}

// ---- the relay and the check --------------------------------------------------------------------

/// The `relay` and `written` cases of the golden through the generated relay and the generated check.
fn relay(release: &str) {
    let ir = outbox_ir();
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path();
    build(
        work,
        &ir,
        &["Bookings", "Notifications"],
        &["OutboxRelayGolden.java"],
        release,
        &json!({}),
    );
    std::fs::copy(repo_root().join(OUTBOX_GOLDEN), work.join("outbox.json")).unwrap();
    assert!(run_java(work, "OutboxRelayGolden").contains("OK"));
}

#[test]
fn the_relay_and_written_goldens_hold_for_the_generated_relay_and_check() {
    each_release(
        "the_relay_and_written_goldens_hold_for_the_generated_relay_and_check",
        &relay,
    );
}

// ---- over the real entry point ------------------------------------------------------------------

/// The moves of the outbox contract over the production handler, and the relay with the memory
/// transport and a subscriber.
fn http(release: &str) {
    let ir = outbox_ir();
    let dir = tempfile::tempdir().unwrap();
    build(
        dir.path(),
        &ir,
        &["Bookings", "Notifications"],
        &["OutboxHttp.java"],
        release,
        &json!({}),
    );
    assert!(run_java(dir.path(), "OutboxHttp").contains("OK"));
}

#[test]
fn an_emitting_move_writes_to_the_outbox_and_the_relay_publishes_it() {
    each_release(
        "an_emitting_move_writes_to_the_outbox_and_the_relay_publishes_it",
        &http,
    );
}

/// The call examples of the outbox contract (`expect emitted`, `expect not emitted`) and its event
/// examples through the generated test handlers and the shared live run.
fn examples(release: &str) {
    let ir = outbox_ir();
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path();
    build(
        work,
        &ir,
        &["Bookings", "Notifications"],
        &["OutboxServe.java"],
        release,
        &json!({ "testing": true }),
    );
    let served = Running::start(work, "veldtest.OutboxServe", &[]);
    let passed = common::live::run_every_example(&ir, &served.url);
    assert_eq!(passed, ir.examples.len());
    assert_eq!(ir.examples.len(), 10, "every example ran, none was skipped");
    let emitted = ir.examples.iter().filter(|e| e.emitted.is_some()).count();
    assert_eq!(emitted, 5, "the examples with an `expect emitted` line");
}

#[test]
fn the_examples_of_the_outbox_contract_pass_live_against_the_generated_test_handlers() {
    each_release(
        "the_examples_of_the_outbox_contract_pass_live_against_the_generated_test_handlers",
        &examples,
    );
}

// ---- the generated files ------------------------------------------------------------------------

fn outbox_files(extra: &Value) -> FileSet {
    generate_with(&outbox_ir(), &["Bookings", "Notifications"], extra, "17")
}

fn file_list(files: &FileSet) -> String {
    files
        .paths()
        .filter(|p| !p.contains("/runtime/"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn snapshot_of_the_files_of_the_outbox_contract() {
    let files = outbox_files(&json!({ "testing": true }));
    insta::assert_snapshot!("events_outbox_file_list", file_list(&files));
    for name in [
        "PayEmit",
        "CancelEmit",
        "BookingsOutboxRelay",
        "BookingsPublisher",
        "BookingsService",
        "BookingsHooks",
        "BookingsHandler",
        "BookingsTestHandler",
    ] {
        insta::assert_snapshot!(
            format!("events_outbox_{name}"),
            text(&files, &format!("gen/b/bookings/server/{name}.java"))
        );
    }
    for name in [
        "Events",
        "Emission",
        "Outbox",
        "MemoryOutbox",
        "TestSupport",
    ] {
        insta::assert_snapshot!(
            format!("events_outbox_runtime_{name}"),
            text(&files, &format!("gen/b/runtime/{name}.java"))
        );
    }
}

/// The outbox exists only for a contract with an emitting move, and the same contract without
/// `emits` keeps `publish` in one piece.
#[test]
fn the_outbox_exists_only_for_a_contract_with_an_emitting_move() {
    let with = outbox_files(&json!({}));
    let events = text(&with, "gen/b/runtime/Events.java");
    for word in [
        "prepare(",
        "static <E> PublishedEvent emit(",
        "RelayOptions",
        "relay(",
    ] {
        assert!(events.contains(word), "{word}");
    }
    for file in ["Outbox", "OutboxEntry", "MemoryOutbox", "Emission"] {
        assert!(
            with.get(&format!("gen/b/runtime/{file}.java")).is_some(),
            "{file}"
        );
    }
    let server = text(&with, "gen/b/bookings/server/BookingsService.java");
    assert!(server.contains("Transition<Status> transition, PayEmit emit, RequestContext ctx"));
    // The move that emits nothing gets no `emit`.
    assert!(
        server.contains("RefundFacts facts, Transition<Status> transition, RequestContext ctx")
    );
    assert!(
        text(&with, "gen/b/bookings/server/BookingsHooks.java")
            .contains("public BookingsHooks(Outbox outbox)")
    );
    // The same contract without `emits` (and without the examples that expect an emitted event): no
    // word of the outbox anywhere.
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
    let ir = compile_source(stripped, "bookings", "the outbox contract without emits");
    let without = generate_with(
        &ir,
        &["Bookings", "Notifications"],
        &json!({ "testing": true }),
        "17",
    );
    for (path, bytes) in without.iter() {
        let source = String::from_utf8_lossy(bytes);
        for word in [
            "Outbox",
            "Emission",
            "hooks.outbox",
            "emissions",
            "Veld-Test-Emitted",
            "veld-test-emitted",
            "checkWritten",
            "Events.emit",
        ] {
            assert!(!source.contains(word), "`{word}` is in {path}");
        }
    }
    let runtime = text(&without, "gen/b/runtime/Events.java");
    assert!(!runtime.contains("private static <E> Prepared prepare("));
    assert!(runtime.contains("public static <E> PublishedEvent publish("));
}

/// The relay hands the transport the options of the channel (`spec/events.md` section 10.6).
#[test]
fn the_relay_hands_the_transport_the_options_of_the_channel() {
    let annotated = read(OUTBOX).replace(
        "@topic(\"shop.bookings.v1\")\nchannel BookingEvents",
        "@topic(\"shop.bookings.v1\") @ordered @deadLetter\nchannel BookingEvents",
    );
    let ir = compile_source(annotated, "bookings", "the annotated outbox contract");
    let files = generate_with(&ir, &["Bookings", "Notifications"], &json!({}), "17");
    let relay = text(&files, "gen/b/bookings/server/BookingsOutboxRelay.java");
    assert!(
        relay.contains("Map.entry(\"shop.bookings.v1\", new Transport.ChannelOptions(true, true))"),
        "{relay}"
    );
    assert!(
        relay
            .contains("Map.entry(\"shop.refunds.v1\", new Transport.ChannelOptions(false, false))"),
        "{relay}"
    );
}

/// A parameter or a fact called `emit` on a move that emits would hide the handler's argument.
#[test]
fn a_parameter_called_emit_on_a_move_that_emits_is_e0801() {
    let text = read(OUTBOX).replace("pay(id: uuid)", "pay(id: uuid, @query emit: int)");
    let ir = compile_source(
        text,
        "bookings",
        "the outbox contract with an `emit` parameter",
    );
    let req = veld_codegen::GenRequest {
        ir: &ir,
        role: Role::Server,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: None,
    };
    let err = veld_codegen::DynGenerator::generate_dyn(
        &veld_emit_java::Java,
        &req,
        &options(BASE, Some("17")),
    )
    .unwrap_err();
    assert!(err.iter().all(|d| d.code == "E0801"), "{err:?}");
    assert!(err[0].message.contains("`emit`"), "{}", err[0].message);
}

/// The outbox next to a timed edge, a derived field, a flagged statement and an `@idempotent`
/// endpoint: the generated server compiles with every warning on, with and without the test
/// handler, for every release.
#[test]
fn the_outbox_compiles_next_to_the_other_features() {
    if !require_javac("the_outbox_compiles_next_to_the_other_features") {
        return;
    }
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
        .filter(|l| {
            !l.trim_start().starts_with("expect emitted")
                && !l.trim_start().starts_with("expect not emitted")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let ir = compile_source(text, "bookings", "the outbox next to the other features");
    for testing in [false, true] {
        for release in releases() {
            let dir = tempfile::tempdir().unwrap();
            let files = generate_with(
                &ir,
                &["Bookings", "Notifications"],
                &json!({ "testing": testing }),
                release,
            );
            let sources = write_files(dir.path(), &files);
            javac_ok(dir.path(), &sources, release);
        }
    }
}

/// The event files and the emit files are the same for `jdk` and `spring` (the Spring classes need
/// Spring on the class path, so this generates and compares only).
#[test]
fn the_outbox_files_are_the_same_for_every_adapter() {
    let ir = outbox_ir();
    let services = ["Bookings", "Notifications"];
    let jdk = generate_with(&ir, &services, &json!({}), "17");
    let opts = options(BASE, Some("17"));
    let spring = run(
        &ir,
        Role::Server,
        services.iter().map(|n| service_id(&ir, n)).collect(),
        vec![],
        Some("spring"),
        opts,
    );
    for file in [
        "PayEmit",
        "CancelEmit",
        "BookingsOutboxRelay",
        "BookingsPublisher",
        "BookingsHooks",
        "BookingsHandler",
    ] {
        let path = format!("gen/b/bookings/server/{file}.java");
        assert_eq!(text(&jdk, &path), text(&spring, &path), "{file}");
    }
    let controller = text(&spring, "gen/b/adapters/bookings/BookingsController.java");
    assert!(controller.contains("BookingsHooks"), "{controller}");
}

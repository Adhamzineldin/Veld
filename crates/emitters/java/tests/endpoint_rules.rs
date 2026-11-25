//! Endpoint rules (`spec/rules.md` section 9, `spec/targets/java.md` section 9): what is generated
//! for `tests/rules/endpoints/bookings.veld`, that it compiles under `javac -Xlint:all -Werror`,
//! and that the test handler leaves the production files alone.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

pub const BOOKINGS: &str = "tests/rules/endpoints/bookings.veld";

pub fn bookings_ir() -> Ir {
    let text = std::fs::read_to_string(repo_root().join(BOOKINGS)).unwrap();
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
    let rendered = out.render();
    out.ir
        .unwrap_or_else(|| panic!("bookings do not compile:\n{rendered}"))
}

pub fn generate(ir: &Ir, role: Role, framework: Option<&str>, testing: bool) -> FileSet {
    let mut o = options("gen.b", Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], framework, o)
}

#[test]
fn the_server_files_of_the_bookings_contract_are_snapshotted() {
    let ir = bookings_ir();
    let files = generate(&ir, Role::Server, None, true);
    let paths: Vec<&str> = files.paths().filter(|p| !p.contains("/runtime/")).collect();
    insta::assert_snapshot!("endpoint_rules_server_file_list", paths.join("\n"));
    for name in [
        "BookingsHandler",
        "BookingsTestHandler",
        "BookingsHooks",
        "BookingsService",
        "BookingsFactLoaders",
        "CreateBookingFacts",
    ] {
        let path = format!("gen/b/bookings/server/{name}.java");
        insta::assert_snapshot!(format!("endpoint_rules_{name}"), text(&files, &path));
    }
    let runtime: Vec<&str> = files
        .paths()
        .filter(|p| {
            ["EndpointRules", "DigestHandler", "TestSupport"]
                .iter()
                .any(|n| p.ends_with(&format!("/runtime/{n}.java")))
        })
        .collect();
    insta::assert_snapshot!("endpoint_rules_runtime_files", runtime.join("\n"));
}

#[test]
fn the_spring_controller_takes_the_fact_loaders_as_a_bean() {
    let ir = bookings_ir();
    let files = generate(&ir, Role::Server, Some("spring"), false);
    insta::assert_snapshot!(
        "endpoint_rules_spring_controller",
        text(&files, "gen/b/adapters/bookings/BookingsController.java")
    );
}

#[test]
fn the_test_handler_changes_nothing_in_the_production_files() {
    let ir = bookings_ir();
    let plain = generate(&ir, Role::Server, None, false);
    let testing = generate(&ir, Role::Server, None, true);
    for (path, bytes) in plain.iter() {
        let other = testing
            .get(path)
            .unwrap_or_else(|| panic!("{path} is gone"));
        assert_eq!(bytes, other, "{path} differs with `testing = true`");
    }
    let added: Vec<&str> = testing.paths().filter(|p| plain.get(p).is_none()).collect();
    assert_eq!(
        added,
        [
            "gen/b/bookings/server/BookingsTestHandler.java",
            "gen/b/runtime/TestSupport.java"
        ]
    );
    for (path, bytes) in plain.iter().filter(|(p, _)| p.ends_with(".java")) {
        let text = String::from_utf8_lossy(bytes);
        assert!(
            !text.contains("Veld-Test") && !text.contains("veld-test"),
            "{path} reads a test header"
        );
    }
}

/// A running Java process that printed `PORT=<n>`; killed when dropped.
#[allow(missing_debug_implementations)]
pub struct Running {
    tree: veld_test_support::ProcessTree,
    pub url: String,
}

impl Running {
    pub fn start(work: &std::path::Path, class: &str, args: &[&str]) -> Running {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        let mut cmd = Command::new("java");
        cmd.args(["-cp", "classes", class])
            .args(args)
            .current_dir(work)
            .stdin(Stdio::null())
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
        Running {
            tree,
            url: format!("http://127.0.0.1:{port}"),
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.tree.kill_tree();
    }
}

/// Sends every example of the contract through the shared live runner, in dry-run mode, and
/// returns how many passed.
pub fn run_every_example(ir: &Ir, url: &str) -> usize {
    let live = veld_testing::Live::connect(url, true).unwrap();
    let (mut passed, mut failures) = (0, Vec::new());
    for ex in &ir.examples {
        match live.run_example(ir, ex) {
            Ok(report) if report.passed() => passed += 1,
            Ok(report) => failures.push(format!("\"{}\": {report}", ex.name)),
            Err(e) => failures.push(format!("\"{}\": {e}", ex.name)),
        }
    }
    assert!(
        failures.is_empty(),
        "{} examples failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    passed
}

#[test]
fn every_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_example_passes_through_the_jdk_test_handler") {
        return;
    }
    let ir = bookings_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, None, true));
    sources.push(repo_root().join("tests/toolchain/java/rules/EndpointServe.java"));
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), "veldtest.EndpointServe", &[]);
    let passed = run_every_example(&ir, &server.url);
    eprintln!("examples passed on the JDK test handler: {passed}");
    assert_eq!(passed, 20, "every example of tests/rules/endpoints");
}

/// Compiles the bookings server (`gen.b`) and the given driver and runs it.
fn run_driver(driver: &str, class: &str, with_client: bool) {
    let ir = bookings_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, None, false));
    if with_client {
        let mut o = options("gen.c", Some("17"));
        o["testing"] = serde_json::json!(false);
        let all = ir.services.iter().map(|s| s.id).collect();
        let client = run(&ir, Role::Client, all, vec![], None, o);
        sources.extend(write_files(work.path(), &client));
    }
    sources.push(repo_root().join("tests/toolchain/java/rules").join(driver));
    javac_ok(work.path(), &sources, "17");
    let out = java_main(work.path(), class, &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "{class} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_production_handler_runs_facts_requires_ensures_and_old_over_http() {
    if !require_javac("the_production_handler_runs_facts") {
        return;
    }
    run_driver("EndpointHttp.java", "veldtest.EndpointHttp", false);
}

#[test]
fn every_example_passes_through_the_generated_test_handler_on_spring() {
    let name = "every_example_passes_through_the_test_handler_on_spring";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    let ir = bookings_ir();
    let files = generate(&ir, Role::Server, Some("spring"), true);
    let app = start_spring(
        "spring-endpoints",
        &files,
        &["rules/spring-endpoints/App.java"],
        "/__veld/test",
        &[],
    );
    let passed = run_every_example(&ir, &format!("http://127.0.0.1:{}", app.port));
    eprintln!("examples passed on the Spring test handler: {passed}");
    assert_eq!(passed, 20, "every example of tests/rules/endpoints");
}

#[test]
fn the_rules_digest_is_sent_and_the_client_follows_it() {
    if !require_javac("the_rules_digest_is_sent") {
        return;
    }
    run_driver("EndpointDigest.java", "veldtest.EndpointDigest", true);
}

#[test]
fn the_bookings_server_and_client_compile_with_every_warning_on() {
    if !require_javac("the_bookings_server_and_client_compile") {
        return;
    }
    let ir = bookings_ir();
    for (role, framework, testing) in [
        (Role::Server, None, false),
        (Role::Server, None, true),
        (Role::Client, None, false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let sources = write_files(dir.path(), &generate(&ir, role, framework, testing));
        for release in releases() {
            javac_ok(dir.path(), &sources, release);
        }
    }
}

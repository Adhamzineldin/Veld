//! Flags on events (`spec/platform.md` section 11, `spec/targets/rust.md` section 17.2): a flagged
//! invariant of an event is evaluated by the publisher and the consumer when the provider of their
//! options says the flag is on, with a context that has no principal. The contract is
//! `tests/toolchain/contract/platform_p2/flags_events.veld`, compiled as `t.veld` of the package
//! `bookings`; the driver is `tests/toolchain/rust/events/flags.rs`.
//!
//! Without `cargo` the toolchain test is skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;

use common::*;
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

const CONTRACT: &str = "tests/toolchain/contract/platform_p2/flags_events.veld";

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap()
        .replace("\r\n", "\n")
}

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

fn server_crate(ir: &Ir) -> FileSet {
    let ids = ir.services.iter().map(|s| s.id).collect();
    run(
        ir,
        Role::Server,
        ids,
        vec![],
        None,
        serde_json::json!({ "crate-name": "events-server" }),
    )
}

#[test]
fn only_a_crate_with_a_flagged_event_has_the_provider_in_its_options() {
    let flagged = server_crate(&compile_text(read(CONTRACT)));
    let events = text(&flagged, "src/veld/events.rs");
    for word in [
        "pub fn flags<P: FlagProvider>(mut self, provider: Arc<P>) -> Self",
        "pub fn on_flag_error(mut self, f: impl Fn(&Error) + Send + Sync + 'static) -> Self",
        "pub flags: &'static [&'static str],",
        "async fn read_flags(",
    ] {
        assert!(events.contains(word), "missing `{word}`");
    }
    assert!(text(&flagged, "src/veld/flags.rs").contains("pub fn of_event(operation: &str)"));
    assert!(
        !text(&flagged, "src/veld/flags.rs").contains("Principal"),
        "a crate that only publishes and subscribes has no principal"
    );
    assert!(text(&flagged, "src/bookings/desk_events.rs").contains("flags: &[\"smallGroups\"],"));
    // The same events without the flag: none of the words, and no `flags` file.
    let plain = server_crate(&compile_text(
        read(CONTRACT).replace(" when flag(\"smallGroups\")", ""),
    ));
    assert!(plain.get("src/veld/flags.rs").is_none());
    let events = text(&plain, "src/veld/events.rs");
    for word in ["FlagProvider", "read_flags", "on_flag_error", "FlagView"] {
        assert!(
            !events.contains(word),
            "`{word}` in a crate with no flagged event"
        );
    }
    assert!(!text(&plain, "src/bookings/desk_events.rs").contains("flags:"));
}

#[test]
fn a_flagged_invariant_of_an_event_is_evaluated_with_the_provider_of_the_options() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_text(read(CONTRACT));
    let ws = work_dir().join("events-flags");
    let _ = std::fs::remove_dir_all(&ws);
    write_files(&ws.join("server"), &server_crate(&ir));
    let harness = ws.join("harness");
    std::fs::create_dir_all(harness.join("src/bin")).unwrap();
    std::fs::write(
        harness.join("Cargo.toml"),
        "[package]\nname = \"events-harness\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\nevents-server = { path = \"../server\" }\nserde_json = \"1\"\n",
    )
    .unwrap();
    std::fs::write(
        harness.join("src/bin/flags.rs"),
        read("tests/toolchain/rust/events/flags.rs"),
    )
    .unwrap();
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"server\", \"harness\"]\n",
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
    let out = run_ok(
        &mut cargo(
            &ws,
            &["run", "-q", "-p", "events-harness", "--bin", "flags"],
        ),
        "the driver flags",
    );
    assert!(out.contains("flags events ok"), "{out}");
}

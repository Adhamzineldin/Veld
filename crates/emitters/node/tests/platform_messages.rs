//! Message catalogs in the generated TypeScript server and client (`spec/targets/typescript.md`
//! section 5l, `spec/platform.md` section 22). The goldens are the workspace of
//! `tests/wire/messages`, loaded the way `crates/veld-rules/tests/messages.rs` loads it: through
//! `compile_with_catalogs`, with the catalogs of its `messages` directory.
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::toolchain::*;
use common::*;
use serde_json::json;
use veld_compiler::messages::{CatalogFile, Catalogs, MemberCatalogs, compile_with_catalogs};
use veld_compiler::{CompileInput, PackageInput, SourceFile};
use veld_ir::{Ir, PackageKind};

fn workspace() -> PathBuf {
    repo_root().join("tests/wire/messages/workspace")
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

fn catalogs() -> Catalogs {
    let mut files: Vec<CatalogFile> = std::fs::read_dir(workspace().join("messages"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .map(|p| CatalogFile {
            name: p.file_name().unwrap().to_str().unwrap().to_owned(),
            text: read(&p),
        })
        .collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    [(
        "bookings".to_owned(),
        MemberCatalogs {
            dir: "messages".into(),
            files,
        },
    )]
    .into()
}

/// The service appended to the contract of the goldens: its body is a `Booking`, so that the
/// invariants of a `Booking` can be sent (they are only an output of the contract itself).
const INTAKE: &str = "
@auth(none)
service Intake at \"/intake\" {
  post \"/\" intake(body: Booking) -> 201 Booking
}
";

fn compile_workspace(extra: &str, with_catalogs: bool) -> Ir {
    let text = format!(
        "{}{extra}",
        read(&workspace().join("contracts/bookings.veld"))
    );
    let out = compile_with_catalogs(
        &CompileInput {
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
        },
        None,
        &BTreeMap::new(),
        &if with_catalogs {
            catalogs()
        } else {
            Catalogs::new()
        },
    );
    assert!(
        out.diagnostics.iter().all(|d| !d.is_error()),
        "{}",
        out.render()
    );
    out.ir.unwrap()
}

fn http_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_workspace(INTAKE, true);
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Bookings", "Intake"],
        &["messages_http.ts"],
        edition,
        &json!({}),
    );
    std::fs::copy(
        repo_root().join("tests/wire/messages/cases.json"),
        work.join("cases.json"),
    )
    .unwrap();
    let embedded = &ir
        .packages
        .iter()
        .find(|p| p.name == "bookings")
        .unwrap()
        .messages;
    std::fs::write(
        work.join("catalogs.json"),
        serde_json::to_string(embedded).unwrap(),
    )
    .unwrap();
    run(
        std::process::Command::new("node")
            .arg("out/messages_http.js")
            .current_dir(work),
        "node messages_http.js",
    );
}

/// Every header row, text row and key row of `tests/wire/messages` through the generated server
/// and client: the choice of the language, the text, `messageKey` and `Content-Language` of the
/// answer of each kind of rule, the hook's event, the client's catalogs and keys.
#[test]
fn the_message_goldens_hold_over_the_production_handler_and_the_client_es2020() {
    http_run("es2020", "messages-2020-");
}

#[test]
fn the_message_goldens_hold_over_the_production_handler_and_the_client_es2022() {
    http_run("es2022", "messages-2022-");
}

#[test]
fn snapshots_of_a_server_and_a_client_with_catalogs() {
    let ir = compile_workspace("", true);
    let server = gen_server_with(&ir, "Bookings", &[], &json!({}));
    insta::assert_snapshot!("messages_server_data", text(&server, "messages.ts"));
    insta::assert_snapshot!(
        "messages_server_module",
        text(&server, "server/bookings.ts")
    );
    insta::assert_snapshot!(
        "messages_server_policy",
        text(&server, "policies/bookings.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!(
        "messages_client_data",
        text(&client, "bookings/messages.ts")
    );
    insta::assert_snapshot!("messages_client_barrel", text(&client, "index.ts"));
}

/// The catalogs are data in a file of their own, sorted, with the keys of the rules next to the
/// ids; the server hands them to the router core and the policy verdict carries the key of each
/// `forbid` that has a message. A client has the catalogs and the keys and none of the server's
/// code.
#[test]
fn the_catalogs_and_their_keys_are_generated_from_the_ir() {
    let ir = compile_workspace("", true);
    let server = gen_server_with(&ir, "Bookings", &[], &json!({}));
    let data = text(&server, "messages.ts");
    for line in [
        "export const messages:",
        "export const messageKeys:",
        "export const fixedMessageKeys =",
        "'veld:bookings/Bookings.pay.version@",
        "ar: {",
        "'pt-br': {",
    ] {
        assert!(data.contains(line), "`{line}` is missing from messages.ts");
    }
    let module = text(&server, "server/bookings.ts");
    assert!(
        module.contains(
            "messages: { catalogs: messages, keys: messageKeys, fixed: fixedMessageKeys }"
        )
    );
    let policy = text(&server, "policies/bookings.ts");
    assert!(
        policy.contains("key: 'bookings/Manage.forbid1'"),
        "{policy}"
    );
    assert!(
        policy.contains("key: 'bookings/Manage.forbid3'"),
        "{policy}"
    );
    // The forbid without a message has no key of its own: the fixed text is used.
    assert_eq!(policy.matches("key: '").count(), 2, "{policy}");
    let runtime = text(&server, "runtime/veld.ts");
    for word in [
        "export function chooseLanguage(",
        "export function acceptedLanguages(",
        "export function catalogText(",
        "function withMessages(",
        "function localizeViolation(",
        "function withContentLanguage(",
    ] {
        assert!(runtime.contains(word), "`{word}` is missing");
    }
    let client = gen_client(&ir);
    let client_data = text(&client, "bookings/messages.ts");
    assert!(client_data.contains("export const messageKeys:"));
    assert!(!client_data.contains("fixedMessageKeys"));
    let client_runtime = text(&client, "runtime/veld.ts");
    assert!(client_runtime.contains("export function chooseLanguage("));
    assert!(!client_runtime.contains("function withMessages("));
    assert!(text(&client, "bookings/client.ts").contains("keys: messageKeys,"));
}

/// Without catalogs nothing changes: the same contract compiled with no catalog has no
/// `messages.ts`, no `messageKey`, no header and none of the words of the feature, in the server
/// and in the client.
#[test]
fn a_member_without_catalogs_has_none_of_the_code() {
    let ir = compile_workspace(INTAKE, false);
    let server = gen_server_with(&ir, "Bookings", &[], &json!({}));
    let client = gen_client(&ir);
    for (name, files) in [("server", &server), ("client", &client)] {
        assert!(
            !files.paths().any(|p| p.ends_with("messages.ts")),
            "{name} has a catalog module"
        );
        for path in files.paths() {
            let body = String::from_utf8_lossy(files.get(path).unwrap_or_default()).into_owned();
            for word in [
                "messageKey",
                "MessageCatalogs",
                "chooseLanguage",
                "catalogText",
                "withContentLanguage",
                "content-language",
                "fixedMessageKeys",
            ] {
                assert!(!body.contains(word), "{name}: `{word}` is in {path}");
            }
        }
    }
}

/// A member with catalogs and no rule has nothing to translate: its catalogs are data, and the
/// runtime keeps the bytes it has without rules.
#[test]
fn catalogs_without_rules_are_data_only() {
    let mut ir = common::standard_ir();
    for p in ir.packages.iter_mut().filter(|p| p.name == "orders") {
        p.messages
            .entry("ar".to_owned())
            .or_default()
            .insert("veld/transition".to_owned(), "x".to_owned());
        p.messages_dir = Some("messages".to_owned());
    }
    let plain = gen_server_with(&standard_ir(), "Orders", &["Iam"], &json!({}));
    let with = gen_server_with(&ir, "Orders", &["Iam"], &json!({}));
    assert_eq!(
        text(&plain, "runtime/veld.ts"),
        text(&with, "runtime/veld.ts"),
        "the runtime has no catalog code without a rule"
    );
    assert!(text(&with, "messages.ts").contains("'veld/transition': 'x'"));
}

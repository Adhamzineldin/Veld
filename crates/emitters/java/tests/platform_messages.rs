//! Message catalogs in the generated Java server and client (`spec/platform.md` section 22,
//! `spec/targets/java.md` section 18.3). The goldens are the workspace of `tests/wire/messages`,
//! loaded the way `crates/veld-rules/tests/messages.rs` loads it: through `compile_with_catalogs`,
//! with the catalogs of its `messages` directory.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::live::require_javac;
use common::p4::{compiles, generate, run_driver_with};
use common::*;
use serde_json::json;
use veld_codegen::Role;
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

/// Every header row, text row and key row of `tests/wire/messages` through the generated server and
/// client: the choice of the language, the text, `messageKey` and `Content-Language` of the answer
/// of each kind of rule, the hook's event, the client's catalogs and keys.
#[test]
fn the_message_goldens_hold_over_the_production_handler_and_the_client() {
    if !require_javac("the_message_goldens_hold_over_the_production_handler_and_the_client") {
        return;
    }
    let ir = compile_workspace(INTAKE, true);
    let server = generate(&ir, Role::Server, "gen.b", &json!({}));
    let client = generate(&ir, Role::Client, "gen.c", &json!({}));
    let embedded = &ir
        .packages
        .iter()
        .find(|p| p.name == "bookings")
        .unwrap()
        .messages;
    let catalogs_json = serde_json::to_string(embedded).unwrap();
    run_driver_with(
        &[&server, &client],
        "MessagesHttp",
        &[("tests/wire/messages/cases.json", "cases.json")],
        &[("catalogs.json", &catalogs_json)],
        &[],
    );
}

#[test]
fn snapshots_of_a_server_and_a_client_with_catalogs() {
    let ir = compile_workspace("", true);
    let server = generate(&ir, Role::Server, "gen.b", &json!({}));
    for (name, path) in [
        ("server_Messages", "gen/b/bookings/Messages.java"),
        ("server_Hooks", "gen/b/bookings/server/BookingsHooks.java"),
        ("server_Policies", "gen/b/bookings/bookings/Policies.java"),
        ("server_Catalogs", "gen/b/runtime/Catalogs.java"),
        ("server_Violation", "gen/b/runtime/Violation.java"),
    ] {
        insta::assert_snapshot!(format!("messages_{name}"), text(&server, path));
    }
    let client = generate(&ir, Role::Client, "gen.c", &json!({}));
    insta::assert_snapshot!(
        "messages_client_Messages",
        text(&client, "gen/c/bookings/Messages.java")
    );
    insta::assert_snapshot!(
        "messages_client_BookingsClient",
        text(&client, "gen/c/bookings/client/BookingsClient.java")
    );
}

/// The catalogs are data in a class of their own, sorted, with the keys of the rules next to the
/// ids; the server hands them to the hooks and the policy verdict carries the key of each `forbid`
/// that has a message. A client has the catalogs and the keys and none of the server's code.
#[test]
fn the_catalogs_and_their_keys_are_generated_from_the_ir() {
    let ir = compile_workspace("", true);
    let server = generate(&ir, Role::Server, "gen.b", &json!({}));
    let data = text(&server, "gen/b/bookings/Messages.java");
    for line in [
        "public static java.util.Map<String, java.util.Map<String, String>> catalogs()",
        "public static java.util.Map<String, String> keys()",
        "public static final String TRANSITION = \"veld/transition\";",
        "public static final String NOT_PERMITTED = \"veld/not-permitted\";",
        "out.put(\"ar\", catalog0());",
        "out.put(\"pt-br\", catalog1());",
        "out.put(\"veld:bookings/Bookings.pay.version@",
    ] {
        assert!(
            data.contains(line),
            "`{line}` is missing from Messages.java"
        );
    }
    let hooks = text(&server, "gen/b/bookings/server/BookingsHooks.java");
    assert!(
        hooks.contains("builder.messages(Catalogs.support(Messages.catalogs(), Messages.keys()));"),
        "{hooks}"
    );
    let policies = text(&server, "gen/b/bookings/bookings/Policies.java");
    assert!(
        policies.contains(", \"bookings/Manage.forbid1\");"),
        "{policies}"
    );
    assert!(
        policies.contains(", \"bookings/Manage.forbid3\");"),
        "{policies}"
    );
    // The forbid without a message has no key of its own: the fixed text is used.
    assert_eq!(policies.matches("Verdict.denied(").count(), 3, "{policies}");
    assert_eq!(policies.matches("\"bookings/Manage.forbid").count(), 2);
    // The fixed keys of the runtime are the four of the IR crate.
    let catalogs = text(&server, "gen/b/runtime/Catalogs.java");
    for (name, key) in [
        ("TRANSITION", veld_ir::message_keys::TRANSITION),
        (
            "TRANSITION_NOT_DUE",
            veld_ir::message_keys::TRANSITION_NOT_DUE,
        ),
        ("VERSION", veld_ir::message_keys::VERSION),
        ("NOT_PERMITTED", veld_ir::message_keys::NOT_PERMITTED),
    ] {
        assert!(
            catalogs.contains(&format!("static final String {name} = \"{key}\";")),
            "{name}"
        );
    }
    // A client has the data and the keys of its local check, never the server's answer.
    let client = generate(&ir, Role::Client, "gen.c", &json!({}));
    let data = text(&client, "gen/c/bookings/Messages.java");
    assert!(data.contains("catalogs()") && data.contains("keys()"));
    assert!(
        !data.contains("TRANSITION"),
        "the fixed keys are the server's"
    );
    let core = text(&client, "gen/c/bookings/client/BookingsClient.java");
    assert!(core.contains("this.core.messageKeys(Messages.keys());"));
    let runtime = text(&client, "gen/c/runtime/Catalogs.java");
    assert!(runtime.contains("public static String chooseLanguage("));
    assert!(
        !runtime.contains("static VeldResponse within("),
        "the client does not answer"
    );
    assert!(!text(&client, "gen/c/runtime/HandlerHooks.java").contains("Catalogs.Support"));
}

/// The same contract without catalogs has none of it: no class, no key, no header, the files of
/// before.
#[test]
fn a_contract_without_catalogs_has_none_of_the_code() {
    let ir = compile_workspace(INTAKE, false);
    for role in [Role::Server, Role::Client] {
        let files = generate(&ir, role, "gen.x", &json!({}));
        for (path, bytes) in files.iter() {
            let body = String::from_utf8_lossy(bytes);
            assert!(
                !path.ends_with("/Messages.java") && !path.ends_with("/Catalogs.java"),
                "{path}"
            );
            for word in ["messageKey", "Catalogs", "content-language", "messages_"] {
                assert!(!body.contains(word), "`{word}` is in {path}");
            }
        }
    }
}

/// A member with catalogs next to captures, the platform features and the timed moves: the files
/// compile with `-Xlint:all -Werror`. Nothing is run.
#[test]
fn the_catalogs_compile_next_to_the_other_features() {
    if !require_javac("the_catalogs_compile_next_to_the_other_features") {
        return;
    }
    let ir = compile_workspace(INTAKE, true);
    for extra in [
        json!({}),
        json!({ "capture-violations": true }),
        json!({ "testing": true }),
    ] {
        let server = generate(&ir, Role::Server, "gen.b", &extra);
        compiles(&[&server]);
    }
    let client = generate(&ir, Role::Client, "gen.c", &json!({}));
    compiles(&[&client]);
    // Both roles in one program: the runtime of each is its own.
    let server = generate(&ir, Role::Server, "gen.b", &json!({}));
    compiles(&[&server, &client]);
}

/// A member with catalogs and no rule at all has the data and none of the runtime code that
/// answers: there is no violation to translate.
#[test]
fn a_member_with_catalogs_and_no_rule_has_the_data_only() {
    if !require_javac("a_member_with_catalogs_and_no_rule_has_the_data_only") {
        return;
    }
    let out = compile_with_catalogs(
        &CompileInput {
            packages: vec![PackageInput {
                name: "items".into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: "items.veld".into(),
                    text: "model Item {\n  name: string\n}\n\nservice Items at \"/items\" {\n  post \"/\" create(body: Item) -> 201 Item\n}\n".into(),
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        },
        None,
        &BTreeMap::new(),
        &[(
            "items".to_owned(),
            MemberCatalogs {
                dir: "messages".into(),
                files: vec![CatalogFile {
                    name: "ar.toml".into(),
                    text: "\"veld/not-permitted\" = \"x\"\n".into(),
                }],
            },
        )]
        .into(),
    );
    let ir = out.ir.unwrap();
    let server = generate(&ir, Role::Server, "gen.b", &json!({}));
    let data = text(&server, "gen/b/items/Messages.java");
    assert!(
        data.contains("catalogs()") && !data.contains("TRANSITION"),
        "{data}"
    );
    assert!(
        !text(&server, "gen/b/runtime/Catalogs.java").contains("static VeldResponse within("),
        "nothing to translate"
    );
    let client = generate(&ir, Role::Client, "gen.c", &json!({}));
    compiles(&[&server]);
    compiles(&[&client]);
}

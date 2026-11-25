//! Policies and the principal in the generated TypeScript (`spec/targets/typescript.md` section
//! 5e): snapshots of the policy functions, the server module, the test handler and the client with
//! `can<Endpoint>`; `tsc --strict` on all of them, in every framework adapter; every example of
//! `tests/rules/endpoints/policies.veld.pending` sent to the generated test handler; and the
//! production handler over HTTP.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;
use veld_ir::Ir;

const CONTRACT: &str = "tests/rules/endpoints/policies.veld.pending";

fn policy_ir() -> Ir {
    compile_path(CONTRACT)
}

fn testing() -> serde_json::Value {
    json!({ "testing": true })
}

#[test]
fn snapshots_of_policies() {
    let ir = policy_ir();
    let server = gen_server_with(&ir, "Bookings", &[], &testing());
    insta::assert_snapshot!(
        "policies_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("policies_server", text(&server, "server/bookings.ts"));
    insta::assert_snapshot!(
        "policies_server_policies",
        text(&server, "policies/bookings.ts")
    );
    insta::assert_snapshot!(
        "policies_test_handler",
        text(&server, "server/bookings-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!(
        "policies_client_file_list",
        client.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("policies_client", text(&client, "bookings/client.ts"));
    insta::assert_snapshot!(
        "policies_client_policies",
        text(&client, "bookings/policies/bookings.ts")
    );
}

#[test]
fn a_contract_without_policies_gets_no_policy_code() {
    let ir = rules_ir();
    let server = gen_server(&ir, "Bookings", &[]);
    assert!(!server.paths().any(|p| p.contains("policies")));
    let runtime = text(&server, "runtime/veld.ts");
    for word in ["checkAllows", "PolicyVerdict", "guarded", "refusal"] {
        assert!(!runtime.contains(word), "`{word}` leaked into the runtime");
    }
}

#[test]
fn a_policy_no_allow_names_generates_nothing_and_an_unread_principal_changes_no_bytes() {
    let full = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    // The examples need the principal and the allows; the point here is the declarations.
    let source = full
        .split("// --- permit and forbid ---")
        .next()
        .unwrap()
        .to_string();
    // Drop both `allow` lines of `getBooking` and `payBooking`: the policies are named by nobody.
    let without: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("allow "))
        .collect::<Vec<_>>()
        .join("\n");
    let ir = compile_text(without);
    let server = gen_server(&ir, "Bookings", &[]);
    assert!(
        !server.paths().any(|p| p.contains("policies")),
        "no policy file"
    );
    assert!(!text(&server, "runtime/veld.ts").contains("checkAllows"));
    let hooks = text(&server, "server/bookings.ts");
    assert!(hooks.contains("Promise<unknown | null>"), "{hooks}");
    // The same contract with no principal at all generates the same server module.
    let bare = compile_text(
        source
            .lines()
            .filter(|l| !l.trim_start().starts_with("allow ") && !l.starts_with("principal "))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let other = gen_server(&bare, "Bookings", &[]);
    assert_eq!(
        text(&server, "server/bookings.ts"),
        text(&other, "server/bookings.ts")
    );
}

#[test]
fn the_auth_hooks_are_typed_by_the_principal_in_every_adapter() {
    let ir = policy_ir();
    for framework in [
        None,
        Some("express"),
        Some("fastify"),
        Some("hono"),
        Some("node-http"),
        Some("nest"),
    ] {
        let server = gen_server_fw(&ir, "Bookings", &[], &serde_json::Value::Null, framework);
        let module = text(&server, "server/bookings.ts");
        assert!(
            module.contains(
                "bearer: (token: string, ctx: veld.RequestContext) => Promise<User | null>"
            ),
            "{framework:?}: {module}"
        );
    }
}

#[test]
fn can_is_in_the_client_and_takes_what_is_read() {
    let ir = policy_ir();
    let client = gen_client(&ir);
    let module = text(&client, "bookings/client.ts");
    assert!(module.contains("static canPayBooking(booking: Booking, user: User): boolean"));
    // Facts first, then the principal, then the parameters.
    assert!(module.contains("static canGetBooking(booking: Booking, user: User): boolean"));
}

fn examples_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = policy_ir();
    let guard = scratch(prefix);
    let work = guard.path();
    build(
        work,
        &tsc,
        &ir,
        &["policies_serve.ts", "policies_http.ts"],
        edition,
    );
    {
        let server = Live::start(work, "policies_serve.js");
        let passed = run_examples(&server.url, &ir);
        assert_eq!(passed, 10);
        assert_eq!(ir.examples.len(), 10, "every example ran, none was skipped");
    }
    run(
        std::process::Command::new("node")
            .arg("out/policies_http.js")
            .current_dir(work),
        "node policies_http.js",
    );
}

#[test]
fn policies_compile_strict_and_pass_every_example_es2020() {
    examples_run("es2020", "policies-2020-");
}

#[test]
fn policies_compile_strict_and_pass_every_example_es2022() {
    examples_run("es2022", "policies-2022-");
}

/// Every guard on one endpoint: two `allow` lines (one reads a request parameter), a transition,
/// and a `require` and an `ensure` that read the caller.
const GUARDED: &str = r#"
enum Stage {
  draft, live, gone
  flow draft -> live -> gone
  final gone
}

model User {
  id: uuid
  level: int
}

principal User

model Item {
  id: uuid
  owner: uuid
  status: Stage = .draft
  note: string
}

policy Tier(user: User, need: int) {
  permit when user.level >= need
}

policy Owns(user: User, item: Item) {
  permit when item.owner == user.id
}

@auth(bearer)
service Bookings at "/items" {
  post "/{id}/publish/{need}" publish(id: uuid, need: int) -> Item {
    fact item: Item = id
    allow Tier(user, need)
    allow Owns(user, item)
    transition item.status: draft -> live
    require user.level > 0
    ensure result.owner == user.id
  }
}
"#;

#[test]
fn every_guard_together_compiles_and_agrees_with_can_over_http() {
    let ir = compile_text(GUARDED.to_string());
    let server = gen_server(&ir, "Bookings", &[]);
    let module = text(&server, "server/bookings.ts");
    // An `allow` argument that reads a parameter gives that parameter in `fields`.
    assert!(module.contains("fields: ['need'],"), "{module}");
    let client = gen_client(&ir);
    assert!(
        text(&client, "bookings/client.ts")
            .contains("static canPublish(item: Item, user: User, need: number): boolean"),
        "facts, then the caller, then the parameters"
    );
    let Some(tsc) = tsc_path() else { return };
    let guard = scratch("guarded-");
    let work = guard.path();
    build(work, &tsc, &ir, &["guarded_http.ts"], "es2020");
    run(
        std::process::Command::new("node")
            .arg("out/guarded_http.js")
            .current_dir(work),
        "node guarded_http.js",
    );
}

#[test]
fn the_nest_server_with_a_typed_principal_compiles_with_the_decorator_flags() {
    let Some(tsc) = tsc_path() else { return };
    let ir = policy_ir();
    let server = gen_server_fw(&ir, "Bookings", &[], &serde_json::Value::Null, Some("nest"));
    let guard = scratch("policies-nest-");
    let work = guard.path();
    veld_project::output::write(
        &work.join("server"),
        &server,
        &veld_project::output::WriteOptions::default(),
    )
    .unwrap();
    std::fs::write(
        work.join("tsconfig.json"),
        serde_json::to_string_pretty(&json!({
            "compilerOptions": {
                "strict": true, "target": "ES2020", "lib": ["ES2020"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "noEmit": true,
                "experimentalDecorators": true, "emitDecoratorMetadata": true,
                "skipLibCheck": true
            },
            "include": ["server/**/*.ts"]
        }))
        .unwrap(),
    )
    .unwrap();
    run(
        std::process::Command::new("node")
            .arg(tsc)
            .args(["-p", "tsconfig.json"])
            .current_dir(work),
        "tsc nest",
    );
}

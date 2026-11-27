//! Flags in the generated TypeScript (`spec/targets/typescript.md` section 5j,
//! `spec/platform.md` section 11).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;
use veld_project::output::{WriteOptions, write};

const EXAMPLES: &str = "tests/rules/endpoints/flags.veld.pending";
const CONTRACT: &str = "tests/toolchain/contract/platform_p2/flags.veld";
const RULES: &str = "tests/toolchain/contract/platform_p2/flags_rules.veld";
const EVENTS: &str = "tests/toolchain/contract/platform_p2/flags_events.veld";

fn examples_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(EXAMPLES);
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Rooms", &["flags_serve.ts"], edition);
    let server = Live::start(work, "flags_serve.js");
    let passed = run_examples(&server.url, &ir);
    assert_eq!(passed, 14);
    assert_eq!(ir.examples.len(), 14, "every example ran, none was skipped");
}

#[test]
fn the_flag_examples_compile_strict_and_pass_es2020() {
    examples_run("es2020", "flags-2020-");
}

#[test]
fn the_flag_examples_compile_strict_and_pass_es2022() {
    examples_run("es2022", "flags-2022-");
}

fn driver_run(contract: &str, service: &str, driver: &str, edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(contract);
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, service, &[driver], edition);
    let js = format!("out/{}", driver.replace(".ts", ".js"));
    run(
        std::process::Command::new("node")
            .arg(&js)
            .current_dir(work),
        &format!("node {js}"),
    );
}

#[test]
fn flags_behave_over_the_production_handler_and_the_client_es2020() {
    driver_run(
        CONTRACT,
        "Rooms",
        "flags_http.ts",
        "es2020",
        "flags-http-2020-",
    );
}

#[test]
fn flags_behave_over_the_production_handler_and_the_client_es2022() {
    driver_run(
        CONTRACT,
        "Rooms",
        "flags_http.ts",
        "es2022",
        "flags-http-2022-",
    );
}

#[test]
fn flagged_invariants_behave_over_the_production_handler_and_the_local_check_es2020() {
    driver_run(
        RULES,
        "Visits",
        "flags_rules_http.ts",
        "es2020",
        "flags-rules-2020-",
    );
}

#[test]
fn flagged_invariants_behave_over_the_production_handler_and_the_local_check_es2022() {
    driver_run(
        RULES,
        "Visits",
        "flags_rules_http.ts",
        "es2022",
        "flags-rules-2022-",
    );
}

/// The publisher and the consumer of an event take the provider in their options.
fn events_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(EVENTS);
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Desk", "Floor"],
        &["flags_events.ts"],
        edition,
        &json!({}),
    );
    run(
        std::process::Command::new("node")
            .arg("out/flags_events.js")
            .current_dir(work),
        "node flags_events.js",
    );
}

#[test]
fn a_flagged_invariant_of_an_event_is_evaluated_by_publisher_and_consumer_es2020() {
    events_run("es2020", "flags-events-2020-");
}

#[test]
fn a_flagged_invariant_of_an_event_is_evaluated_by_publisher_and_consumer_es2022() {
    events_run("es2022", "flags-events-2022-");
}

/// The React and Vue helpers call `can<Endpoint>` and the policy functions: the optional last
/// argument must not break them. A React and a Vue client of the flag contract compile under
/// `tsc --strict`.
#[test]
fn a_react_and_a_vue_client_of_a_flagged_policy_and_edge_compile() {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let guard = scratch("flags-react-");
    let work = guard.path();
    let opts = WriteOptions::default();
    for (dir, framework, version) in [("client_react", "react", "18"), ("client_vue", "vue", "3")] {
        let options = json!({ "framework-version": version });
        write(
            &work.join(dir),
            &gen_client_fw(&ir, &options, Some(framework)),
            &opts,
        )
        .unwrap();
    }
    std::fs::write(
        work.join("tsconfig.json"),
        json!({
            "compilerOptions": {
                "strict": true, "target": "ES2020", "lib": ["ES2020", "dom"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "noEmit": true, "jsx": "react-jsx"
            },
            "include": ["client_react/**/*.ts", "client_vue/**/*.ts"]
        })
        .to_string(),
    )
    .unwrap();
    run(
        std::process::Command::new("node")
            .arg(&tsc)
            .args(["-p", "tsconfig.json"])
            .current_dir(work),
        "tsc the React and Vue clients",
    );
}

#[test]
fn snapshots_of_a_contract_with_flags() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Rooms", &[], &json!({ "testing": true }));
    insta::assert_snapshot!(
        "flags_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("flags_server_module", text(&server, "server/rooms.ts"));
    insta::assert_snapshot!(
        "flags_test_handler",
        text(&server, "server/rooms-test-handler.ts")
    );
    insta::assert_snapshot!(
        "flags_server_flags_runtime",
        text(&server, "runtime/flags.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!("flags_client", text(&client, "bookings/client.ts"));
    insta::assert_snapshot!(
        "flags_client_policies",
        text(&client, "bookings/policies/bookings.ts")
    );
    let rules = compile_path(RULES);
    let models = gen_client(&rules);
    insta::assert_snapshot!(
        "flags_rules_models",
        text(&models, "bookings/models/bookings.ts")
    );
}

#[test]
fn the_names_of_the_spec_are_there() {
    let ir = compile_path(CONTRACT);
    let server = gen_server(&ir, "Rooms", &[]);
    let module = text(&server, "server/rooms.ts");
    for word in [
        "flags: veld.FlagProvider;",
        "flags: ['betaDesk', 'lockSuspended', 'openOnly'],",
        "flag: 'openOnly',",
        "flagged: { live: 'archiving' },",
    ] {
        assert!(module.contains(word), "`{word}` is missing in:\n{module}");
    }
    let client = gen_client(&ir);
    let can = text(&client, "bookings/client.ts");
    assert!(can.contains("flags?: (name: string) => boolean"));
    // A flagged contract: the statements are written in the models and policies.
    let rules = gen_client(&compile_path(RULES));
    assert!(text(&rules, "bookings/models/bookings.ts").contains("invariantOn('smallGroups')"));
}

#[test]
fn a_contract_without_flags_has_none_of_the_code() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"]);
    let client = gen_client(&ir);
    for (name, files) in [("server", &server), ("client", &client)] {
        assert!(files.get("runtime/flags.ts").is_none(), "{name}");
        for path in files.paths() {
            let body = String::from_utf8_lossy(files.get(path).unwrap_or_default()).into_owned();
            for word in [
                "veldFlags",
                "FlagProvider",
                "readFlags",
                "flagged(",
                "invariantOn",
                "withReadings",
                "hooks.flags",
            ] {
                assert!(!body.contains(word), "{name}: `{word}` is in {path}");
            }
        }
    }
}

/// A flagged statement next to timed moves, deadlines and an `@slo`, idempotency, derived fields
/// and an audited rule: the runtime of each combination compiles under `tsc --strict`.
#[test]
fn flags_compile_next_to_the_other_features() {
    let Some(tsc) = tsc_path() else { return };
    let base = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let combos = [
        "",
        "\nmodel Memo {\n  note: string\n  shout: string = note\n}\n",
        "\nmodel Audited {\n  n: int\n\n  @rollout(audit)\n  rule small: n < 5\n}\n",
    ];
    for extra in combos {
        let ir = compile_text(format!("{base}{extra}"));
        for (edition, _) in TARGETS {
            let guard = scratch("flags-combo-");
            build_service(guard.path(), &tsc, &ir, "Rooms", &[], edition);
        }
    }
}

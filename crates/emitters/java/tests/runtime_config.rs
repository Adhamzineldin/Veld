//! The typed runtime config of the Java generator (`spec/runtime-config.md`, `spec/targets/java.md`
//! section 14): the shape of the generated classes, the entries, the gate for a target without
//! settings, the goldens of `tests/wire/runtime-config/cases.json` through the generated check, and
//! the compile check of the Spring properties class.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;

use common::live::{java_main, javac_ok, require_javac, write_files};
use common::*;
use serde_json::{Value, json};
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, with_target_name};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_java::Java;
use veld_ir::{Ir, PackageKind};

/// A package of the contracts below: one service, one model.
fn package(name: &str, service: &str, path: &str, rule: bool) -> PackageInput {
    let ensure = if rule {
        "{\n    ensure same: result.id == id\n  }"
    } else {
        ""
    };
    let text = format!(
        "model Pong {{\n  id: uuid\n}}\n\nservice {service} at \"/{path}\" {{\n  get \"/{{id}}\" ping(id: uuid) -> Pong {ensure}\n}}\n"
    );
    PackageInput {
        name: name.into(),
        kind: PackageKind::Service,
        files: vec![SourceFile {
            path: format!("{service}.veld"),
            text,
        }],
        base_url: None,
    }
}

/// Packages and who consumes whom, compiled.
fn workspace(packages: Vec<PackageInput>, consumes: &[(&str, &[&str])]) -> Ir {
    let out = compile(&CompileInput {
        packages,
        consumes: consumes
            .iter()
            .map(|(k, v)| {
                (
                    (*k).to_string(),
                    v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    });
    let rendered = out.render();
    out.ir
        .unwrap_or_else(|| panic!("does not compile:\n{rendered}"))
}

fn ids(ir: &Ir, packages: &[&str]) -> Vec<veld_ir::ServiceId> {
    ir.services
        .iter()
        .filter(|s| packages.contains(&ir.package(s.package).name.as_str()))
        .map(|s| s.id)
        .collect()
}

fn generate(
    ir: &Ir,
    role: Role,
    (services, consumes): (&[&str], &[&str]),
    (target, package, framework): (&str, &str, Option<&str>),
    extra: Value,
) -> Result<FileSet, Vec<veld_codegen::GenDiagnostic>> {
    let mut opts = json!({ "package": package });
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        opts[k] = v;
    }
    let req = GenRequest {
        ir,
        role,
        services: ids(ir, services),
        consumes: ids(ir, consumes),
        framework: framework.map(str::to_string),
    };
    Java.generate_dyn(&req, &with_target_name(&opts, target))
}

/// The server `bookings` of the file `cases.json`: consumes iam and payments, has rules.
fn bookings_ir() -> Ir {
    workspace(
        vec![
            package("iam", "Iam", "iam", false),
            package("payments", "Payments", "payments", false),
            package("bookings", "Bookings", "bookings", true),
        ],
        &[("bookings", &["iam", "payments"])],
    )
}

fn bookings(framework: Option<&str>, extra: Value) -> FileSet {
    generate(
        &bookings_ir(),
        Role::Server,
        (&["bookings"], &["iam", "payments"]),
        ("bookings", "gen.k.server", framework),
        extra,
    )
    .unwrap()
}

#[test]
fn the_runtime_module_of_a_consuming_server_with_rules() {
    let files = bookings(None, json!({}));
    let base = "gen/k/server/runtimeconfig";
    insta::assert_snapshot!(
        "runtime_server_BookingsRuntime",
        text(&files, &format!("{base}/BookingsRuntime.java"))
    );
    insta::assert_snapshot!(
        "runtime_server_BookingsClients",
        text(&files, &format!("{base}/BookingsClients.java"))
    );
    insta::assert_snapshot!(
        "runtime_server_RuntimeCheck",
        text(&files, "gen/k/server/runtime/RuntimeCheck.java")
    );
    insta::assert_snapshot!(
        "runtime_server_BookingsHooks",
        text(&files, "gen/k/server/bookings/server/BookingsHooks.java")
    );
    insta::assert_snapshot!(
        "runtime_server_BookingsHandler",
        text(&files, "gen/k/server/bookings/server/BookingsHandler.java")
    );
    insta::assert_snapshot!(
        "runtime_server_ClientCore",
        text(&files, "gen/k/server/runtime/ClientCore.java")
    );
}

#[test]
fn the_runtime_module_of_a_client_target() {
    let ir = bookings_ir();
    let files = generate(
        &ir,
        Role::Client,
        (&["bookings", "iam"], &[]),
        ("web", "gen.k.web", None),
        json!({}),
    )
    .unwrap();
    let base = "gen/k/web/runtimeconfig";
    insta::assert_snapshot!(
        "runtime_client_WebRuntime",
        text(&files, &format!("{base}/WebRuntime.java"))
    );
    insta::assert_snapshot!(
        "runtime_client_WebClients",
        text(&files, &format!("{base}/WebClients.java"))
    );
    insta::assert_snapshot!(
        "runtime_client_BookingsClient",
        text(&files, "gen/k/web/bookings/client/BookingsClient.java")
    );
    insta::assert_snapshot!(
        "runtime_client_ClientOptions",
        text(&files, "gen/k/web/runtime/ClientOptions.java")
    );
    let client = text(&files, "gen/k/web/iam/client/IamClient.java");
    assert!(
        !client.contains("VELD_"),
        "no environment variable: {client}"
    );
    assert!(
        !client.contains("public IamClient()"),
        "a client has no constructor without options"
    );
    assert!(!text(&files, "gen/k/web/runtime/ClientCore.java").contains("System.getenv"));
}

#[test]
fn the_spring_properties_class_binds_the_settings_and_hands_them_to_the_check() {
    let files = bookings(Some("spring"), json!({}));
    let props = text(
        &files,
        "gen/k/server/adapters/BookingsRuntimeProperties.java",
    );
    insta::assert_snapshot!("runtime_spring_BookingsRuntimeProperties", props);
    assert!(props.contains("prefix = \"veld\""));
    insta::assert_snapshot!(
        "runtime_spring_BookingsController",
        text(
            &files,
            "gen/k/server/adapters/bookings/BookingsController.java"
        )
    );
    let custom = bookings(Some("spring"), json!({ "runtime-prefix": "acme.cfg" }));
    assert!(
        text(
            &custom,
            "gen/k/server/adapters/BookingsRuntimeProperties.java"
        )
        .contains("prefix = \"acme.cfg\"")
    );
    assert!(
        files
            .get("gen/k/server/adapters/BookingsRuntimeProperties.java")
            .is_some()
            && bookings(None, json!({}))
                .get("gen/k/server/adapters/BookingsRuntimeProperties.java")
                .is_none(),
        "only the spring framework gets the properties class"
    );
}

#[test]
fn a_bad_runtime_prefix_and_a_missing_target_name_are_refused() {
    let ir = bookings_ir();
    let bad = generate(
        &ir,
        Role::Server,
        (&["bookings"], &["iam", "payments"]),
        ("bookings", "gen.k.server", Some("spring")),
        json!({ "runtime-prefix": "Not A Prefix" }),
    )
    .unwrap_err();
    assert_eq!(bad[0].code, "E0802");
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: ids(&ir, &["bookings"]),
        consumes: ids(&ir, &["iam", "payments"]),
        framework: None,
    };
    let unnamed = Java
        .generate_dyn(&req, &json!({ "package": "gen.k.server" }))
        .unwrap_err();
    assert_eq!(
        unnamed[0].code, "E0802",
        "a target with settings needs its name"
    );
}

#[test]
fn the_entry_takes_the_runtime_config_last_and_it_is_optional_without_required_paths() {
    let handler = |files: &FileSet, path: &str| text(files, path).to_string();
    let required = bookings(None, json!({}));
    let h = handler(
        &required,
        "gen/k/server/bookings/server/BookingsHandler.java",
    );
    assert!(
        h.contains("BookingsHooks hooks, BookingsRuntime runtime)"),
        "{h}"
    );
    assert!(
        !h.contains(
            "create(BookingsService impl, BookingsFactLoaders loaders, BookingsHooks hooks)"
        ) && !h.contains("create(BookingsService impl, BookingsHooks hooks) {"),
        "a runtime config with required paths is not optional: {h}"
    );
    assert!(h.contains("BookingsRuntime config = BookingsRuntime.ensure(runtime);"));
    assert!(h.contains("hooks.build(config)"));
    let hooks = text(&required, "gen/k/server/bookings/server/BookingsHooks.java");
    assert!(
        !hooks.contains("rulesOutput(RuleWalk.OutputMode"),
        "the mode moved: {hooks}"
    );

    // Only rules: no required path, so the old entries stay and the config is optional.
    let ir = workspace(vec![package("solo", "Solo", "solo", true)], &[]);
    let files = generate(
        &ir,
        Role::Server,
        (&["solo"], &[]),
        ("solo", "gen.k.solo", None),
        json!({}),
    )
    .unwrap();
    let h = handler(&files, "gen/k/solo/solo/server/SoloHandler.java");
    for shape in [
        "create(SoloService impl, SoloHooks hooks, SoloRuntime runtime)",
        "create(SoloService impl, SoloHooks hooks)",
        "create(SoloService impl, SoloRuntime runtime)",
        "create(SoloService impl)",
    ] {
        assert!(h.contains(shape), "missing `{shape}`:\n{h}");
    }
    insta::assert_snapshot!("runtime_rules_only_SoloHandler", h);
}

#[test]
fn a_target_without_settings_keeps_its_bytes() {
    let ir = workspace(vec![package("plain", "Plain", "plain", false)], &[]);
    let files = generate(
        &ir,
        Role::Server,
        (&["plain"], &[]),
        ("plain", "gen.k.plain", Some("spring")),
        json!({}),
    )
    .unwrap();
    assert!(
        files
            .paths()
            .all(|p| !p.contains("runtimeconfig") && !p.contains("RuntimeProperties")),
        "{:?}",
        files.paths().collect::<Vec<_>>()
    );
    assert!(files.get("gen/k/plain/runtime/RuntimeCheck.java").is_none());
    assert!(
        text(&files, "gen/k/plain/runtime/ClientCore.java")
            .contains("System.getenv(defaults.envVar())"),
        "the runtime of a target without settings is the one of before"
    );
    let h = text(&files, "gen/k/plain/plain/server/PlainHandler.java");
    assert!(h.contains("create(PlainService impl)") && !h.contains("Runtime"));
}

#[test]
fn the_output_mode_exists_only_where_the_model_says_so() {
    // A contract with rules on a consumed service only: the old predicate said "rules", the model
    // does not give this server a `rules.output`, and its hooks keep `rulesOutput`.
    let ir = workspace(
        vec![
            package("ruled", "Ruled", "ruled", true),
            package("host", "Host", "host", false),
        ],
        &[("host", &["ruled"])],
    );
    let files = generate(
        &ir,
        Role::Server,
        (&["host"], &["ruled"]),
        ("host", "gen.k.host", None),
        json!({}),
    )
    .unwrap();
    let runtime = text(&files, "gen/k/host/runtimeconfig/HostRuntime.java");
    assert!(!runtime.contains("rulesOutput"), "{runtime}");
    assert!(text(&files, "gen/k/host/host/server/HostHooks.java").contains("rulesOutput("));
}

// -- the goldens ------------------------------------------------------------------------------

/// The four models of `cases.json`, generated from contracts: `(id, target, base package)`.
struct Fixture {
    id: &'static str,
    class: String,
    files: FileSet,
    getters: BTreeMap<String, String>,
}

fn fixtures() -> Vec<Fixture> {
    let ir = bookings_ir();
    let ordering = workspace(
        vec![
            package("a-b", "Beta", "b", false),
            package("a", "Alpha", "a", false),
            package("card-service", "Cards", "cards", false),
        ],
        &[("card-service", &["a-b", "a"])],
    );
    let orders = workspace(vec![package("orders", "Orders", "orders", true)], &[]);
    let make = |ir: &Ir, id, role, sel: (&[&str], &[&str]), target: &str, package| {
        let files = generate(ir, role, sel, (target, package, None), json!({})).unwrap();
        let class = format!("{package}.runtimeconfig.{}", runtime_class(target));
        let getters = getters_of(ir, role, sel, target);
        Fixture {
            id,
            package,
            class,
            files,
            getters,
        }
    };
    vec![
        make(
            &ir,
            "server",
            Role::Server,
            (&["bookings"], &["iam", "payments"]),
            "bookings",
            "gen.k.server",
        ),
        make(
            &ir,
            "client",
            Role::Client,
            (&["bookings", "iam"], &[]),
            "web",
            "gen.k.client",
        ),
        make(
            &ordering,
            "ordering",
            Role::Server,
            (&["card-service"], &["a-b", "a"]),
            "card-service",
            "gen.k.ordering",
        ),
        make(
            &orders,
            "rules_only",
            Role::Server,
            (&["orders"], &[]),
            "orders",
            "gen.k.rules",
        ),
    ]
}

fn runtime_class(target: &str) -> String {
    veld_codegen::runtime_config::runtime_type_name(target)
}

/// The getter of each path of the model of a target.
fn getters_of(
    ir: &Ir,
    role: Role,
    (services, consumes): (&[&str], &[&str]),
    target: &str,
) -> BTreeMap<String, String> {
    let settings = veld_codegen::runtime_config::settings_for(
        ir,
        role,
        &ids(ir, services),
        &ids(ir, consumes),
    );
    let model = veld_codegen::runtime_config::RuntimeModel::from_settings(target, settings);
    let names = veld_emit_java::config::member_names(&model);
    model
        .settings()
        .iter()
        .zip(names)
        .map(|(s, n)| (s.path.clone(), n))
        .collect()
}

#[test]
fn the_generated_models_have_the_settings_of_the_golden_file() {
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("tests/wire/runtime-config/cases.json")).unwrap(),
    )
    .unwrap();
    for f in fixtures() {
        let model = &golden["models"][f.id];
        let paths: Vec<&str> = model["settings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["path"].as_str().unwrap())
            .collect();
        let have: Vec<&str> = f.getters.keys().map(String::as_str).collect();
        assert_eq!(have, paths, "model {}", f.id);
    }
}

#[test]
fn the_goldens_of_the_runtime_config_through_the_generated_check() {
    if !require_javac("the_goldens_of_the_runtime_config") {
        return;
    }
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("tests/wire/runtime-config/cases.json")).unwrap(),
    )
    .unwrap();
    let fixtures = fixtures();
    let work = tempfile::tempdir().unwrap();
    let mut sources = Vec::new();
    for f in &fixtures {
        sources.extend(write_files(work.path(), &f.files));
    }
    sources.push(repo_root().join("tests/toolchain/java/runtime-config/RuntimeConfigGolden.java"));
    javac_ok(work.path(), &sources, "17");
    let cases = golden["cases"].as_array().unwrap();
    let mut lines = String::new();
    for (id, case) in cases.iter().enumerate() {
        let f = fixtures
            .iter()
            .find(|f| f.id == case["model"].as_str().unwrap())
            .unwrap();
        let mut line = json!({
            "id": id, "class": f.class, "from": case["from"], "getters": f.getters,
        });
        for key in ["input", "names", "env"] {
            if case.get(key).is_some() {
                line[key] = case[key].clone();
            }
        }
        lines.push_str(&line.to_string());
        lines.push('\n');
    }
    std::fs::write(work.path().join("cases.jsonl"), lines).unwrap();
    let out = java_main(
        work.path(),
        "veldtest.RuntimeConfigGolden",
        &["cases.jsonl"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let answers: Vec<Value> = stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(answers.len(), cases.len(), "{stdout}");
    for (case, answer) in cases.iter().zip(&answers) {
        let name = case["name"].as_str().unwrap();
        let expect = &case["expect"];
        assert!(answer.get("failure").is_none(), "{name}: {answer}");
        match expect.get("values") {
            Some(values) => assert_eq!(&answer["values"], values, "{name}"),
            None => {
                assert_eq!(answer["error"], expect["error"], "{name}");
                let got: Vec<(&str, &str)> = answer["problems"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| (p["path"].as_str().unwrap(), p["message"].as_str().unwrap()))
                    .collect();
                let want: Vec<(&str, &str)> = expect["problems"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| (p["path"].as_str().unwrap(), p["message"].as_str().unwrap()))
                    .collect();
                assert_eq!(got, want, "{name}");
            }
        }
    }
}

#[test]
fn the_url_goldens_through_the_generated_url_rule() {
    if !require_javac("the_url_goldens") {
        return;
    }
    // `urls` of cases.json: the rule is the one of `RuntimeCheck.normalizeUrl`, driven through the
    // `from: map` path of the client model.
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("tests/wire/runtime-config/cases.json")).unwrap(),
    )
    .unwrap();
    let f = fixtures().into_iter().find(|f| f.id == "client").unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &f.files);
    sources.push(repo_root().join("tests/toolchain/java/runtime-config/RuntimeConfigGolden.java"));
    // The driver imports the `gen.k.server` runtime: generate that too so the program compiles.
    let server = fixtures().into_iter().find(|f| f.id == "server").unwrap();
    sources.extend(write_files(work.path(), &server.files));
    javac_ok(work.path(), &sources, "17");
    let urls = golden["urls"].as_array().unwrap();
    let mut lines = String::new();
    for (id, u) in urls.iter().enumerate() {
        let input = json!({
            "services.bookings.url": u["input"],
            "services.iam.url": "http://iam",
        });
        let line = json!({
            "id": id, "class": f.class, "from": "map", "input": input, "getters": f.getters,
        });
        lines.push_str(&format!("{line}\n"));
    }
    std::fs::write(work.path().join("urls.jsonl"), lines).unwrap();
    let out = java_main(work.path(), "veldtest.RuntimeConfigGolden", &["urls.jsonl"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    for (u, line) in urls.iter().zip(stdout.lines()) {
        let answer: Value = serde_json::from_str(line).unwrap();
        match u["normalized"].as_str() {
            Some(n) => assert_eq!(
                answer["values"]["services.bookings.url"], n,
                "{}",
                u["input"]
            ),
            None => assert!(
                answer.get("error").is_some(),
                "{} must be refused",
                u["input"]
            ),
        }
    }
}

// -- compile checks ---------------------------------------------------------------------------

#[test]
fn the_generated_runtime_config_compiles_without_warnings() {
    if !require_javac("the_generated_runtime_config_compiles") {
        return;
    }
    for f in fixtures() {
        let work = tempfile::tempdir().unwrap();
        let sources = write_files(work.path(), &f.files);
        javac_ok(work.path(), &sources, "17");
    }
}

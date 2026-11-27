//! Snapshots, determinism, signature conformance and capability checks for the generated Java.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeSet;

use common::*;
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_java::Java;
use veld_ir::{DeclKind, Feature, Ir, PackageKind};

fn files_of(fs: &FileSet) -> Vec<(String, Vec<u8>)> {
    fs.iter()
        .map(|(p, b)| (p.to_string(), b.to_vec()))
        .collect()
}

fn server() -> FileSet {
    gen_server(&standard_ir(), "Orders", &["Iam"], "gen.a", None)
}

fn spring_server() -> FileSet {
    gen_server(&standard_ir(), "Orders", &["Iam"], "gen.a", Some("spring"))
}

fn client() -> FileSet {
    gen_client(&standard_ir(), "gen.c")
}

#[test]
fn client_snapshots() {
    let fs = client();
    insta::assert_snapshot!(
        "client_file_list",
        fs.paths().collect::<Vec<_>>().join("\n")
    );
    for path in [
        "gen/c/orders/orders/Order.java",
        "gen/c/orders/orders/OutOfStock.java",
        "gen/c/orders/catalog/Payment.java",
        "gen/c/orders/catalog/Card.java",
        "gen/c/orders/catalog/Status.java",
        "gen/c/orders/catalog/Money.java",
        "gen/c/orders/client/OrdersClient.java",
        "gen/c/orders/client/CreateOrderRequest.java",
        "gen/c/prelude/NotFound.java",
    ] {
        insta::assert_snapshot!(
            format!("client_{}", path.replace('/', "__")),
            text(&fs, path)
        );
    }
}

#[test]
fn server_snapshots() {
    let fs = server();
    insta::assert_snapshot!(
        "server_file_list",
        fs.paths().collect::<Vec<_>>().join("\n")
    );
    for path in [
        "gen/a/orders/server/OrdersService.java",
        "gen/a/orders/server/OrdersHandler.java",
        "gen/a/orders/server/OrdersHooks.java",
        "gen/a/orders/server/CreateOrderRequest.java",
        "gen/a/adapters/JdkHttpServerAdapter.java",
        "gen/a/runtime/Veld.java",
        "gen/a/runtime/VeldError.java",
    ] {
        insta::assert_snapshot!(
            format!("server_{}", path.replace('/', "__")),
            text(&fs, path)
        );
    }
}

#[test]
fn spring_server_snapshots() {
    let fs = spring_server();
    insta::assert_snapshot!(
        "spring_server_file_list",
        fs.paths().collect::<Vec<_>>().join(
            "
"
        )
    );
    for path in [
        "gen/a/adapters/SpringAdapter.java",
        "gen/a/adapters/orders/OrdersController.java",
    ] {
        insta::assert_snapshot!(
            format!("spring_server_{}", path.replace('/', "__")),
            text(&fs, path)
        );
    }
}

#[test]
fn output_is_deterministic() {
    let ir = standard_ir();
    assert_eq!(
        files_of(&gen_client(&ir, "gen.c")),
        files_of(&gen_client(&ir, "gen.c"))
    );
    assert_eq!(files_of(&server()), files_of(&server()));
    assert_eq!(files_of(&spring_server()), files_of(&spring_server()));
    assert_eq!(
        files_of(&client()),
        files_of(&gen_client(&standard_ir(), "gen.c"))
    );
}

#[test]
fn runtime_is_identical_in_both_roles_modulo_package() {
    let ir = standard_ir();
    let c = gen_client(&ir, "gen.x");
    let s = gen_server(&ir, "Orders", &["Iam"], "gen.x", None);
    for (path, bytes) in c.iter().filter(|(p, _)| p.starts_with("gen/x/runtime/")) {
        assert_eq!(s.get(path), Some(bytes), "{path}");
    }
}

fn quoted_sigs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("\"veld:") {
        let tail = &rest[i + 1..];
        let end = tail.find('"').unwrap();
        out.push(tail[..end].to_string());
        rest = &tail[end..];
    }
    out
}

#[test]
fn signatures_equal_the_ir_byte_for_byte() {
    let ir = standard_ir();
    let known: BTreeSet<&str> = ir
        .decls
        .iter()
        .map(|d| d.sig.as_str())
        .chain(
            ir.services
                .iter()
                .flat_map(|s| s.endpoints.iter().map(|e| e.sig.as_str())),
        )
        .collect();
    for fs in [client(), server()] {
        let mut found: BTreeSet<String> = BTreeSet::new();
        for (path, bytes) in fs.iter().filter(|(p, _)| !p.contains("/runtime/")) {
            for sig in quoted_sigs(std::str::from_utf8(bytes).unwrap()) {
                assert!(
                    known.contains(sig.as_str()),
                    "{path}: signature {sig} is not in the IR"
                );
                found.insert(sig);
            }
        }
        for d in ir
            .decls
            .iter()
            .filter(|d| !matches!(d.kind, DeclKind::Const(_)))
        {
            assert!(
                found.contains(&d.sig),
                "no generated file carries the signature of {}",
                d.qualified
            );
        }
        for e in ir
            .services
            .iter()
            .filter(|s| s.name == "Orders")
            .flat_map(|s| &s.endpoints)
        {
            assert!(
                found.contains(&e.sig),
                "no signature for endpoint {}",
                e.name
            );
        }
    }
}

fn sse_ir() -> Ir {
    let src = "service Feed at \"/feed\" {\n  get \"/\" events() -> stream string\n}\n";
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "app".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "app.veld".into(),
                text: src.into(),
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    out.ir.unwrap()
}

fn streaming_server(framework: Option<&str>, ir: &Ir) -> FileSet {
    gen_server_services(ir, &["Live", "Vault"], "gen.s", framework, None)
}

fn streaming_client() -> FileSet {
    gen_client(&streaming_ir(), "gen.sf")
}

/// What `generate` answers for a server request, without the capability check of `common::run`.
fn server_request(
    ir: &Ir,
    services: &[&str],
    framework: Option<&str>,
) -> Result<FileSet, Vec<veld_codegen::GenDiagnostic>> {
    let req = GenRequest {
        ir,
        role: Role::Server,
        services: services.iter().map(|n| service_id(ir, n)).collect(),
        consumes: vec![],
        framework: framework.map(str::to_string),
    };
    Java.generate_dyn(
        &req,
        &veld_codegen::with_target_name(&serde_json::Value::Null, "t"),
    )
}

/// SSE generates for the client and for every server framework; `ws` generates for the client and
/// for Spring, and is `E0801` (with a message that says what to do) for the JDK adapter, which has
/// no WebSocket server. The capability gate itself accepts both features.
#[test]
fn streaming_generates_except_ws_on_the_jdk_adapter() {
    let sse = sse_ir();
    let svc = sse.services[0].id;
    assert!(sse.features_used(&[svc]).contains(&Feature::StreamingSse));
    for role in [Role::Client, Role::Server] {
        let req = GenRequest {
            ir: &sse,
            role,
            services: vec![svc],
            consumes: vec![],
            framework: None,
        };
        assert!(check_capabilities(&sse, &req, &Java.capabilities(role)).is_empty());
        assert!(
            Java.generate_dyn(
                &req,
                &veld_codegen::with_target_name(&serde_json::Value::Null, "t")
            )
            .is_ok()
        );
    }
    for framework in [None, Some("jdk"), Some("spring")] {
        let files = server_request(&sse, &["Feed"], framework).unwrap();
        let path = files
            .paths()
            .find(|p| p.ends_with("/FeedService.java"))
            .unwrap();
        let service = text(&files, path);
        assert!(
            service.contains(
                "Stream<SseEvent<String>> events(EventsRequest request, RequestContext ctx)"
            ),
            "{service}"
        );
    }

    let full = streaming_ir();
    let live = service_id(&full, "Live");
    assert!(full.features_used(&[live]).contains(&Feature::StreamingWs));
    for framework in [None, Some("jdk")] {
        let diags = server_request(&full, &["Live", "Vault"], framework).unwrap_err();
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].code, "E0801");
        assert!(diags[0].message.contains("ws"), "{}", diags[0].message);
        assert!(diags[0].message.contains("spring"), "{}", diags[0].message);
    }
    let spring = streaming_server(Some("spring"), &full);
    let paths: Vec<&str> = spring.paths().collect();
    for expected in [
        "gen/s/adapters/SpringAdapter.java",
        "gen/s/adapters/SpringWebSocketAdapter.java",
        "gen/s/adapters/live/LiveController.java",
        "gen/s/adapters/live/LiveWebSocketConfig.java",
        "gen/s/adapters/live/VaultWebSocketConfig.java",
    ] {
        assert!(paths.contains(&expected), "{expected} in {paths:?}");
    }
    assert!(!paths.iter().any(|p| p.contains("JdkHttpServerAdapter")));
    assert!(
        gen_client(&full, "gen.c")
            .get("gen/c/live/client/LiveClient.java")
            .is_some()
    );
}

/// The WebSocket adapter, and with it the Spring WebSocket dependency, exists only when the contract
/// has `ws` endpoints.
#[test]
fn the_spring_websocket_adapter_only_for_contracts_with_ws() {
    let sse = server_request(&sse_ir(), &["Feed"], Some("spring")).unwrap();
    assert!(
        sse.paths().all(|p| !p.contains("WebSocket")),
        "SSE needs no WebSocket"
    );
    let plain = spring_server();
    assert!(plain.paths().all(|p| !p.contains("WebSocket")));
    for set in [&sse, &plain] {
        for path in set.paths() {
            assert!(
                !text(set, path).contains("org.springframework.web.socket"),
                "{path}"
            );
        }
    }
}

#[test]
fn streaming_snapshots() {
    let full = streaming_ir();
    let jdk = streaming_server(None, &streaming_sse_ir());
    let spring = streaming_server(Some("spring"), &full);
    let client = streaming_client();
    for (set, label, paths) in [
        (
            &jdk,
            "jdk",
            vec![
                "gen/s/live/server/LiveService.java",
                "gen/s/live/server/LiveHandler.java",
                "gen/s/live/server/VaultHandler.java",
                "gen/s/adapters/JdkHttpServerAdapter.java",
            ],
        ),
        (
            &spring,
            "spring",
            vec![
                "gen/s/live/server/LiveService.java",
                "gen/s/live/server/LiveHandler.java",
                "gen/s/live/server/VaultService.java",
                "gen/s/live/server/ChatRequest.java",
                "gen/s/adapters/SpringAdapter.java",
                "gen/s/adapters/SpringWebSocketAdapter.java",
                "gen/s/adapters/live/LiveController.java",
                "gen/s/adapters/live/LiveWebSocketConfig.java",
            ],
        ),
        (
            &client,
            "client",
            vec![
                "gen/sf/live/client/LiveClient.java",
                "gen/sf/live/client/VaultClient.java",
                "gen/sf/live/client/ChatRequest.java",
                "gen/sf/runtime/SseEvent.java",
                "gen/sf/runtime/SseParser.java",
                "gen/sf/runtime/WsChannel.java",
                "gen/sf/runtime/WsSession.java",
            ],
        ),
    ] {
        insta::assert_snapshot!(
            format!("streaming_{label}_file_list"),
            set.paths().collect::<Vec<_>>().join("\n")
        );
        for path in paths {
            insta::assert_snapshot!(
                format!("streaming_{label}_{}", path.replace('/', "__")),
                text(set, path)
            );
        }
    }
}

#[test]
fn streaming_output_is_deterministic() {
    let full = streaming_ir();
    let sse = streaming_sse_ir();
    for framework in [None, Some("spring")] {
        let ir = if framework.is_some() { &full } else { &sse };
        assert_eq!(
            files_of(&streaming_server(framework, ir)),
            files_of(&streaming_server(framework, ir))
        );
    }
    assert_eq!(files_of(&streaming_client()), files_of(&streaming_client()));
}

#[test]
fn role_framework_and_options_are_checked() {
    let ir = standard_ir();
    let orders = service_id(&ir, "Orders");
    let req = |role, framework: Option<&str>| GenRequest {
        ir: &ir,
        role,
        services: vec![orders],
        consumes: vec![],
        framework: framework.map(str::to_string),
    };
    let null = veld_codegen::with_target_name(&serde_json::Value::Null, "t");
    assert_eq!(
        Java.generate_dyn(&req(Role::Server, Some("koa")), &null)
            .unwrap_err()[0]
            .code,
        "E0803"
    );
    assert_eq!(
        Java.generate_dyn(&req(Role::Client, Some("spring")), &null)
            .unwrap_err()[0]
            .code,
        "E0803"
    );
    assert!(
        Java.generate_dyn(&req(Role::Server, Some("jdk")), &null)
            .is_ok()
    );
    assert_eq!(
        Java.generate_dyn(&req(Role::Server, None), &serde_json::json!({ "nope": 1 }))
            .unwrap_err()[0]
            .code,
        "E0802"
    );
    assert_eq!(
        Java.generate_dyn(
            &req(Role::Server, None),
            &serde_json::json!({ "package": "1bad" })
        )
        .unwrap_err()[0]
            .code,
        "E0802"
    );
}

#[test]
fn spring_adapter_only_with_the_spring_framework() {
    let ir = standard_ir();
    assert!(
        gen_server(&ir, "Orders", &[], "gen.a", None)
            .get("gen/a/adapters/SpringAdapter.java")
            .is_none()
    );
    assert!(
        gen_server(&ir, "Orders", &[], "gen.a", Some("spring"))
            .get("gen/a/adapters/SpringAdapter.java")
            .is_some()
    );
    assert!(
        gen_server(&ir, "Orders", &[], "gen.a", None)
            .get("gen/a/adapters/JdkHttpServerAdapter.java")
            .is_some()
    );
}

#[test]
fn language_and_framework_versions_are_validated_centrally() {
    let ir = standard_ir();
    let orders = service_id(&ir, "Orders");
    let req = |role, framework: Option<&str>| GenRequest {
        ir: &ir,
        role,
        services: vec![orders],
        consumes: vec![],
        framework: framework.map(str::to_string),
    };
    let err = |role, framework, options: serde_json::Value| {
        Java.generate_dyn(&req(role, framework), &options)
            .unwrap_err()
            .remove(0)
    };
    let bad_java = err(Role::Client, None, serde_json::json!({ "java": "11" }));
    assert_eq!(bad_java.code, "E0802");
    assert!(bad_java.message.contains("17, 21"), "{}", bad_java.message);
    let bad_version = err(
        Role::Server,
        Some("spring"),
        serde_json::json!({ "framework-version": "2" }),
    );
    assert_eq!(bad_version.code, "E0803");
    assert!(bad_version.message.contains("expected one of: 3"));
    let unversioned = err(
        Role::Server,
        Some("jdk"),
        serde_json::json!({ "framework-version": "1" }),
    );
    assert_eq!(unversioned.code, "E0803");
    // Valid combinations generate, and the options never reach the typed `Options`.
    for java in ["17", "21"] {
        let options = veld_codegen::with_target_name(
            &serde_json::json!({ "java": java, "framework-version": "3" }),
            "t",
        );
        assert!(
            Java.generate_dyn(&req(Role::Server, Some("spring")), &options)
                .is_ok(),
            "java {java}"
        );
    }
}

/// `java = "21"` does not change the output: the profile only gates newer syntax, and the
/// generated code is the same for every supported release.
#[test]
fn java_release_does_not_change_the_default_output() {
    let ir = standard_ir();
    let run_with = |options: serde_json::Value| {
        run(
            &ir,
            Role::Client,
            ir.services.iter().map(|s| s.id).collect(),
            vec![],
            None,
            options,
        )
    };
    let named = |v: serde_json::Value| veld_codegen::with_target_name(&v, "t");
    let base = run_with(named(serde_json::json!({ "package": "gen.a" })));
    let v21 = run_with(named(
        serde_json::json!({ "package": "gen.a", "java": "21" }),
    ));
    assert_eq!(base, v21);
}

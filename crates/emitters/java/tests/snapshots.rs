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
    fs.iter().map(|(p, b)| (p.to_string(), b.to_vec())).collect()
}

fn server() -> FileSet {
    gen_server(&standard_ir(), "Orders", &["Iam"], "gen.a", Some("spring"))
}

fn client() -> FileSet {
    gen_client(&standard_ir(), "gen.c")
}

#[test]
fn client_snapshots() {
    let fs = client();
    insta::assert_snapshot!("client_file_list", fs.paths().collect::<Vec<_>>().join("\n"));
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
        insta::assert_snapshot!(format!("client_{}", path.replace('/', "__")), text(&fs, path));
    }
}

#[test]
fn server_snapshots() {
    let fs = server();
    insta::assert_snapshot!("server_file_list", fs.paths().collect::<Vec<_>>().join("\n"));
    for path in [
        "gen/a/orders/server/OrdersService.java",
        "gen/a/orders/server/OrdersHandler.java",
        "gen/a/orders/server/OrdersHooks.java",
        "gen/a/orders/server/CreateOrderRequest.java",
        "gen/a/adapters/JdkHttpServerAdapter.java",
        "gen/a/adapters/SpringAdapter.java",
        "gen/a/runtime/Veld.java",
        "gen/a/runtime/VeldError.java",
    ] {
        insta::assert_snapshot!(format!("server_{}", path.replace('/', "__")), text(&fs, path));
    }
}

#[test]
fn output_is_deterministic() {
    let ir = standard_ir();
    assert_eq!(files_of(&gen_client(&ir, "gen.c")), files_of(&gen_client(&ir, "gen.c")));
    assert_eq!(files_of(&server()), files_of(&server()));
    assert_eq!(files_of(&client()), files_of(&gen_client(&standard_ir(), "gen.c")));
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
        .chain(ir.services.iter().flat_map(|s| s.endpoints.iter().map(|e| e.sig.as_str())))
        .collect();
    for fs in [client(), server()] {
        let mut found: BTreeSet<String> = BTreeSet::new();
        for (path, bytes) in fs.iter().filter(|(p, _)| !p.contains("/runtime/")) {
            for sig in quoted_sigs(std::str::from_utf8(bytes).unwrap()) {
                assert!(known.contains(sig.as_str()), "{path}: signature {sig} is not in the IR");
                found.insert(sig);
            }
        }
        for d in ir.decls.iter().filter(|d| !matches!(d.kind, DeclKind::Const(_))) {
            assert!(found.contains(&d.sig), "no generated file carries the signature of {}", d.qualified);
        }
        for e in ir.services.iter().filter(|s| s.name == "Orders").flat_map(|s| &s.endpoints) {
            assert!(found.contains(&e.sig), "no signature for endpoint {}", e.name);
        }
    }
}

fn sse_ir() -> Ir {
    let src = "service Feed at \"/feed\" {\n  get \"/\" events() -> stream string\n}\n";
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "app".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile { path: "app.veld".into(), text: src.into() }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    out.ir.unwrap()
}

#[test]
fn streaming_reports_e0801() {
    let ir = sse_ir();
    let svc = ir.services[0].id;
    assert!(ir.features_used(&[svc]).contains(&Feature::StreamingSse));
    for role in [Role::Client, Role::Server] {
        let req = GenRequest { ir: &ir, role, services: vec![svc], consumes: vec![], framework: None };
        let diags = check_capabilities(&ir, &req, &Java.capabilities(role));
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].code, "E0801");
        assert_eq!(Java.generate_dyn(&req, &serde_json::Value::Null).unwrap_err()[0].code, "E0801");
    }
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
    let null = serde_json::Value::Null;
    assert_eq!(Java.generate_dyn(&req(Role::Server, Some("koa")), &null).unwrap_err()[0].code, "E0803");
    assert_eq!(Java.generate_dyn(&req(Role::Client, Some("spring")), &null).unwrap_err()[0].code, "E0803");
    assert!(Java.generate_dyn(&req(Role::Server, Some("jdk")), &null).is_ok());
    assert_eq!(Java.generate_dyn(&req(Role::Server, None), &serde_json::json!({ "nope": 1 })).unwrap_err()[0].code, "E0802");
    assert_eq!(Java.generate_dyn(&req(Role::Server, None), &serde_json::json!({ "package": "1bad" })).unwrap_err()[0].code, "E0802");
}

#[test]
fn spring_adapter_only_with_the_spring_framework() {
    let ir = standard_ir();
    assert!(gen_server(&ir, "Orders", &[], "gen.a", None).get("gen/a/adapters/SpringAdapter.java").is_none());
    assert!(gen_server(&ir, "Orders", &[], "gen.a", Some("spring")).get("gen/a/adapters/SpringAdapter.java").is_some());
    assert!(gen_server(&ir, "Orders", &[], "gen.a", None).get("gen/a/adapters/JdkHttpServerAdapter.java").is_some());
}

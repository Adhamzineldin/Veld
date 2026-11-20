//! Snapshots, determinism, signature conformance and capability checks for the generated TypeScript.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeSet;

use common::*;
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_node::Node;
use veld_emit_typescript::{TypeScript, runtime};
use veld_ir::{Feature, Ir, PackageKind};

fn files_of(fs: &FileSet) -> Vec<(String, Vec<u8>)> {
    fs.iter()
        .map(|(p, b)| (p.to_string(), b.to_vec()))
        .collect()
}

#[test]
fn client_snapshots() {
    let ir = standard_ir();
    let fs = gen_client(&ir);
    insta::assert_snapshot!(
        "client_file_list",
        fs.paths().collect::<Vec<_>>().join("\n")
    );
    for path in [
        "index.ts",
        "prelude.ts",
        "orders/models/catalog.ts",
        "orders/models/orders.ts",
        "orders/errors/orders.ts",
        "orders/client.ts",
        "iam/client.ts",
    ] {
        insta::assert_snapshot!(
            format!("client_{}", path.replace('/', "__")),
            text(&fs, path)
        );
    }
}

#[test]
fn server_snapshots() {
    let ir = standard_ir();
    let fs = gen_server(&ir, "Orders", &["Iam"]);
    insta::assert_snapshot!(
        "server_file_list",
        fs.paths().collect::<Vec<_>>().join("\n")
    );
    for path in [
        "index.ts",
        "server/orders.ts",
        "models/orders.ts",
        "clients/iam/index.ts",
    ] {
        insta::assert_snapshot!(
            format!("server_{}", path.replace('/', "__")),
            text(&fs, path)
        );
    }
}

#[test]
fn output_is_deterministic() {
    let ir = standard_ir();
    assert_eq!(files_of(&gen_client(&ir)), files_of(&gen_client(&ir)));
    assert_eq!(
        files_of(&gen_server(&ir, "Orders", &["Iam"])),
        files_of(&gen_server(&ir, "Orders", &["Iam"]))
    );
    // Recompiling from the same sources gives the same IR and so the same files.
    assert_eq!(
        files_of(&gen_client(&ir)),
        files_of(&gen_client(&standard_ir()))
    );
}

#[test]
fn runtime_is_byte_identical_in_both_roles() {
    let ir = standard_ir();
    let client = gen_client(&ir);
    let server = gen_server(&ir, "Orders", &["Iam"]);
    assert_eq!(client.get("runtime/veld.ts"), server.get("runtime/veld.ts"));
    assert_eq!(
        client.get("runtime/veld.ts"),
        Some(runtime::SOURCE.as_bytes())
    );
}

/// Model files are the same in both roles; only the relative import specifiers depend on the layout depth.
fn body(text: &str) -> String {
    text.lines()
        .filter(|l| !l.starts_with("import "))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn models_match_across_roles() {
    let ir = standard_ir();
    let client = gen_client(&ir);
    let server = gen_server(&ir, "Orders", &["Iam"]);
    for (c, s) in [
        ("orders/models/catalog.ts", "models/catalog.ts"),
        ("orders/models/orders.ts", "models/orders.ts"),
        ("orders/errors/orders.ts", "errors/orders.ts"),
        ("prelude.ts", "prelude.ts"),
    ] {
        assert_eq!(body(text(&client, c)), body(text(&server, s)), "{c} vs {s}");
    }
}

fn quoted_sigs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("'veld:") {
        let tail = &rest[i + 1..];
        let end = tail.find('\'').unwrap();
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
    for fs in [gen_client(&ir), gen_server(&ir, "Orders", &["Iam"])] {
        let mut found: BTreeSet<String> = BTreeSet::new();
        for (path, bytes) in fs.iter().filter(|(p, _)| !p.starts_with("runtime/")) {
            for sig in quoted_sigs(std::str::from_utf8(bytes).unwrap()) {
                assert!(
                    known.contains(sig.as_str()),
                    "{path}: signature {sig} is not in the IR"
                );
                found.insert(sig);
            }
        }
        // Every declaration that becomes a value (everything except consts) and every
        // endpoint of the generated services carries its signature.
        for d in ir
            .decls
            .iter()
            .filter(|d| !matches!(d.kind, veld_ir::DeclKind::Const(_)))
        {
            assert!(
                found.contains(&d.sig),
                "no generated file carries the signature of {}",
                d.qualified
            );
        }
        for s in ir.services.iter().filter(|s| s.name == "Orders") {
            for e in &s.endpoints {
                assert!(
                    found.contains(&e.sig),
                    "no signature for {}.{}",
                    s.name,
                    e.name
                );
            }
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

#[test]
fn streaming_reports_e0801_through_check_capabilities_and_generate() {
    let ir = sse_ir();
    let svc = ir.services[0].id;
    assert!(ir.features_used(&[svc]).contains(&Feature::StreamingSse));
    for (generator, role) in [
        (&TypeScript as &dyn DynGenerator, Role::Client),
        (&Node as &dyn DynGenerator, Role::Server),
    ] {
        let req = GenRequest {
            ir: &ir,
            role,
            services: vec![svc],
            consumes: vec![],
            framework: None,
        };
        let diags = check_capabilities(&ir, &req, &generator.capabilities(role));
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].code, "E0801");
        assert!(diags[0].origin.is_some());
        let err = generator
            .generate_dyn(&req, &serde_json::Value::Null)
            .unwrap_err();
        assert_eq!(err[0].code, "E0801");
    }
}

#[test]
fn role_and_framework_are_checked() {
    let ir = standard_ir();
    let orders = service_id(&ir, "Orders");
    let wrong_role = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![orders],
        consumes: vec![],
        framework: None,
    };
    assert_eq!(
        TypeScript
            .generate_dyn(&wrong_role, &serde_json::Value::Null)
            .unwrap_err()[0]
            .code,
        "E0803"
    );
    let bad_framework = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![orders],
        consumes: vec![],
        framework: Some("koa".into()),
    };
    assert_eq!(
        Node.generate_dyn(&bad_framework, &serde_json::Value::Null)
            .unwrap_err()[0]
            .code,
        "E0803"
    );
    let ok = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![orders],
        consumes: vec![],
        framework: Some("express".into()),
    };
    assert!(Node.generate_dyn(&ok, &serde_json::Value::Null).is_ok());
    let bad_opts = Node
        .generate_dyn(&ok, &serde_json::json!({ "nope": true }))
        .unwrap_err();
    assert_eq!(bad_opts[0].code, "E0802");
}

#[test]
fn changed_contract_changes_signatures() {
    let a = standard_ir();
    let c = compile_contract("orders", &|s| {
        s.replace(
            "error OutOfStock 409 {\n  sku: string\n}",
            "error OutOfStock 409 {\n  sku: string\n  warehouse: string\n}",
        )
    });
    let sig = |ir: &Ir| {
        ir.decls
            .iter()
            .find(|d| d.name == "OutOfStock")
            .unwrap()
            .sig
            .clone()
    };
    assert_ne!(sig(&a), sig(&c));
}

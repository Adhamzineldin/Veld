//! Snapshots, determinism, signature conformance and capability checks for the generated TypeScript.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeSet;

use common::*;
use veld_codegen::{Capabilities, DynGenerator, FileSet, GenRequest, Role, check_capabilities};
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
    insta::assert_snapshot!("client_package.json", text(&fs, "package.json"));
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
    insta::assert_snapshot!("server_package.json", text(&fs, "package.json"));
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
    // The runtime of a target with settings: the plain runtime with the client code that takes
    // its address from `baseUrl` alone (`spec/runtime-config.md` section 5).
    let expected = runtime::source_gated(runtime::Gates {
        runtime_config: true,
        ..runtime::Gates::default()
    })
    .unwrap();
    assert_eq!(client.get("runtime/veld.ts"), Some(expected.as_bytes()));
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
fn streaming_is_supported_and_the_gate_still_reports_e0801_for_a_generator_without_it() {
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
        let caps = generator.capabilities(role);
        assert!(caps.supports(Feature::StreamingSse) && caps.supports(Feature::StreamingWs));
        assert!(check_capabilities(&ir, &req, &caps).is_empty());
        let files = generator
            .generate_dyn(&req, &named(role, &serde_json::Value::Null))
            .unwrap();
        assert!(files.paths().any(|p| p == "runtime/veld.ts"));
        // A generator that does not declare the capability is still stopped by the gate.
        let without = Capabilities::all_except(&[Feature::StreamingSse]);
        let diags = check_capabilities(&ir, &req, &without);
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].code, "E0801");
        assert!(diags[0].origin.is_some());
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
    assert!(
        Node.generate_dyn(&ok, &named(Role::Server, &serde_json::Value::Null))
            .is_ok()
    );
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

#[test]
fn typescript_edition_is_validated_centrally() {
    let ir = standard_ir();
    let orders = service_id(&ir, "Orders");
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![orders],
        consumes: vec![],
        framework: Some("express".into()),
    };
    let err = Node
        .generate_dyn(&req, &serde_json::json!({ "typescript": "es2015" }))
        .unwrap_err();
    assert_eq!(err[0].code, "E0802");
    assert!(
        err[0].message.contains("es2020, es2022"),
        "{}",
        err[0].message
    );
    let err = Node
        .generate_dyn(&req, &serde_json::json!({ "framework-version": "4" }))
        .unwrap_err();
    assert_eq!(err[0].code, "E0803");
    assert!(err[0].message.contains("no versions"), "{}", err[0].message);
    // The same output for every supported edition.
    let base = Node
        .generate_dyn(&req, &named(Role::Server, &serde_json::Value::Null))
        .unwrap();
    for edition in ["es2020", "es2022"] {
        let files = Node
            .generate_dyn(
                &req,
                &named(Role::Server, &serde_json::json!({ "typescript": edition })),
            )
            .unwrap();
        assert_eq!(base, files, "{edition}");
    }
    let client = GenRequest {
        role: Role::Client,
        framework: None,
        ..req.clone()
    };
    let err = TypeScript
        .generate_dyn(&client, &serde_json::json!({ "typescript": "es5" }))
        .unwrap_err();
    assert_eq!(err[0].code, "E0802");
}

fn without_framework_files(fs: &FileSet, keep: &dyn Fn(&str) -> bool) -> Vec<(String, Vec<u8>)> {
    files_of(fs).into_iter().filter(|(p, _)| keep(p)).collect()
}

#[test]
fn nest_snapshots() {
    let ir = standard_ir();
    let fs = gen_server_fw(
        &ir,
        "Orders",
        &["Iam"],
        &serde_json::Value::Null,
        Some("nest"),
    );
    insta::assert_snapshot!("nest_file_list", fs.paths().collect::<Vec<_>>().join("\n"));
    insta::assert_snapshot!("nest_package.json", text(&fs, "package.json"));
    insta::assert_snapshot!(
        "nest_server__nest__orders.ts",
        text(&fs, "server/nest/orders.ts")
    );
    // Nest adds files and changes nothing else: every other file is what the plain server emits.
    let plain = gen_server(&ir, "Orders", &["Iam"]);
    // (`package.json` differs only by the `@nestjs/common` peer dependency, snapshotted above.)
    assert_eq!(
        without_framework_files(&fs, &|p| !p.starts_with("server/nest/")
            && p != "package.json"),
        without_framework_files(&plain, &|p| p != "package.json")
    );
    // The nest file is the only one that imports a package.
    for (path, bytes) in files_of(&fs) {
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(
            text.contains("from '@nestjs/"),
            path.starts_with("server/nest/"),
            "{path}"
        );
    }
}

#[test]
fn react_and_vue_snapshots() {
    let ir = standard_ir();
    let plain = gen_client(&ir);
    for framework in ["react", "vue"] {
        let fs = gen_client_fw(&ir, &serde_json::Value::Null, Some(framework));
        insta::assert_snapshot!(
            format!("{framework}_file_list"),
            fs.paths().collect::<Vec<_>>().join("\n")
        );
        insta::assert_snapshot!(
            format!("{framework}_package.json"),
            text(&fs, "package.json")
        );
        insta::assert_snapshot!(
            format!("{framework}_orders__{framework}.ts"),
            text(&fs, &format!("orders/{framework}.ts"))
        );
        insta::assert_snapshot!(
            format!("{framework}_runtime__{framework}.ts"),
            text(&fs, &format!("runtime/{framework}.ts"))
        );
        // The plain client output is unchanged; only the framework files are added.
        let own = |p: &str| p.ends_with(&format!("/{framework}.ts"));
        assert_eq!(
            without_framework_files(&fs, &|p| !own(p) && p != "package.json"),
            without_framework_files(&plain, &|p| p != "package.json")
        );
        for (path, bytes) in files_of(&fs) {
            let text = String::from_utf8(bytes).unwrap();
            assert_eq!(
                text.contains(&format!("from '{framework}'")),
                own(&path),
                "{path}"
            );
        }
    }
}

#[test]
fn query_options_snapshots() {
    let ir = standard_ir();
    let plain = gen_client(&ir);
    assert!(
        !plain.paths().any(|p| p.ends_with("queries.ts")),
        "no queries.ts unless `query-options = true`"
    );
    let fs = gen_client_fw(&ir, &serde_json::json!({ "query-options": true }), None);
    insta::assert_snapshot!(
        "query_options_file_list",
        fs.paths().collect::<Vec<_>>().join(
            "
"
        )
    );
    insta::assert_snapshot!(
        "query_options_orders__queries.ts",
        text(&fs, "orders/queries.ts")
    );
    // The plain client output is unchanged; only `queries.ts` and its barrel line are added.
    let own = |p: &str| p.ends_with("/queries.ts");
    let keep = |p: &str| !own(p) && !p.ends_with("index.ts");
    assert_eq!(
        without_framework_files(&fs, &keep),
        without_framework_files(&plain, &keep)
    );
    // Zero runtime dependencies: every import of the generated files is relative.
    for (path, bytes) in files_of(&fs) {
        if !path.ends_with(".ts") {
            continue;
        }
        let text = String::from_utf8(bytes).unwrap();
        for line in text
            .lines()
            .filter(|l| l.starts_with("import ") || l.starts_with("export "))
        {
            if let Some(from) = line.split(" from '").nth(1) {
                assert!(from.starts_with('.'), "{path}: {line}");
            }
        }
    }
    let package = text(&fs, "package.json");
    assert!(!package.contains("tanstack") && !package.contains("peerDependencies"));
}

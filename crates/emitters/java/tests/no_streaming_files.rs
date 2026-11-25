//! A contract without streaming endpoints generates no streaming files in any target, and a
//! contract with one transport gets only that transport's files.
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role};
use veld_emit_conformance::{FIXTURES, compile};
use veld_emit_java::Java;
use veld_emit_node::Node;
use veld_emit_typescript::TypeScript;
use veld_ir::{Feature, Ir, ServiceId};

fn streaming_features(ir: &Ir, services: &[ServiceId]) -> (bool, bool) {
    let used = ir.features_used(services);
    (
        used.contains(&Feature::StreamingSse),
        used.contains(&Feature::StreamingWs),
    )
}

/// Every (role, framework) of a generator: the default framework and each named one.
fn targets(generator: &dyn DynGenerator) -> Vec<(Role, Option<String>)> {
    let mut out = Vec::new();
    for role in generator.roles() {
        out.push((*role, None));
        for info in generator.frameworks(*role) {
            out.push((*role, Some(info.id)));
        }
    }
    out
}

fn generate(
    generator: &dyn DynGenerator,
    ir: &Ir,
    role: Role,
    framework: Option<&str>,
) -> Result<FileSet, String> {
    let services: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    let req = GenRequest {
        ir,
        role,
        services,
        consumes: vec![],
        framework: framework.map(str::to_string),
    };
    generator
        .generate_dyn(&req, &serde_json::Value::Null)
        .map_err(|d| format!("{d:?}"))
}

/// The file name without directories and extension, lower case.
fn stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.split('.').next().unwrap_or(name).to_lowercase()
}

fn is_sse_file(path: &str) -> bool {
    let s = stem(path);
    s.starts_with("sse") || s.contains("sse_") || s.contains("_sse")
}

fn is_ws_file(path: &str) -> bool {
    let s = stem(path);
    s == "ws" || s.starts_with("ws") || s.contains("websocket") || s.contains("_ws")
}

fn is_stream_helper(path: &str) -> bool {
    stem(path).ends_with("_stream")
}

#[test]
fn contracts_without_streaming_generate_no_streaming_files() {
    let generators: [&dyn DynGenerator; 3] = [&Node, &TypeScript, &Java];
    let mut checked = 0;
    for fixture in FIXTURES {
        let ir = compile(fixture.name, fixture.source);
        let services: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
        if streaming_features(&ir, &services) != (false, false) {
            continue;
        }
        let used = ir.features_used(&services);
        for generator in generators {
            for (role, framework) in targets(generator) {
                // A generator that does not declare a feature of the contract refuses it
                // (`E0801`, as a server refuses events today): it writes no file to look at.
                // Any other failure to generate still fails the test below.
                let caps = generator.capabilities(role);
                if used.iter().any(|f| !caps.supports(*f)) {
                    continue;
                }
                let files =
                    generate(generator, &ir, role, framework.as_deref()).unwrap_or_else(|e| {
                        panic!(
                            "{} {role:?} {framework:?} on `{}`: {e}",
                            generator.id(),
                            fixture.name
                        )
                    });
                for (path, bytes) in files.iter() {
                    let at = format!(
                        "{} {role:?} {framework:?} `{}`: {path}",
                        generator.id(),
                        fixture.name
                    );
                    assert!(
                        !is_sse_file(path) && !is_ws_file(path) && !is_stream_helper(path),
                        "a streaming file without streaming: {at}"
                    );
                    if generator.id() == "java" {
                        let text = String::from_utf8_lossy(bytes);
                        assert!(
                            !text.contains("Sse") && !text.contains("WsSession"),
                            "streaming code without streaming: {at}"
                        );
                    }
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "no fixture without streaming was checked");
}

#[test]
fn java_emits_only_the_streaming_files_of_the_transports_in_use() {
    for fixture in FIXTURES {
        let ir = compile(fixture.name, fixture.source);
        let services: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
        let (sse, ws) = streaming_features(&ir, &services);
        for (role, framework) in targets(&Java) {
            // The jdk framework refuses `ws` endpoints (`E0801`); there is nothing to inspect.
            let Ok(files) = generate(&Java, &ir, role, framework.as_deref()) else {
                continue;
            };
            let has = |f: fn(&str) -> bool| files.paths().any(f);
            assert_eq!(
                has(is_sse_file),
                sse,
                "{} {role:?} {framework:?}: sse files",
                fixture.name
            );
            assert_eq!(
                files.paths().any(|p| p.contains("/runtime/Ws")),
                ws,
                "{} {role:?} {framework:?}: ws runtime files",
                fixture.name
            );
        }
    }
}

//! The embedded runtime: real `.java` files, byte-identical in every role except for the
//! package declaration.

use std::collections::BTreeSet;

use veld_codegen::{FileSet, GenDiagnostic, gate_regions};
use veld_ir::Feature;

use crate::tree::java_path;
use crate::util::{HEADER, add_file};

/// The package the embedded sources declare. It is replaced by `<base>.runtime`.
const SOURCE_PACKAGE: &str = "dev.veld.runtime";
/// The runtime classes that exist only for a contract with rules (`spec/targets/java.md` section 8).
const RULE_FILES: &[&str] = &[
    "DigestHandler",
    "RuleError",
    "RuleWalk",
    "Rules",
    "Violation",
];
/// The runtime classes that exist only for a contract with endpoint rules (`spec/targets/java.md` section 9).
const ENDPOINT_FILES: &[&str] = &["EndpointRules"];
/// The runtime classes of the test handler, emitted only with the target option `testing`.
const TESTING_FILES: &[&str] = &["TestSupport"];
/// The runtime classes of the state machines of endpoints (`spec/targets/java.md` section 10).
const MACHINE_FILES: &[&str] = &["Transition"];

/// The package the embedded adapter sources declare. It is replaced by `<base>.adapters`.
const ADAPTER_PACKAGE: &str = "dev.veld.adapters";

macro_rules! runtime_files {
    ($($name:literal),* $(,)?) => {
        /// `(simple name, source)` of every runtime class, in a fixed order.
        pub const RUNTIME: &[(&str, &str)] = &[$(($name, include_str!(concat!("../runtime/", $name, ".java")))),*];
    };
}

runtime_files!(
    "ClientCore",
    "ClientDefaults",
    "ClientOptions",
    "Codec",
    "DigestHandler",
    "EndpointRules",
    "EndpointSpec",
    "HandlerHooks",
    "Issue",
    "Json",
    "JsonParser",
    "JsonWriter",
    "Page",
    "Reconnect",
    "RequestContext",
    "RuleError",
    "RuleWalk",
    "Router",
    "Rules",
    "Schema",
    "ServerEndpoint",
    "SseEvent",
    "SseFrames",
    "SseOptions",
    "SseParser",
    "SseStream",
    "TestSupport",
    "Transition",
    "UntypedError",
    "Veld",
    "VeldError",
    "VeldHandler",
    "VeldHttpError",
    "VeldRequest",
    "VeldResponse",
    "VeldTransportError",
    "Violation",
    "Wire",
    "WsChannel",
    "WsConnection",
    "WsSession",
    "WsSocket",
);

/// The Java package of the runtime for a base package.
#[must_use]
pub fn package(base: &str) -> String {
    format!("{base}.runtime")
}

/// The Java package of the adapters for a base package.
#[must_use]
pub fn adapter_package(base: &str) -> String {
    format!("{base}.adapters")
}

/// Which streaming transports a target uses. The runtime sources carry the streaming code between
/// `// veld:sse`, `// veld:ws` and `// veld:streaming` (either) markers and `// veld:end`; a target
/// gets only the regions, and only the `Sse*` / `Ws*` classes, it needs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Streaming {
    /// Server-Sent Events (`streaming_sse`).
    pub sse: bool,
    /// WebSockets (`streaming_ws`).
    pub ws: bool,
}

impl Streaming {
    /// The transports a set of IR features asks for.
    #[must_use]
    pub fn of(features: &BTreeSet<Feature>) -> Self {
        Self {
            sse: features.contains(&Feature::StreamingSse),
            ws: features.contains(&Feature::StreamingWs),
        }
    }

    /// Whether a runtime class belongs to a transport this target does not use.
    fn omits(self, class: &str) -> bool {
        (class.starts_with("Sse") && !self.sse)
            || (class.starts_with("Ws") && !self.ws)
            || (class == "Reconnect" && !(self.sse || self.ws))
    }

    fn keeps(self, tag: &str) -> Option<bool> {
        match tag {
            "sse" => Some(self.sse),
            "ws" => Some(self.ws),
            "streaming" => Some(self.sse || self.ws),
            _ => None,
        }
    }
}

/// Resolves the `// veld:<tag>` ... `// veld:end` regions of `text`. Marker lines never reach the
/// output; the lines of a region whose transport is unused go with it. Regions may nest.
fn strip_regions(text: &str, streaming: Streaming) -> String {
    let mut out = String::with_capacity(text.len());
    let mut open: Vec<bool> = Vec::new();
    for line in text.split_inclusive('\n') {
        let marker = line.trim().strip_prefix("// veld:");
        match marker {
            Some("end") => {
                open.pop();
            }
            Some(tag) if streaming.keeps(tag).is_some() => {
                open.push(streaming.keeps(tag).unwrap_or(true));
            }
            _ if open.iter().all(|keep| *keep) => out.push_str(line),
            _ => {}
        }
    }
    out
}

/// A runtime or adapter source with its package (and the runtime import) substituted and its
/// streaming regions resolved.
pub(crate) fn substitute(source: &str, base: &str, streaming: Streaming) -> String {
    let text = strip_regions(source, streaming)
        .replace(
            &format!("package {SOURCE_PACKAGE};"),
            &format!("package {};", package(base)),
        )
        .replace(
            &format!("package {ADAPTER_PACKAGE};"),
            &format!("package {};", adapter_package(base)),
        )
        .replace(
            &format!("import {SOURCE_PACKAGE}."),
            &format!("import {}.", package(base)),
        );
    format!("{HEADER}\n{text}")
}

/// Adds the runtime classes the target needs to `files`. With `rules` the rule classes are added
/// and the `// #if rules` regions of the other classes are kept (`veld_codegen::gate_regions`);
/// without it the output is exactly what it was before rules existed.
pub fn emit(
    files: &mut FileSet,
    base: &str,
    streaming: Streaming,
    rules: bool,
) -> Result<(), Vec<GenDiagnostic>> {
    emit_with(
        files,
        base,
        streaming,
        Flags {
            rules,
            ..Flags::default()
        },
    )
}

/// Which optional parts of the runtime a target carries. `endpoint` and `testing` need `rules`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    /// Invariants (`// #if rules` regions and the rule classes).
    pub rules: bool,
    /// Endpoint rules (`// #if endpoint` regions and `EndpointRules`).
    pub endpoint: bool,
    /// The test handler support (`// #if testing` regions and `TestSupport`).
    pub testing: bool,
    /// The transition guard of endpoints (`// #if machine` regions and `Transition`). Needs `endpoint`.
    pub machine: bool,
}

/// Like [`emit`], with the endpoint rules and test handler parts switched by `flags`.
pub fn emit_with(
    files: &mut FileSet,
    base: &str,
    streaming: Streaming,
    flags: Flags,
) -> Result<(), Vec<GenDiagnostic>> {
    let package = package(base);
    let wanted = |name: &str| {
        !streaming.omits(name)
            && (flags.rules || !RULE_FILES.contains(&name))
            && (flags.endpoint || !ENDPOINT_FILES.contains(&name))
            && (flags.testing || !TESTING_FILES.contains(&name))
            && (flags.machine || !MACHINE_FILES.contains(&name))
    };
    let enabled = |region: &str| match region {
        "rules" => flags.rules,
        "endpoint" => flags.endpoint,
        "testing" => flags.testing,
        "machine" => flags.machine,
        _ => false,
    };
    for (name, source) in RUNTIME.iter().filter(|(name, _)| wanted(name)) {
        let gated = gate_regions(source, &enabled).map_err(|e| {
            vec![GenDiagnostic::new(
                "E0805",
                format!("runtime class `{name}`: {e}"),
            )]
        })?;
        add_file(
            files,
            &java_path(&package, name),
            substitute(&gated, base, streaming),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "a\n// veld:streaming\nb\n// veld:sse\nc\n// veld:end\n// veld:ws\nd\n// veld:end\n// veld:end\ne\n";

    const SSE: Streaming = Streaming {
        sse: true,
        ws: false,
    };
    const WS: Streaming = Streaming {
        sse: false,
        ws: true,
    };

    #[test]
    fn regions_resolve_per_transport() {
        assert_eq!(strip_regions(SAMPLE, Streaming::default()), "a\ne\n");
        assert_eq!(strip_regions(SAMPLE, SSE), "a\nb\nc\ne\n");
        assert_eq!(strip_regions(SAMPLE, WS), "a\nb\nd\ne\n");
    }

    #[test]
    fn no_marker_survives_and_only_needed_classes_are_emitted() {
        let both = Streaming {
            sse: true,
            ws: true,
        };
        for streaming in [Streaming::default(), SSE, WS, both] {
            let mut files = FileSet::new();
            emit(&mut files, "a.b", streaming, false).unwrap();
            let has = |prefix: &str| {
                files
                    .paths()
                    .any(|p| p.contains(&format!("/runtime/{prefix}")))
            };
            assert_eq!(has("Sse"), streaming.sse);
            assert_eq!(has("Ws"), streaming.ws);
            for (_, bytes) in files.iter() {
                assert!(!String::from_utf8_lossy(bytes).contains("// veld:"));
            }
        }
    }
}

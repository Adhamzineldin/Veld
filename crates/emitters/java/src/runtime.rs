//! The embedded runtime: real `.java` files, byte-identical in every role except for the
//! package declaration.

use std::collections::BTreeSet;

use veld_codegen::{FileSet, GenDiagnostic};
use veld_ir::Feature;

use crate::tree::java_path;
use crate::util::{HEADER, add_file};

/// The package the embedded sources declare. It is replaced by `<base>.runtime`.
const SOURCE_PACKAGE: &str = "dev.veld.runtime";
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
    "EndpointSpec",
    "HandlerHooks",
    "Issue",
    "Json",
    "JsonParser",
    "JsonWriter",
    "Page",
    "RequestContext",
    "Router",
    "Schema",
    "ServerEndpoint",
    "SseEvent",
    "SseFrames",
    "SseOptions",
    "SseParser",
    "SseStream",
    "UntypedError",
    "Veld",
    "VeldError",
    "VeldHandler",
    "VeldHttpError",
    "VeldRequest",
    "VeldResponse",
    "VeldTransportError",
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
        (class.starts_with("Sse") && !self.sse) || (class.starts_with("Ws") && !self.ws)
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

/// Adds the runtime classes the target needs to `files`.
pub fn emit(
    files: &mut FileSet,
    base: &str,
    streaming: Streaming,
) -> Result<(), Vec<GenDiagnostic>> {
    let package = package(base);
    for (name, source) in RUNTIME.iter().filter(|(name, _)| !streaming.omits(name)) {
        add_file(
            files,
            &java_path(&package, name),
            substitute(source, base, streaming),
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
            emit(&mut files, "a.b", streaming).unwrap();
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

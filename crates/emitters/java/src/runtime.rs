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
/// The runtime class of the due-moves runner (`spec/platform.md` section 10.1).
const TIMED_FILES: &[&str] = &["DueMoves"];
/// The measurement of the metrics hook (`spec/platform.md` section 13).
const SLO_FILES: &[&str] = &["Metric"];
/// The deadline of a request (`spec/platform.md` section 12).
const TIMEOUT_FILES: &[&str] = &["Deadline"];
/// The provider and the readings of the flags (`spec/platform.md` section 11).
const FLAG_FILES: &[&str] = &["Flags"];
/// The platform phase P1 state and features (`spec/targets/java.md` section 17).
const PLATFORM_FILES: &[&str] = &["Platform"];
/// The runtime classes of the policies (`spec/targets/java.md` section 11).
const POLICY_FILES: &[&str] = &["Policy"];
/// The runtime class of audited rules (`@rollout(audit)`, `spec/rules.md` section 17.1).
const ROLLOUT_FILES: &[&str] = &["Audit"];
/// The runtime classes of a server with `@version` or `@idempotent` endpoints
/// (`spec/targets/java.md` section 13).
const CONDITIONAL_FILES: &[&str] = &[
    "Conditions",
    "IdempotencyStore",
    "MemoryIdempotencyStore",
    "StoredAnswer",
];
/// The runtime class of derived fields (`spec/rules.md` section 17.2, `spec/targets/java.md` section
/// 15), emitted only for a target that has a model with a derived field. It needs the rule classes.
const DERIVED_FILES: &[&str] = &["Derive"];
/// The runtime class of the `If-Match` of a client call (`spec/targets/java.md` section 13).
const PRECONDITION_FILES: &[&str] = &["IfMatch"];
/// The runtime classes of events (`spec/events.md` sections 6 to 9), emitted only for a server that
/// publishes or subscribes. They need the rule classes (`Flags::rules`).
const EVENT_FILES: &[&str] = &["Events", "MemoryTransport", "Transport"];

/// The runtime classes of the outbox of events phase E3 (`spec/events.md` section 11,
/// `spec/targets/java.md` section 12.6): emitted only for a server with an emitting move.
const OUTBOX_FILES: &[&str] = &["Emission", "MemoryOutbox", "Outbox", "OutboxEntry"];

/// The canonical scalars (`Canon`), shared by the data of an event and the fingerprint of an
/// idempotency key: emitted for a server with events or with `@idempotent`/`@version` endpoints.
const CANON_FILES: &[&str] = &["Canon"];

/// The runtime classes of the typed runtime config (`spec/targets/java.md` section 14): the check
/// and its error. A target that has a runtime config only.
const RUNTIME_CONFIG_FILES: &[&str] = &["RuntimeCheck", "VeldConfigException"];

/// The package the embedded adapter sources declare. It is replaced by `<base>.adapters`.
const ADAPTER_PACKAGE: &str = "dev.veld.adapters";

macro_rules! runtime_files {
    ($($name:literal),* $(,)?) => {
        /// `(simple name, source)` of every runtime class, in a fixed order.
        pub const RUNTIME: &[(&str, &str)] = &[$(($name, include_str!(concat!("../runtime/", $name, ".java")))),*];
    };
}

runtime_files!(
    "Audit",
    "Canon",
    "ClientCore",
    "ClientDefaults",
    "ClientOptions",
    "Codec",
    "Conditions",
    "Deadline",
    "Derive",
    "DigestHandler",
    "DueMoves",
    "Emission",
    "EndpointRules",
    "EndpointSpec",
    "Events",
    "Flags",
    "HandlerHooks",
    "IdempotencyStore",
    "IfMatch",
    "Issue",
    "Json",
    "JsonParser",
    "JsonWriter",
    "MemoryIdempotencyStore",
    "MemoryOutbox",
    "MemoryTransport",
    "Metric",
    "Outbox",
    "OutboxEntry",
    "Page",
    "Platform",
    "Policy",
    "Reconnect",
    "RequestContext",
    "RuleError",
    "RuleWalk",
    "Router",
    "Rules",
    "RuntimeCheck",
    "Schema",
    "ServerEndpoint",
    "SseEvent",
    "SseFrames",
    "SseOptions",
    "SseParser",
    "SseStream",
    "StoredAnswer",
    "TestSupport",
    "Transition",
    "Transport",
    "UntypedError",
    "Veld",
    "VeldConfigException",
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

/// Which optional parts of the runtime a target carries. `endpoint` needs `rules`; `testing` needs
/// `endpoint` or `conditional` (a service with only a key, a guard or an `ETag` has a test handler);
/// `derived` needs `rules`.
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
    /// Timed moves (`rules_timed`, `// #if timed` regions and `DueMoves`). Needs `machine`.
    pub timed: bool,
    /// Service levels (`slo`, `// #if slo` regions and `Metric`): the metrics hook. A server only.
    pub slo: bool,
    /// Deadlines (`timeouts`, `// #if timeouts` regions and `Deadline`): a server runs the pipeline of
    /// an endpoint with a `@timeout` on a worker thread; a client uses the deadline as its default
    /// timeout.
    pub timeouts: bool,
    /// Flags (`rules_flags`, `// #if flags` regions and `Flags`). Needs `rules`.
    pub flags: bool,
    /// Policies and the typed principal (`// #if policies` regions and `Policy`). Needs `rules`.
    pub policies: bool,
    /// Audited rules (`// #if rollout` regions and `Audit`). Needs `rules`.
    pub rollout: bool,
    /// Events (`Events`, `MemoryTransport`, `Transport`). Needs `rules`.
    pub events: bool,
    /// `@version` and `@idempotent` on the server side: `Conditions`, the store classes and the
    /// `// #if conditional` regions. A server only.
    pub conditional: bool,
    /// `If-Match` and the idempotency key on a client call: `IfMatch` and the
    /// `// #if preconditions` regions.
    pub preconditions: bool,
    /// Derived fields (`Derive` and the `// #if derived` regions). Needs `rules`.
    pub derived: bool,
    /// The outbox of events phase E3 (`// #if outbox` regions, `Outbox`, `MemoryOutbox`, `Emission`):
    /// a server with a move that emits. Needs `events` and `machine`.
    pub outbox: bool,
    /// The typed runtime config (`spec/runtime-config.md`): `RuntimeCheck`, `VeldConfigException`
    /// and the `// #if runtimecfg` regions. Without it a client keeps the environment fallback of
    /// before, so a target without settings keeps its bytes.
    pub runtime_config: bool,
    /// Field visibility (`visibility`, `spec/platform.md` section 2): a server's answers can reach a
    /// `@visible` field.
    pub visibility: bool,
    /// Tenancy (`tenancy`, section 3): a server's answers or facts can be tenant-scoped. Needs `rules`.
    pub tenancy: bool,
    /// The audit log (`audit`, section 4): an endpoint of a server is `@audited`.
    pub audit: bool,
    /// Rate limits of a server (`limits`, section 5): an endpoint has a `@limit`.
    pub limits_server: bool,
    /// Rate limits of a client: the call waits and sends again on a `429`.
    pub limits: bool,
    /// A declaration of the tree has a `@tenant` field (a client's request may lack it).
    pub tenant_fields: bool,
}

impl Flags {
    /// Any platform phase P1 feature of a server: `Platform` and the per-request state.
    #[must_use]
    pub fn platform(&self) -> bool {
        self.visibility || self.tenancy || self.audit || self.limits_server
    }

    /// The answers of a server are shaped (visibility, the tenant check).
    #[must_use]
    pub fn shapes(&self) -> bool {
        self.visibility || self.tenancy
    }

    /// An object schema carries platform marks.
    #[must_use]
    pub fn marks(&self) -> bool {
        self.visibility || self.tenancy || self.audit
    }
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
            && (flags.timed || !TIMED_FILES.contains(&name))
            && (flags.slo || !SLO_FILES.contains(&name))
            && (flags.timeouts || !TIMEOUT_FILES.contains(&name))
            && (flags.flags || !FLAG_FILES.contains(&name))
            && (flags.policies || !POLICY_FILES.contains(&name))
            && (flags.rollout || !ROLLOUT_FILES.contains(&name))
            && (flags.outbox || !OUTBOX_FILES.contains(&name))
            && (flags.events || !EVENT_FILES.contains(&name))
            && (flags.events || flags.conditional || !CANON_FILES.contains(&name))
            && (flags.derived || !DERIVED_FILES.contains(&name))
            && (flags.conditional || !CONDITIONAL_FILES.contains(&name))
            && (flags.preconditions || !PRECONDITION_FILES.contains(&name))
            && (flags.runtime_config || !RUNTIME_CONFIG_FILES.contains(&name))
            && (flags.platform() || !PLATFORM_FILES.contains(&name))
    };
    let enabled = |region: &str| match region {
        "rules" => flags.rules,
        "endpoint" => flags.endpoint,
        "testing" => flags.testing,
        "machine" => flags.machine,
        "timed" => flags.timed,
        "slo" => flags.slo,
        "timeouts" => flags.timeouts,
        "flags" => flags.flags,
        "flags_conditional" => flags.flags && flags.conditional,
        "testing_rules_flags" => flags.testing && flags.rules && flags.flags,
        "no_flags" => !flags.flags,
        "slo_timeouts" => flags.slo && flags.timeouts,
        "slo_only" => flags.slo && !flags.timeouts,
        "timeouts_only" => flags.timeouts && !flags.slo,
        "wrapped" => flags.slo || flags.timeouts,
        "no_wrap" => !(flags.slo || flags.timeouts),
        "timeouts_conditional" => flags.timeouts && flags.conditional,
        "no_timeouts" => !flags.timeouts,
        "timeouts_runtimecfg" => flags.timeouts && flags.runtime_config,
        "policies" => flags.policies,
        "rollout" => flags.rollout,
        "conditional" => flags.conditional,
        "plain" => !flags.conditional,
        "conditional_rules" => flags.conditional && flags.rules,
        "plain_rules" => !(flags.conditional && flags.rules),
        "early" => flags.conditional && !flags.endpoint,
        "derived" => flags.derived,
        "rollout_derived" => flags.rollout && flags.derived,
        "no_derived" => !flags.derived,
        "derived_conditional" => flags.derived && flags.conditional,
        "no_derived_conditional" => !flags.derived && flags.conditional,
        "rules_no_derived" => flags.rules && !flags.derived,
        "preconditions" => flags.preconditions,
        "testing_conditional" => flags.testing && flags.conditional,
        "testing_events" => flags.testing && flags.events,
        "outbox" => flags.outbox,
        "not_outbox" => !flags.outbox,
        "testing_outbox" => flags.testing && flags.outbox,
        "testing_rules" => flags.testing && flags.rules,
        "testing_rules_conditional" => flags.testing && flags.rules && flags.conditional,
        "testing_plain" => !(flags.testing && flags.conditional),
        "runtimecfg" => flags.runtime_config,
        "legacy" => !flags.runtime_config,
        "runtimecfg_rules" => flags.runtime_config && flags.rules,
        "platform" => flags.platform(),
        "shapes" => flags.shapes(),
        "marks" => flags.marks(),
        "marks_rules" => flags.marks() && flags.rules,
        "marks_derived" => flags.marks() && flags.derived,
        "visibility" => flags.visibility,
        "platform_conditional" => flags.platform() && flags.conditional,
        "testing_platform" => flags.testing && flags.platform(),
        "fin_flags" => flags.flags && !flags.platform(),
        "fin_platform_flags" => flags.flags && flags.platform(),
        "fin_platform" => flags.platform() && !flags.flags,
        "sse_flags" => flags.flags && !flags.platform(),
        "sse_no_flags" => !flags.flags && !flags.platform(),
        "sse_platform_flags" => flags.flags && flags.platform(),
        "sse_platform" => flags.platform() && !flags.flags,
        "platform_early" => flags.platform() && !flags.endpoint && !flags.conditional,
        "testing_platform_early" => {
            flags.testing && flags.platform() && !flags.endpoint && !flags.conditional
        }
        "shapes_endpoint" => flags.shapes() && flags.endpoint,
        "not_shapes_endpoint" => flags.endpoint && !flags.shapes(),
        "not_shapes" => !flags.shapes(),
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

    /// The gates the older features switch, in the combinations a target really has.
    fn older() -> Vec<Flags> {
        let rules = Flags {
            rules: true,
            ..Flags::default()
        };
        let endpoint = Flags {
            endpoint: true,
            testing: true,
            ..rules
        };
        let everything = Flags {
            machine: true,
            conditional: true,
            policies: true,
            rollout: true,
            flags: true,
            derived: true,
            slo: true,
            timeouts: true,
            runtime_config: true,
            events: true,
            preconditions: true,
            ..endpoint
        };
        vec![
            Flags::default(),
            rules,
            endpoint,
            Flags {
                conditional: true,
                ..endpoint
            },
            Flags {
                flags: true,
                ..endpoint
            },
            Flags {
                derived: true,
                timeouts: true,
                ..rules
            },
            Flags {
                slo: true,
                timeouts: true,
                runtime_config: true,
                ..rules
            },
            everything,
        ]
    }

    /// Every combination of the gates of phase P1 with a set of older ones: the regions pair up
    /// (`gate_regions` accepts the file), no marker survives, and the platform classes are there
    /// exactly when a platform gate is on.
    #[test]
    fn the_regions_of_the_platform_features_pair_up_in_every_combination() {
        for base in older() {
            for bits in 0..64u32 {
                let flags = Flags {
                    visibility: bits & 1 != 0,
                    tenancy: bits & 2 != 0,
                    audit: bits & 4 != 0,
                    limits_server: bits & 8 != 0,
                    limits: bits & 16 != 0,
                    tenant_fields: bits & 32 != 0,
                    rules: base.rules || bits & 2 != 0,
                    ..base
                };
                let mut files = FileSet::new();
                emit_with(&mut files, "a.b", Streaming::default(), flags).unwrap();
                let has_platform = files.paths().any(|p| p.ends_with("/runtime/Platform.java"));
                assert_eq!(has_platform, flags.platform(), "{flags:?}");
                for (path, bytes) in files.iter() {
                    let text = String::from_utf8_lossy(bytes);
                    assert!(
                        !text.contains("// #if") && !text.contains("// #endif"),
                        "{path}"
                    );
                    if !flags.platform() {
                        assert!(!text.contains("Platform."), "{path} in {flags:?}");
                    }
                }
            }
        }
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

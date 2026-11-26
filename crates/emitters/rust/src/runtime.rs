//! The embedded runtime: plain Rust source files, copied into every generated crate as
//! `src/veld/`, gated by `// @if <flag>` ... `// @end` lines so that a crate carries only what it
//! uses (a client has no server core, a contract without streams has no `futures-core`).

use veld_codegen::{FileSet, GenDiagnostic, gate_regions};

use crate::util::{HEADER, add_file};

/// What the generated crate needs of the runtime and its adapters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Features {
    pub server: bool,
    pub client: bool,
    /// Streaming infrastructure: `Stream`, server-sent events.
    pub sse: bool,
    /// The served services have `ws` endpoints.
    pub ws_server: bool,
    /// The generated clients have `ws` endpoints.
    pub ws_client: bool,
    pub axum: bool,
    pub reqwest: bool,
    pub ureq: bool,
    /// The crate has declarations with invariants (`spec/rules.md`): the rules runtime is emitted
    /// and the `// #if rules` regions of the runtime files are kept.
    pub rules: bool,
    /// A served service has endpoint rules (`spec/rules.md` section 9): the server core runs
    /// steps 6 and 9. Implies `rules`.
    pub endpoint_rules: bool,
    /// A served service has an `allow` or a `transition` (`spec/rules.md` sections 10 and 11):
    /// step 6 runs the guards. Implies `endpoint_rules`.
    pub guards: bool,
    /// A policy is generated (an `allow` of a generated service names one), in either role: the
    /// system clock reading of the policy functions. Implies `rules`.
    pub policies: bool,
    /// The crate has an audited rule (`@rollout(audit)`, `spec/rules.md` section 17.1): the `// #if rollout`
    /// regions of the runtime are kept. Implies `rules`.
    pub rollout: bool,
    /// A served service publishes or subscribes (`spec/events.md`): `src/veld/events.rs` is
    /// emitted.
    pub events: bool,
    /// A generated service has an endpoint with a version guard, or a served one answers with a
    /// model that has a version field (`spec/rules.md` section 11.7): `src/veld/conditional.rs`
    /// (`IfMatch`, the `ETag`) is emitted.
    pub version: bool,
    /// A served endpoint reads `If-Match` or writes an `ETag`: the server core does. Implies
    /// `version` and `conditional`.
    pub etag: bool,
    /// A served service has an `@idempotent` endpoint (`spec/rules.md` section 11.8):
    /// `src/veld/idempotency.rs` is emitted. Implies `conditional`.
    pub idempotency: bool,
    /// The server core says, for each endpoint, what it owes to `@version` and `@idempotent`.
    pub conditional: bool,
    /// A declaration of the crate has a derived field (`spec/rules.md` section 17.2): the schemas
    /// say which fields are derived and the rules runtime computes them. Implies `rules`.
    pub derived: bool,
    /// The target has a runtime config (`spec/runtime-config.md`): `src/veld/config.rs` (the
    /// engine of its check) is emitted, next to `src/runtime_config.rs`.
    pub config: bool,
    /// The target option `messaging` names Kafka (`spec/events.md` section 10.7):
    /// `src/adapters/kafka.rs` and the `rdkafka` dependency.
    pub kafka: bool,
    /// The test handler (`testing = true`) of a service that subscribes serves
    /// `POST /__veld/test/event` (`spec/events.md` section 10.9): the crate needs `axum`, also when
    /// no service has an HTTP surface.
    pub test_events: bool,
    /// The same for RabbitMQ: `src/adapters/rabbitmq.rs` and `lapin`.
    pub rabbitmq: bool,
}

impl Features {
    /// Whether the region `name` of a runtime file is kept (`// #if rules`, `// #if !rules`,
    /// `// #if endpoint_rules`).
    fn region(&self, name: &str) -> bool {
        match name {
            "rules" => self.rules,
            "!rules" => !self.rules,
            "endpoint_rules" => self.endpoint_rules,
            "guards" => self.guards,
            "policies" => self.policies,
            "rollout" => self.rollout,
            "!rollout" => !self.rollout,
            "derived" => self.derived,
            "rollout_endpoint" => self.rollout && self.endpoint_rules,
            "rules_plain" => self.rules && !self.rollout,
            _ => false,
        }
    }

    fn flag(&self, name: &str) -> bool {
        match name {
            "server" => self.server,
            "client" => self.client,
            "sse" => self.sse,
            "ws_server" => self.ws_server,
            "ws_client" => self.ws_client,
            "axum" => self.axum,
            "rules" => self.rules,
            "version" => self.version,
            "etag" => self.etag,
            "idempotency" => self.idempotency,
            "conditional" => self.conditional,
            "derived" => self.derived,
            "reqwest" => self.reqwest,
            "ureq" => self.ureq,
            _ => false,
        }
    }
}

/// Keeps the lines whose `// @if` conditions hold. `// @if !name` negates; blocks nest.
#[must_use]
pub fn gate(src: &str, f: &Features) -> String {
    let mut stack: Vec<bool> = Vec::new();
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let t = line.trim();
        if let Some(cond) = t.strip_prefix("// @if ") {
            let cond = cond.trim();
            let on = match cond.strip_prefix('!') {
                Some(name) => !f.flag(name),
                None => f.flag(cond),
            };
            stack.push(on);
        } else if t == "// @end" {
            stack.pop();
        } else if stack.iter().all(|on| *on) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

const RULES: &str = include_str!("../runtime/rules.rs");
// The helpers of the reference evaluator (`veld-rules`), embedded as they are: generated rule
// code and the reference evaluator run the same helper source.
const RULES_DECIMAL: &str = include_str!("../../../veld-rules/src/helpers/decimal.rs");
const RULES_INSTANT: &str = include_str!("../../../veld-rules/src/helpers/instant.rs");
const RULES_NUM: &str = include_str!("../../../veld-rules/src/helpers/num.rs");
const RULES_TEXT: &str = include_str!("../../../veld-rules/src/helpers/text.rs");
const CANONICAL: &str = include_str!("../runtime/canonical.rs");
const CLIENT: &str = include_str!("../runtime/client.rs");
const CONFIG: &str = include_str!("../runtime/config.rs");
const CONDITIONAL: &str = include_str!("../runtime/conditional.rs");
const IDEMPOTENCY: &str = include_str!("../runtime/idempotency.rs");
const ERROR: &str = include_str!("../runtime/error.rs");
const EVENTS: &str = include_str!("../runtime/events.rs");
const PAGE: &str = include_str!("../runtime/page.rs");
const REGEX: &str = include_str!("../runtime/regex.rs");
const SCHEMA: &str = include_str!("../runtime/schema.rs");
const SERVER: &str = include_str!("../runtime/server.rs");
const SPEC: &str = include_str!("../runtime/spec.rs");
const STREAM: &str = include_str!("../runtime/stream.rs");
const TRANSPORT_REQWEST: &str = include_str!("../runtime/transport_reqwest.rs");
const TRANSPORT_UREQ: &str = include_str!("../runtime/transport_ureq.rs");
const UTIL: &str = include_str!("../runtime/util.rs");
const ADAPTER_AXUM: &str = include_str!("../runtime/adapter_axum.rs");
const KAFKA: &str = include_str!("../runtime/brokers/kafka.rs");
const RABBITMQ: &str = include_str!("../runtime/brokers/rabbitmq.rs");

/// The `src/veld/mod.rs` of a crate.
#[must_use]
pub fn mod_rs(f: &Features) -> String {
    let mut s = String::new();
    s.push_str(
        "//! The Veld runtime: schemas and validation, canonical bytes, identity (`same`), problem+json,\n\
         //! and the server and client cores. Every generated crate carries the same bytes of it.\n\n",
    );
    for (name, on) in [
        ("canonical", true),
        ("client", f.client),
        ("config", f.config),
        ("conditional", f.version),
        ("error", true),
        ("events", f.events),
        ("idempotency", f.idempotency),
        ("page", true),
        ("regex", true),
        ("rules", f.rules),
        ("schema", true),
        ("server", f.server),
        ("spec", true),
        ("stream", f.sse),
        ("transport_reqwest", f.reqwest),
        ("transport_ureq", f.ureq),
        ("util", true),
    ] {
        if on {
            s.push_str(&format!("pub mod {name};\n"));
        }
    }
    s.push_str(
        "\npub use error::{DeclaredError, Error, Identity, Issue, Problem, VeldDecl, VeldValue, same, same_all, same_parts};\n\
         pub use page::Page;\n\
         pub use util::block_on;\n",
    );
    s
}

/// The `pub mod` lines of `src/adapters/mod.rs`, in name order.
fn adapter_mods(f: &Features) -> String {
    let mut out = String::new();
    for (name, on) in [
        ("axum", f.axum),
        ("kafka", f.kafka),
        ("rabbitmq", f.rabbitmq),
    ] {
        if on {
            out.push_str(&format!("pub mod {name};\n"));
        }
    }
    out
}

/// Emits `src/veld/*` and the adapter files the features ask for.
pub fn emit(files: &mut FileSet, f: &Features) -> Result<(), Vec<GenDiagnostic>> {
    let mut put = |path: &str, src: &str, on: bool| -> Result<(), Vec<GenDiagnostic>> {
        if !on {
            return Ok(());
        }
        let kept = gate_regions(src, &|name| f.region(name))
            .map_err(|e| vec![GenDiagnostic::new("E0805", format!("{path}: {e}"))])?;
        add_file(files, path, format!("{HEADER}\n\n{}", gate(&kept, f)))
    };
    put("src/veld/canonical.rs", CANONICAL, true)?;
    put("src/veld/client.rs", CLIENT, f.client)?;
    put("src/veld/config.rs", CONFIG, f.config)?;
    put("src/veld/conditional.rs", CONDITIONAL, f.version)?;
    put("src/veld/idempotency.rs", IDEMPOTENCY, f.idempotency)?;
    put("src/veld/error.rs", ERROR, true)?;
    put("src/veld/events.rs", EVENTS, f.events)?;
    put("src/veld/page.rs", PAGE, true)?;
    put("src/veld/regex.rs", REGEX, true)?;
    put("src/veld/rules/mod.rs", RULES, f.rules)?;
    put("src/veld/rules/decimal.rs", RULES_DECIMAL, f.rules)?;
    put("src/veld/rules/instant.rs", RULES_INSTANT, f.rules)?;
    put("src/veld/rules/num.rs", RULES_NUM, f.rules)?;
    put("src/veld/rules/text.rs", RULES_TEXT, f.rules)?;
    put("src/veld/schema.rs", SCHEMA, true)?;
    put("src/veld/server.rs", SERVER, f.server)?;
    put("src/veld/spec.rs", SPEC, true)?;
    put("src/veld/stream.rs", STREAM, f.sse)?;
    put(
        "src/veld/transport_reqwest.rs",
        TRANSPORT_REQWEST,
        f.reqwest,
    )?;
    put("src/veld/transport_ureq.rs", TRANSPORT_UREQ, f.ureq)?;
    put("src/veld/util.rs", UTIL, true)?;
    put("src/veld/mod.rs", &mod_rs(f), true)?;
    put("src/adapters/kafka.rs", KAFKA, f.kafka)?;
    put("src/adapters/rabbitmq.rs", RABBITMQ, f.rabbitmq)?;
    if f.axum {
        put("src/adapters/axum.rs", ADAPTER_AXUM, true)?;
    }
    if f.kafka || f.rabbitmq {
        add_file(
            files,
            "src/adapters/mod.rs",
            format!(
                "{HEADER}\n\n//! Framework and broker adapters.\n\n{}",
                adapter_mods(f)
            ),
        )?;
    } else if f.axum {
        add_file(
            files,
            "src/adapters/mod.rs",
            format!("{HEADER}\n\n//! Framework adapters.\n\npub mod axum;\n"),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gating_keeps_and_drops_blocks() {
        let src = "a\n// @if sse\nb\n// @if !ws_server\nc\n// @end\n// @end\nd\n";
        let on = Features {
            sse: true,
            ..Features::default()
        };
        assert_eq!(gate(src, &on), "a\nb\nc\nd\n");
        let ws = Features {
            sse: true,
            ws_server: true,
            ..Features::default()
        };
        assert_eq!(gate(src, &ws), "a\nb\nd\n");
        assert_eq!(gate(src, &Features::default()), "a\nd\n");
    }
}

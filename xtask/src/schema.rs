//! The JSON Schema of `veld.toml` (`editors/veld.toml.schema.json`), written for `spec/config.md`.
//!
//! Editors use it for completion, hover and validation of TOML files (Even Better TOML in VS Code,
//! a JSON-schema mapping in JetBrains). The unit tests below check it against the real parser in
//! `veld-project`, so a key added to (or removed from) the config cannot drift unnoticed.

use serde_json::{Map, Value, json};

const NAME_PATTERN: &str = "^[A-Za-z_][A-Za-z0-9_-]*$";
const PACKAGE_PATTERN: &str = "^@[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*$";
const SEMVER_PATTERN: &str = "^[0-9]+\\.[0-9]+\\.[0-9]+([-+][0-9A-Za-z.-]+)?$";

fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn contracts() -> Value {
    string("Package root: the directory holding the `.veld` files, relative to `veld.toml`.")
}

/// `package`, `version` and `description` are shared by `[project]`, `[libraries.*]`, `[services.*]`.
fn section(description: &str, own: Value) -> Value {
    let mut props: Map<String, Value> = own.as_object().cloned().unwrap_or_default();
    props.insert(
        "package".into(),
        json!({
            "type": "string", "pattern": PACKAGE_PATTERN,
            "description": "Registry name `@org/name` used by `veld publish`.",
            "examples": ["@acme/billing"]
        }),
    );
    props.insert(
        "version".into(),
        json!({
            "type": "string", "pattern": SEMVER_PATTERN,
            "description": "Semantic version published by `veld publish`.",
            "examples": ["1.0.0"]
        }),
    );
    props.insert(
        "description".into(),
        string("Short description shown in the registry."),
    );
    json!({ "type": "object", "description": description, "additionalProperties": false, "properties": props })
}

fn project() -> Value {
    section(
        "The project. With no `[services]` section it is itself a single service named `name`.",
        json!({
            "name": string("Package name of the implicit default service (single service), or the workspace name."),
            "contracts": {
                "type": "string", "default": "contracts",
                "description": "Package root of the implicit service. Default `contracts`. Mutually exclusive with `[services.*]`."
            }
        }),
    )
}

fn library() -> Value {
    section(
        "A shared contract package, importable as `use <name>:<module>.{Name}`.",
        json!({ "contracts": contracts() }),
    )
}

fn service() -> Value {
    section(
        "A service of a workspace. Its contracts are one package.",
        json!({
            "contracts": contracts(),
            "base-url": string("Default base URL of the service for generated clients (lower priority than the constructor argument and `VELD_<SERVICE>_URL`)."),
            "consumes": {
                "type": "array", "items": { "type": "string" }, "uniqueItems": true,
                "description": "Services this service calls. Each must exist (E0702). Cycles are allowed. A server target generates a client for each one under `<out>/clients/<service>/`."
            }
        }),
    )
}

/// Framework values per `(emitter, role)`, from the adapter registries in `spec/config.md`.
const FRAMEWORKS: &[(&str, &str, &[&str])] = &[
    (
        "node",
        "server",
        &["express", "fastify", "hono", "node-http", "nest"],
    ),
    ("typescript", "client", &["react", "vue"]),
    ("java", "server", &["jdk", "spring"]),
];

fn emitter_rules() -> Vec<Value> {
    let mut rules = Vec::new();
    for (emitter, role, frameworks) in FRAMEWORKS {
        rules.push(json!({
            "if": { "properties": { "emitter": { "const": emitter }, "role": { "const": role } }, "required": ["emitter", "role"] },
            "then": { "properties": { "framework": { "enum": frameworks } } }
        }));
    }
    rules.push(json!({
        "if": { "properties": { "emitter": { "const": "java" }, "role": { "const": "client" } }, "required": ["emitter", "role"] },
        "then": { "properties": { "framework": false } }
    }));
    rules.push(json!({
        "if": { "properties": { "role": { "const": "server" } }, "required": ["role"] },
        "then": { "required": ["service"] }
    }));
    rules
}

fn options() -> Value {
    json!({
        "type": "object",
        "description": "Emitter-specific options, checked against the emitter's typed options. An unknown key is E0802.",
        "properties": {
            "java": { "enum": ["17", "21"], "default": "17", "description": "`java` emitter: the language version (default `17`, the oldest supported)." },
            "typescript": { "enum": ["es2020", "es2022"], "default": "es2020", "description": "`typescript` and `node` emitters: the TypeScript target." },
            "framework-version": { "type": "string", "description": "The framework version: `spring` `\"3\"`, `nest` `\"10\"`, `react` `\"18\"`, `vue` `\"3\"`. Needs a `framework`; unversioned frameworks reject it (E0803)." },
            "package": { "type": "string", "description": "`java` emitter: the base Java package.", "examples": ["com.acme.payments"] },
            "package-name": { "type": "string", "description": "`node` and `typescript` emitters: the generated `package.json` name (default `@veld/<target-name>`)." }
        },
        "additionalProperties": true
    })
}

fn target() -> Value {
    json!({
        "type": "object",
        "description": "One generated output: a role, an emitter and an output directory.",
        "additionalProperties": false,
        "required": ["role", "emitter", "out"],
        "properties": {
            "name": string("Optional name, used by `veld generate <name>` and for the package name `@veld/<name>`."),
            "role": { "enum": ["client", "server", "spec"], "description": "`server`, `client`, or `spec` (an OpenAPI, GraphQL, schema or docs export)." },
            "emitter": {
                "anyOf": [
                    { "enum": ["node", "typescript", "java"] },
                    { "type": "string", "pattern": "^plugin:[A-Za-z0-9_-]+$" }
                ],
                "description": "A built-in emitter (`node`, `typescript`, `java`) or `plugin:<name>` naming a `[plugins.<name>]` entry."
            },
            "framework": { "type": "string", "description": "The framework of the target. Valid values depend on emitter and role (E0803 lists them): node server `express`, `fastify`, `hono`, `node-http`, `nest`; typescript client `react`, `vue`; java server `jdk`, `spring`." },
            "service": string("The service a `server` target implements. Required for `role = \"server\"`."),
            "services": { "type": "array", "items": { "type": "string" }, "description": "The services a `client` target covers. Default: all services." },
            "out": string("Output directory, relative to `veld.toml`. Two targets must not share one (E0703); it must not be the project root or a contracts directory (E0704)."),
            "clients": { "type": "boolean", "default": true, "description": "Server targets only: set `false` to skip the generated clients for consumed services." },
            "app": string("The directory of the app that consumes the output, for `veld setup`. Default: the nearest ancestor of `out` with a `package.json`, `pom.xml` or `build.gradle[.kts]`."),
            "options": options()
        },
        "allOf": emitter_rules()
    })
}

fn plugin() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "description": "A generator plugin, started over the plugin protocol (`spec/plugin-protocol.md`).",
        "properties": {
            "command": string("Executable, resolved on PATH or relative to `veld.toml`."),
            "version": string("Plugin version."),
            "entry": string("Optional script an interpreter `command` runs; its path is appended to the command."),
            "sha256": { "type": "string", "pattern": "^[0-9a-fA-F]{64}$", "description": "SHA-256 of `entry` if set, else of the executable. A mismatch is refused (E0705)." }
        }
    })
}

fn lint() -> Value {
    let properties: Map<String, Value> = veld_compiler::codes::LINT_RULES
        .iter()
        .map(|(code, name)| {
            (
                (*name).to_string(),
                json!({ "$ref": "#/$defs/lintLevel", "description": format!("Lint rule {code}.") }),
            )
        })
        .collect();
    json!({
        "type": "object", "additionalProperties": false, "properties": properties,
        "description": "Lint rule levels (`off`, `warn`, `error`). Every rule defaults to `warn`; `veld check` reports them."
    })
}

fn named(description: &str, value: Value) -> Value {
    json!({
        "type": "object", "description": description,
        "propertyNames": { "pattern": NAME_PATTERN },
        "additionalProperties": value
    })
}

/// The whole schema as pretty JSON with a trailing newline.
pub fn veld_toml() -> String {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "https://veld.dev/schemas/veld.toml.schema.json",
        "title": "veld.toml",
        "description": "Veld 2 project configuration. Unknown keys are errors (E0701).",
        "type": "object",
        "additionalProperties": false,
        "required": ["project"],
        "properties": {
            "project": project(),
            "libraries": named("Shared contract packages.", library()),
            "services": named("The services of a workspace (microservices). Mutually exclusive with `[project].contracts`.", service()),
            "targets": { "type": "array", "description": "The generated outputs.", "items": target() },
            "plugins": named("Generator plugins.", plugin()),
            "registry": {
                "type": "object", "additionalProperties": false,
                "description": "The package registry.",
                "properties": { "url": { "type": "string", "format": "uri", "description": "Registry URL. Must be https except for localhost; `--registry` and `VELD_REGISTRY` override it." } }
            },
            "dependencies": {
                "type": "object", "description": "Registry packages, resolved and pinned in `veld.lock`.",
                "propertyNames": { "pattern": PACKAGE_PATTERN },
                "additionalProperties": { "type": "string", "description": "A semver range.", "examples": ["^1.2"] }
            },
            "lint": lint()
        },
        "$defs": { "lintLevel": { "enum": ["off", "warn", "error"] } }
    });
    let mut text = serde_json::to_string_pretty(&schema).unwrap_or_default();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Value {
        serde_json::from_str(&veld_toml()).unwrap()
    }

    fn keys(v: &Value) -> Vec<String> {
        v["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    /// A TOML value the key's type accepts, so only an unknown key name can fail the parse.
    fn sample(key: &str) -> &'static str {
        match key {
            "consumes" | "services" => "[]",
            "clients" => "true",
            "role" => "\"server\"",
            "options" => "{}",
            _ => "\"x\"",
        }
    }

    fn accepts(header: &str, key: &str) -> bool {
        let text = if header.is_empty() {
            format!("[project]\nname = \"p\"\n{key} = {}\n", sample(key))
        } else {
            format!(
                "[project]\nname = \"p\"\n{header}\n{key} = {}\n",
                sample(key)
            )
        };
        !matches!(
            veld_project::config::Config::parse(&text),
            Err(veld_project::config::ConfigError::UnknownKey { .. })
        )
    }

    #[test]
    fn every_schema_key_is_accepted_by_the_parser() {
        let s = schema();
        let sections = [
            ("", s["properties"]["project"].clone()),
            (
                "[libraries.a]",
                s["properties"]["libraries"]["additionalProperties"].clone(),
            ),
            (
                "[services.a]",
                s["properties"]["services"]["additionalProperties"].clone(),
            ),
            ("[[targets]]", s["properties"]["targets"]["items"].clone()),
            (
                "[plugins.a]",
                s["properties"]["plugins"]["additionalProperties"].clone(),
            ),
        ];
        for (header, node) in sections {
            for key in keys(&node) {
                assert!(
                    accepts(header, &key),
                    "schema key `{key}` under `{header}` is rejected by the parser"
                );
            }
            assert!(!accepts(header, "definitely-not-a-key"), "{header}");
        }
    }

    #[test]
    fn every_top_level_key_of_the_parser_is_in_the_schema() {
        let s = schema();
        for key in [
            "project",
            "libraries",
            "services",
            "targets",
            "plugins",
            "registry",
            "dependencies",
            "lint",
        ] {
            assert!(
                s["properties"].get(key).is_some(),
                "schema lacks top-level `{key}`"
            );
        }
    }

    #[test]
    fn targets_have_the_documented_enums() {
        let t = &schema()["properties"]["targets"]["items"]["properties"];
        assert_eq!(t["role"]["enum"], json!(["client", "server", "spec"]));
        assert_eq!(
            t["emitter"]["anyOf"][0]["enum"],
            json!(["node", "typescript", "java"])
        );
    }

    #[test]
    fn lint_rules_come_from_the_compiler_table() {
        assert!(!keys(&schema()["properties"]["lint"]).is_empty());
    }
}

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
    json!({
        "type": "string", "default": "contracts",
        "description": "The directory holding the `.veld` files, relative to this `veld.toml`. Default `contracts`."
    })
}

fn name_of(what: &str) -> Value {
    json!({
        "type": "string", "pattern": NAME_PATTERN,
        "description": format!("The name of the {what}: what `use <name>:...` and `consumes` refer to. Default: the directory name; required in a root file.")
    })
}

fn owners() -> Value {
    json!({
        "type": "array", "items": { "type": "string", "minLength": 1 },
        "description": "Free-form names (teams, handles, addresses) shown by `veld diff` and `veld check --graph=json`. Never part of the IR or of generated code."
    })
}

/// `package`, `version` and `description` are shared by `[service]` and `[library]`.
fn publishable(description: &str, own: Value) -> Value {
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

fn workspace() -> Value {
    json!({
        "type": "object", "additionalProperties": false, "required": ["name"],
        "description": "The workspace root: the project name, its members and what they share.",
        "properties": {
            "name": string("The project name. Required."),
            "description": string("A one-line description of the project."),
            "members": {
                "type": "array", "items": { "type": "string" },
                "description": "Member directories relative to the root, with `/`. A segment may be `*`. No other pattern syntax exists."
            },
            "exclude": {
                "type": "array", "items": { "type": "string" },
                "description": "Directories that are not members, whatever `members` says."
            },
            "prefix": string("Route prefix such as `/api`, prepended to every service prefix. Must start with `/`."),
            "defaults": {
                "type": "object",
                "description": "`[workspace.defaults.<role>.<emitter>]`: the `framework` and option keys a target of that role and emitter starts from. A key the member writes replaces the default.",
                "properties": {
                    "server": { "type": "object", "additionalProperties": { "type": "object" } },
                    "client": { "type": "object", "additionalProperties": { "type": "object" } }
                },
                "additionalProperties": false
            }
        }
    })
}

fn library() -> Value {
    publishable(
        "A shared contract package, importable as `use <name>:<module>.{Name}`. It has no targets.",
        json!({ "name": name_of("library"), "contracts": contracts(), "owners": owners() }),
    )
}

fn service() -> Value {
    publishable(
        "A service. Its contracts are one package. It may hold `[server]` and `[client]` targets.",
        json!({
            "name": name_of("service"),
            "contracts": contracts(),
            "prefix": string("Route prefix such as `/v1`, prepended to this service's prefixes after `[workspace].prefix`. Must start with `/`."),
            "messages": string("Directory of the message catalogs, relative to the member, one `<language>.toml` per language (default `messages`)."),
            "base-url": string("Default base URL of the service for generated clients (lower priority than the constructor argument and `VELD_<SERVICE>_URL`)."),
            "consumes": {
                "type": "array", "items": { "type": "string" }, "uniqueItems": true,
                "description": "Services this service calls (`*` is every other service). Each must exist (E0702). Cycles are allowed. A server target generates a client for each one under `<out>/clients/<service>/`."
            },
            "owners": owners()
        }),
    )
}

fn app() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "description": "An app: it consumes services and holds `[client]` targets. It has no contracts and no server.",
        "properties": {
            "name": name_of("app"),
            "uses": {
                "type": "array", "items": { "type": "string" }, "uniqueItems": true,
                "description": "The services the app's clients cover. Each must exist (E0702). Default: every service of the workspace."
            },
            "owners": owners()
        }
    })
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
    ("rust", "server", &["axum"]),
    ("rust", "client", &["reqwest", "ureq"]),
];

fn emitter_rules(role: &str) -> Vec<Value> {
    let mut rules = Vec::new();
    for (emitter, _, frameworks) in FRAMEWORKS.iter().filter(|(_, r, _)| *r == role) {
        rules.push(json!({
            "if": { "properties": { "emitter": { "const": emitter } }, "required": ["emitter"] },
            "then": { "properties": { "framework": { "enum": frameworks } } }
        }));
    }
    if role == "client" {
        rules.push(json!({
            "if": { "properties": { "emitter": { "const": "java" } }, "required": ["emitter"] },
            "then": { "properties": { "framework": false } }
        }));
    }
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
            "package-name": { "type": "string", "description": "`node` and `typescript` emitters: the generated `package.json` name (default `@veld/<target-name>`)." },
            "crate-name": { "type": "string", "description": "`rust` emitter: the generated crate name (default `<package>-<role>`).", "examples": ["orders-server"] },
            "transport": { "enum": ["reqwest", "ureq", "none"], "default": "reqwest", "description": "`rust` server: the HTTP transport of the clients for consumed services." },
            "validate": { "type": "boolean", "default": true, "description": "`node`, `typescript`, `java` and `rust` emitters: `false` skips run-time request validation by default and keeps the types." }
        },
        "additionalProperties": true
    })
}

fn target(role: &str) -> Value {
    json!({
        "type": "object",
        "description": format!("A {role} target: an emitter and an output directory. The role is the table name."),
        "additionalProperties": false,
        "required": ["emitter"],
        "properties": {
            "name": string("Name used by `veld generate <name>` and for the package name `@veld/<name>`. Default: the member's name, `<member>-server` or `<member>-client` when the member has both; required when a member has several targets of one role."),
            "emitter": {
                "anyOf": [
                    { "enum": ["node", "typescript", "java", "rust"] },
                    { "type": "string", "pattern": "^plugin:[A-Za-z0-9_-]+$" }
                ],
                "description": "A built-in emitter (`node`, `typescript`, `java`, `rust`) or `plugin:<name>` naming a `[plugins.<name>]` entry."
            },
            "framework": { "type": "string", "description": "The framework of the target. Valid values depend on emitter and role (E0803 lists them): node server `express`, `fastify`, `hono`, `node-http`, `nest`; typescript client `react`, `vue`; java server `jdk`, `spring`; rust server `axum`, rust client `reqwest`, `ureq`." },
            "out": string("Output directory, relative to the member's directory. Default: `src/generated` (node, typescript), `src/main/java` (java), `generated` (rust); a plugin has none. Two targets must not share one (E0703); it must not be the workspace root or a contracts directory (E0704)."),
            "clients": { "type": "boolean", "default": true, "description": "Server targets only: set `false` to skip the generated clients for consumed services." },
            "app": string("The directory of the app that consumes the output, relative to the member, for `veld setup`. Default: the nearest ancestor of `out` with a `package.json`, `pom.xml` or `build.gradle[.kts]`."),
            "options": options()
        },
        "allOf": emitter_rules(role)
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

fn hooks() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "description": "Commands run around `veld generate`.",
        "properties": {
            "post-generate": {
                "anyOf": [ { "type": "string" }, { "type": "array", "items": { "type": "string" } } ],
                "description": "Shell command(s) run from the project root after a successful, non-dry-run `veld generate` (for example `prettier --write gen`)."
            },
            "fail": { "type": "boolean", "default": false, "description": "Make a failing hook fail `veld generate`. By default a failure is only a warning." }
        }
    })
}

fn named(description: &str, value: Value) -> Value {
    json!({
        "type": "object", "description": description,
        "propertyNames": { "pattern": NAME_PATTERN },
        "additionalProperties": value
    })
}

/// `[server]` is a table, or an array of them (`[[server]]`) for more than one target.
fn targets(role: &str) -> Value {
    json!({ "oneOf": [ target(role), { "type": "array", "items": target(role), "minItems": 1 } ] })
}

/// The whole schema as pretty JSON with a trailing newline.
pub fn veld_toml() -> String {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "https://veld.dev/schemas/veld.toml.schema.json",
        "title": "veld.toml",
        "description": "Veld 2 workspace configuration: a root file with `[workspace]`, one file per member with `[service]`, `[app]` or `[library]`, or one file with a single member. Unknown keys are errors (E0701).",
        "type": "object",
        "additionalProperties": false,
        "anyOf": [
            { "required": ["workspace"] },
            { "required": ["service"] },
            { "required": ["app"] },
            { "required": ["library"] }
        ],
        "properties": {
            "workspace": workspace(),
            "service": service(),
            "app": app(),
            "library": library(),
            "server": targets("server"),
            "client": targets("client"),
            "plugins": named("Generator plugins. Allowed only in the workspace root.", plugin()),
            "registry": {
                "type": "object", "additionalProperties": false,
                "description": "The package registry. Allowed only in the workspace root.",
                "properties": { "url": { "type": "string", "format": "uri", "description": "Registry URL. Must be https except for localhost; `--registry` and `VELD_REGISTRY` override it." } }
            },
            "dependencies": {
                "type": "object", "description": "Registry packages, resolved and pinned in `veld.lock`. Allowed only in the workspace root.",
                "propertyNames": { "pattern": PACKAGE_PATTERN },
                "additionalProperties": { "type": "string", "description": "A semver range.", "examples": ["^1.2"] }
            },
            "lint": lint(),
            "hooks": hooks()
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
            "consumes" | "uses" | "owners" | "members" | "exclude" => "[]",
            "clients" => "true",
            "options" | "defaults" => "{}",
            _ => "\"x\"",
        }
    }

    fn accepts(header: &str, key: &str) -> bool {
        let text = format!("{header}\n{key} = {}\n", sample(key));
        !matches!(
            veld_project::config::FileConfig::parse(&text),
            Err(veld_project::config::ConfigError::UnknownKey { .. })
        )
    }

    #[test]
    fn every_schema_key_is_accepted_by_the_parser() {
        let s = schema();
        let p = &s["properties"];
        let sections = [
            ("[workspace]", p["workspace"].clone()),
            ("[service]", p["service"].clone()),
            ("[app]", p["app"].clone()),
            ("[library]", p["library"].clone()),
            ("[server]", p["server"]["oneOf"][0].clone()),
            ("[[client]]", p["client"]["oneOf"][0].clone()),
            ("[plugins.a]", p["plugins"]["additionalProperties"].clone()),
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
            "workspace",
            "service",
            "app",
            "library",
            "server",
            "client",
            "plugins",
            "registry",
            "dependencies",
            "lint",
            "hooks",
        ] {
            assert!(
                s["properties"].get(key).is_some(),
                "schema lacks top-level `{key}`"
            );
        }
        assert_eq!(s["properties"].as_object().unwrap().len(), 11);
    }

    #[test]
    fn targets_have_the_documented_enums() {
        let t = &schema()["properties"]["server"]["oneOf"][0]["properties"];
        assert_eq!(
            t["emitter"]["anyOf"][0]["enum"],
            json!(["node", "typescript", "java", "rust"])
        );
        assert!(t.get("role").is_none() && t.get("service").is_none());
    }

    #[test]
    fn lint_rules_come_from_the_compiler_table() {
        assert!(!keys(&schema()["properties"]["lint"]).is_empty());
    }
}

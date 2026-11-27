//! Messages of `spec/plugin-protocol.md` and the checks the host applies to them.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::{Value, json};
use std::collections::BTreeMap;

use veld_codegen::{
    Capabilities, FileSet, FrameworkInfo, GenDiagnostic, GenRequest, LanguageInfo, LanguageVersion,
    Role,
};
use veld_ir::Feature;

use crate::error::PluginError;

pub const PROTOCOL: &str = "1";
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_GENERATE_TIMEOUT: Duration = Duration::from_secs(60);
pub const MAX_GENERATE_TIMEOUT: Duration = Duration::from_secs(600);
/// How long a plugin gets to exit after its input is closed.
pub const EXIT_GRACE: Duration = Duration::from_secs(5);

pub const MAX_FILES: usize = 10_000;
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 200 * 1024 * 1024;
const MAX_DIAGNOSTICS: usize = 100;

/// What a plugin said about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub name: String,
    pub version: String,
    pub roles: Vec<Role>,
    pub capabilities: Capabilities,
    /// Frameworks per role. Absent in `hello` means none (a framework is then `E0803`).
    pub frameworks: BTreeMap<Role, Vec<FrameworkInfo>>,
    /// The language and its versions. Absent in `hello` means any version, unchecked.
    pub language: Option<LanguageInfo>,
}

impl Handshake {
    /// The frameworks the plugin declared for `role`.
    #[must_use]
    pub fn frameworks_for(&self, role: Role) -> Vec<FrameworkInfo> {
        self.frameworks.get(&role).cloned().unwrap_or_default()
    }
}

#[must_use]
pub fn hello(veld_version: &str) -> Value {
    json!({ "type": "hello", "protocol": PROTOCOL, "veld_version": veld_version })
}

/// Checks the plugin's `hello`: type, protocol major version, roles and capability names.
pub fn parse_handshake(plugin: &str, msg: &Value) -> Result<Handshake, PluginError> {
    let bad = |m: String| PluginError::protocol(plugin, m);
    if msg.get("type").and_then(Value::as_str) != Some("hello") {
        return Err(bad("the first message was not a `hello`".into()));
    }
    let protocol = msg
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if protocol.split('.').next() != Some(PROTOCOL) {
        return Err(bad(format!(
            "speaks protocol `{protocol}`, but this veld speaks protocol `{PROTOCOL}`"
        )));
    }
    let text = |key: &str| {
        msg.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let name = text("name").ok_or_else(|| bad("its `hello` has no `name`".into()))?;
    let version = text("version").ok_or_else(|| bad("its `hello` has no `version`".into()))?;
    let roles = parse_roles(plugin, msg.get("roles"))?;
    let capabilities = parse_capabilities(plugin, msg.get("capabilities"))?;
    let frameworks = parse_frameworks(plugin, msg.get("frameworks"))?;
    let language = parse_language(plugin, msg.get("language"))?;
    Ok(Handshake {
        name,
        version,
        roles,
        capabilities,
        frameworks,
        language,
    })
}

fn parse_roles(plugin: &str, v: Option<&Value>) -> Result<Vec<Role>, PluginError> {
    let items = v.and_then(Value::as_array).filter(|a| !a.is_empty());
    let items = items
        .ok_or_else(|| PluginError::protocol(plugin, "its `hello` must list at least one role"))?;
    let mut roles = BTreeSet::new();
    for item in items {
        match item.as_str() {
            Some("server") => roles.insert(Role::Server),
            Some("client") => roles.insert(Role::Client),
            other => {
                return Err(PluginError::protocol(
                    plugin,
                    format!("unknown role {other:?} (expected `server` or `client`)"),
                ));
            }
        };
    }
    Ok(roles.into_iter().collect())
}

fn parse_capabilities(plugin: &str, v: Option<&Value>) -> Result<Capabilities, PluginError> {
    let mut set = BTreeSet::new();
    for item in v
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let name = item.as_str().unwrap_or_default();
        let feature = Feature::from_name(name).ok_or_else(|| {
            let known: Vec<&str> = Feature::ALL.iter().map(|f| f.name()).collect();
            PluginError::protocol(
                plugin,
                format!("unknown capability `{name}` (known: {})", known.join(", ")),
            )
        })?;
        set.insert(feature);
    }
    Ok(Capabilities(set))
}

/// A non-empty string field, or a protocol error naming `what`.
fn text_field(plugin: &str, v: &Value, key: &str, what: &str) -> Result<String, PluginError> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| PluginError::protocol(plugin, format!("{what} needs a string `{key}`")))
}

/// `versions`: absent means none; otherwise an array of non-empty strings.
fn versions_field(plugin: &str, v: &Value, what: &str) -> Result<Vec<String>, PluginError> {
    let bad = || {
        PluginError::protocol(
            plugin,
            format!("{what}: `versions` must be an array of strings"),
        )
    };
    match v.get("versions") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|i| i.as_str().filter(|s| !s.is_empty()).map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(bad),
        Some(_) => Err(bad()),
    }
}

/// The optional `frameworks` of `hello`: `{ "server": [ {id, versions?, min_language?} ], "client": [] }`.
fn parse_frameworks(
    plugin: &str,
    v: Option<&Value>,
) -> Result<BTreeMap<Role, Vec<FrameworkInfo>>, PluginError> {
    let mut out = BTreeMap::new();
    let Some(v) = v.filter(|v| !v.is_null()) else {
        return Ok(out);
    };
    let map = v.as_object().ok_or_else(|| {
        PluginError::protocol(
            plugin,
            "`frameworks` must be an object with `server` and `client`",
        )
    })?;
    for (key, list) in map {
        let role = match key.as_str() {
            "server" => Role::Server,
            "client" => Role::Client,
            other => {
                return Err(PluginError::protocol(
                    plugin,
                    format!(
                        "`frameworks` has the unknown role `{other}` (expected `server` or `client`)"
                    ),
                ));
            }
        };
        let items = list.as_array().ok_or_else(|| {
            PluginError::protocol(plugin, format!("`frameworks.{key}` must be an array"))
        })?;
        let mut infos: Vec<FrameworkInfo> = Vec::new();
        for item in items {
            let info = parse_framework(plugin, item)?;
            if infos.iter().any(|i| i.id == info.id) {
                return Err(PluginError::protocol(
                    plugin,
                    format!("`frameworks.{key}` lists `{}` twice", info.id),
                ));
            }
            infos.push(info);
        }
        out.insert(role, infos);
    }
    Ok(out)
}

fn parse_framework(plugin: &str, item: &Value) -> Result<FrameworkInfo, PluginError> {
    let id = text_field(plugin, item, "id", "a framework")?;
    let what = format!("framework `{id}`");
    let versions = versions_field(plugin, item, &what)?;
    let min_language =
        match item.get("min_language") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.parse::<LanguageVersion>().map_err(|e| {
                PluginError::protocol(plugin, format!("{what}: `min_language`: {e}"))
            })?),
            Some(_) => {
                return Err(PluginError::protocol(
                    plugin,
                    format!("{what}: `min_language` must be a string"),
                ));
            }
        };
    Ok(FrameworkInfo {
        id,
        versions,
        min_language,
    })
}

/// The optional `language` of `hello`: `{ "id": "kotlin", "versions": ["1.9", "2.0"] }`.
fn parse_language(plugin: &str, v: Option<&Value>) -> Result<Option<LanguageInfo>, PluginError> {
    let Some(v) = v.filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    if !v.is_object() {
        return Err(PluginError::protocol(
            plugin,
            "`language` must be an object with `id` and `versions`",
        ));
    }
    let id = text_field(plugin, v, "id", "`language`")?;
    let versions = versions_field(plugin, v, "`language`")?;
    if let Some(bad) = versions
        .iter()
        .find(|s| s.parse::<LanguageVersion>().is_err())
    {
        return Err(PluginError::protocol(
            plugin,
            format!("`language.versions` has `{bad}`, which is not a version like `17` or `1.9`"),
        ));
    }
    Ok(Some(LanguageInfo { id, versions }))
}

/// The generate timeout from the target's options (`timeout`, in seconds), and the options
/// that are passed on to the plugin (without `timeout`).
pub fn split_options(plugin: &str, options: &Value) -> Result<(Duration, Value), PluginError> {
    let bad = |m: &str| PluginError::Options {
        plugin: plugin.to_string(),
        message: m.to_string(),
    };
    let Some(map) = options.as_object() else {
        return Ok((DEFAULT_GENERATE_TIMEOUT, options.clone()));
    };
    let mut rest = map.clone();
    let timeout = match rest.remove("timeout") {
        None => DEFAULT_GENERATE_TIMEOUT,
        Some(v) => match v.as_u64() {
            Some(s) if (1..=MAX_GENERATE_TIMEOUT.as_secs()).contains(&s) => Duration::from_secs(s),
            _ => {
                return Err(bad(
                    "`timeout` must be a whole number of seconds from 1 to 600",
                ));
            }
        },
    };
    Ok((timeout, Value::Object(rest)))
}

pub fn generate_message(req: &GenRequest<'_>, options: &Value) -> Result<Value, PluginError> {
    let ir = serde_json::to_value(req.ir)
        .map_err(|e| PluginError::protocol("host", format!("cannot encode the IR: {e}")))?;
    Ok(json!({
        "type": "generate",
        "role": req.role.name(),
        "ir": ir,
        "services": req.services,
        "options": if options.is_null() { json!({}) } else { options.clone() },
        "consumes": req.consumes,
    }))
}

/// A decoded `result`.
#[derive(Debug)]
pub struct Outcome {
    pub files: FileSet,
    pub diagnostics: Vec<GenDiagnostic>,
}

/// Validates a `result` message and builds the file set (E0707 for anything refused).
pub fn parse_result(plugin: &str, msg: &Value) -> Result<Outcome, PluginError> {
    if msg.get("type").and_then(Value::as_str) != Some("result") {
        return Err(PluginError::protocol(
            plugin,
            "the reply to `generate` was not a `result`",
        ));
    }
    let diagnostics = parse_diagnostics(plugin, msg.get("diagnostics"))?;
    let list = msg
        .get("files")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if list.len() > MAX_FILES {
        return Err(PluginError::files(
            plugin,
            format!("{} files, more than the limit of {MAX_FILES}", list.len()),
        ));
    }
    let mut files = FileSet::new();
    let mut total = 0usize;
    for entry in list {
        let (path, content) = file_entry(plugin, entry)?;
        total += content.len();
        if content.len() > MAX_FILE_BYTES || total > MAX_TOTAL_BYTES {
            return Err(PluginError::files(
                plugin,
                format!("`{path}` exceeds the size limits (10 MiB per file, 200 MiB in total)"),
            ));
        }
        files
            .add(path, content)
            .map_err(|e| PluginError::files(plugin, e.to_string()))?;
    }
    Ok(Outcome { files, diagnostics })
}

fn file_entry(plugin: &str, entry: &Value) -> Result<(String, Vec<u8>), PluginError> {
    let path = entry.get("path").and_then(Value::as_str);
    let content = entry.get("content").and_then(Value::as_str);
    match (path, content) {
        (Some(p), Some(c)) => Ok((p.to_string(), c.as_bytes().to_vec())),
        _ => Err(PluginError::files(
            plugin,
            "a file entry needs a string `path` and a string `content`",
        )),
    }
}

/// Plugin diagnostics. Warnings are returned with code prefix `W`; errors make the run fail.
fn parse_diagnostics(plugin: &str, v: Option<&Value>) -> Result<Vec<GenDiagnostic>, PluginError> {
    let mut out = Vec::new();
    for d in v
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .take(MAX_DIAGNOSTICS)
    {
        let severity = d.get("severity").and_then(Value::as_str).unwrap_or("error");
        let code = d.get("code").and_then(Value::as_str).unwrap_or("E0899");
        let message = d
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("(no message)");
        let at = d
            .get("ir_ref")
            .and_then(Value::as_str)
            .map(|r| format!(" ({r})"))
            .unwrap_or_default();
        let text = format!("plugin `{plugin}`: {message}{at}");
        match severity {
            "error" => out.push(GenDiagnostic::new(leak(code), text)),
            "warning" => out.push(GenDiagnostic::new(leak(&format!("W:{code}")), text)),
            other => {
                return Err(PluginError::protocol(
                    plugin,
                    format!("unknown diagnostic severity `{other}`"),
                ));
            }
        }
    }
    Ok(out)
}

/// Diagnostic codes are `&'static str`; plugin codes are leaked (at most 100 short strings per run).
fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Whether a diagnostic from `parse_diagnostics` is a warning.
#[must_use]
pub fn is_warning(d: &GenDiagnostic) -> bool {
    d.code.starts_with("W:")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello_reply(extra: Value) -> Value {
        let mut v = json!({"type":"hello","protocol":"1","name":"k","version":"1.0.0","roles":["client"],"capabilities":["auth"]});
        if let (Some(m), Some(e)) = (v.as_object_mut(), extra.as_object()) {
            m.extend(e.clone());
        }
        v
    }

    #[test]
    fn handshake_accepts_a_valid_hello() {
        let h = parse_handshake("k", &hello_reply(json!({}))).unwrap();
        assert_eq!(
            (h.name.as_str(), h.version.as_str(), h.roles.clone()),
            ("k", "1.0.0", vec![Role::Client])
        );
        assert!(
            h.capabilities.supports(Feature::Auth) && !h.capabilities.supports(Feature::Unions)
        );
        assert!(parse_handshake("k", &hello_reply(json!({"protocol":"1.4"}))).is_ok());
    }

    #[test]
    fn a_plugin_that_does_not_list_the_rules_features_does_not_support_them() {
        // An older plugin knows no rules feature: it is not treated as supporting any.
        let older = parse_handshake("k", &hello_reply(json!({}))).unwrap();
        for f in Feature::RULES {
            assert!(!older.capabilities.supports(f), "{}", f.name());
        }
        let newer = json!({"capabilities": ["auth", "rules_invariants"]});
        let h = parse_handshake("k", &hello_reply(newer)).unwrap();
        assert!(h.capabilities.supports(Feature::RulesInvariants));
        assert!(!h.capabilities.supports(Feature::RulesEndpoint));
    }

    #[test]
    fn handshake_without_frameworks_or_language_still_parses() {
        let h = parse_handshake("k", &hello_reply(json!({}))).unwrap();
        assert!(h.frameworks.is_empty() && h.language.is_none());
        assert!(h.frameworks_for(Role::Server).is_empty());
    }

    #[test]
    fn handshake_reads_frameworks_and_language() {
        let extra = json!({
            "frameworks": { "server": [ { "id": "ktor", "versions": ["2"], "min_language": "1.9" } ], "client": [] },
            "language": { "id": "kotlin", "versions": ["1.9", "2.0"] }
        });
        let h = parse_handshake("k", &hello_reply(extra)).unwrap();
        let ktor = &h.frameworks_for(Role::Server)[0];
        assert_eq!(
            (ktor.id.as_str(), ktor.versions.clone()),
            ("ktor", vec!["2".to_string()])
        );
        assert_eq!(ktor.min_language, Some(LanguageVersion::new(1, 9)));
        assert!(h.frameworks_for(Role::Client).is_empty());
        let lang = h.language.unwrap();
        assert_eq!((lang.id.as_str(), lang.versions.len()), ("kotlin", 2));
    }

    #[test]
    fn handshake_rejects_malformed_frameworks_and_language() {
        for (extra, needle) in [
            (json!({"frameworks": []}), "must be an object"),
            (json!({"frameworks": {"both": []}}), "unknown role"),
            (json!({"frameworks": {"server": {}}}), "must be an array"),
            (
                json!({"frameworks": {"server": [{"versions": ["1"]}]}}),
                "needs a string `id`",
            ),
            (
                json!({"frameworks": {"server": [{"id": "a"}, {"id": "a"}]}}),
                "twice",
            ),
            (
                json!({"frameworks": {"server": [{"id": "a", "versions": "1"}]}}),
                "array of strings",
            ),
            (
                json!({"frameworks": {"server": [{"id": "a", "min_language": "new"}]}}),
                "min_language",
            ),
            (json!({"language": "kotlin"}), "must be an object"),
            (
                json!({"language": {"versions": ["1"]}}),
                "needs a string `id`",
            ),
            (
                json!({"language": {"id": "k", "versions": ["one"]}}),
                "not a version",
            ),
        ] {
            let err = parse_handshake("k", &hello_reply(extra)).unwrap_err();
            assert_eq!(err.code(), "E0706");
            assert!(err.to_string().contains(needle), "{err}");
        }
    }

    #[test]
    fn handshake_rejects_wrong_major_and_bad_fields() {
        for (extra, needle) in [
            (json!({"protocol":"2"}), "protocol `2`"),
            (json!({"type":"result"}), "not a `hello`"),
            (json!({"roles":[]}), "at least one role"),
            (json!({"roles":["both"]}), "unknown role"),
            (json!({"capabilities":["telepathy"]}), "unknown capability"),
            (json!({"name":""}), "no `name`"),
        ] {
            let err = parse_handshake("k", &hello_reply(extra)).unwrap_err();
            assert_eq!(err.code(), "E0706");
            assert!(err.to_string().contains(needle), "{err}");
        }
    }

    #[test]
    fn timeout_option_is_validated_and_removed() {
        let (t, rest) = split_options("k", &json!({"timeout": 5, "x": 1})).unwrap();
        assert_eq!((t, rest), (Duration::from_secs(5), json!({"x": 1})));
        assert_eq!(
            split_options("k", &Value::Null).unwrap().0,
            DEFAULT_GENERATE_TIMEOUT
        );
        for bad in [
            json!({"timeout": 0}),
            json!({"timeout": 601}),
            json!({"timeout": "5"}),
            json!({"timeout": 1.5}),
        ] {
            assert_eq!(split_options("k", &bad).unwrap_err().code(), "E0802");
        }
        assert_eq!(
            split_options("k", &json!({"timeout": 600})).unwrap().0,
            MAX_GENERATE_TIMEOUT
        );
    }

    #[test]
    fn result_refuses_unsafe_paths_and_limits() {
        let ok = json!({"type":"result","files":[{"path":"a/b.txt","content":"x"}]});
        assert_eq!(parse_result("k", &ok).unwrap().files.len(), 1);
        for path in ["../x", "/abs", "C:/x", "a\\b", "a/../b", "a\u{0}b"] {
            let msg = json!({"type":"result","files":[{"path":path,"content":"x"}]});
            assert_eq!(
                parse_result("k", &msg).unwrap_err().code(),
                "E0707",
                "{path:?}"
            );
        }
        let dup =
            json!({"type":"result","files":[{"path":"a","content":""},{"path":"a","content":""}]});
        assert_eq!(parse_result("k", &dup).unwrap_err().code(), "E0707");
        let big = "x".repeat(MAX_FILE_BYTES + 1);
        let msg = json!({"type":"result","files":[{"path":"a","content":big}]});
        assert_eq!(parse_result("k", &msg).unwrap_err().code(), "E0707");
        let many: Vec<Value> = (0..=MAX_FILES)
            .map(|i| json!({"path": format!("f{i}"), "content": ""}))
            .collect();
        assert_eq!(
            parse_result("k", &json!({"type":"result","files":many}))
                .unwrap_err()
                .code(),
            "E0707"
        );
        assert_eq!(
            parse_result("k", &json!({"type":"hello"}))
                .unwrap_err()
                .code(),
            "E0706"
        );
    }

    #[test]
    fn diagnostics_split_into_errors_and_warnings() {
        let msg = json!({"type":"result","files":[],"diagnostics":[
            {"code":"E0801","severity":"error","message":"no","ir_ref":"decl:3"},
            {"code":"W1","severity":"warning","message":"hmm"}]});
        let out = parse_result("k", &msg).unwrap();
        assert_eq!(out.diagnostics.len(), 2);
        assert!(!is_warning(&out.diagnostics[0]) && is_warning(&out.diagnostics[1]));
        assert!(out.diagnostics[0].message.contains("(decl:3)"));
    }
}

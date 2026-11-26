//! The typed runtime config of a target (`spec/runtime-config.md`): the one description that every
//! generator builds its `<Target>Runtime` type, its check and its messages from.
//!
//! The module decides from data, never from an emitter's name: which settings a target has
//! (section 2), what the type is called (section 3), and the exact text of every problem
//! (section 4). It also holds a *reference check*: [`RuntimeModel::check`], [`RuntimeModel::check_map`]
//! and [`RuntimeModel::check_env`] are what a generated check must agree with, and
//! `tests/wire/runtime-config/cases.json` pins that agreement.
//!
//! Orders (`spec/runtime-config.md` section 9): the settings, the problems and every generated list
//! of paths are in byte order of the path text (so `services.a-b.url` comes before
//! `services.a.url`, because `-` sorts before `.`). The members of a nested type or schema object
//! are sorted by key at each level.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use veld_ir::{DeclId, Ir, PackageId, PackageKind, ServiceId};

use crate::generator::{GenDiagnostic, GenRequest, Role};
use crate::naming;
use crate::profile::Profile;

/// The path of the rule output mode.
pub const RULES_OUTPUT: &str = "rules.output";
/// The values of [`RULES_OUTPUT`], in the order the message lists them.
pub const RULE_OUTPUT_VALUES: &[&str] = &["strict", "audit", "off"];
/// The default of [`RULES_OUTPUT`].
pub const RULE_OUTPUT_DEFAULT: &str = "strict";
/// The prefix of the text of the whole error.
pub const ERROR_PREFIX: &str = "invalid runtime config: ";
/// The path used for a problem with the whole value (it is not an object).
pub const ROOT_PATH: &str = "runtime config";

/// What a setting holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    /// An absolute `http` or `https` URL ([`normalize_url`]).
    Url,
    /// One of a fixed list of strings.
    OneOf(&'static [&'static str]),
}

/// The workspace service a URL setting is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRef {
    /// The service as the workspace spells it (`card-service`): the name of its package.
    pub name: String,
    /// The IR services of that package (a package may declare several `service` blocks).
    pub ids: Vec<ServiceId>,
}

/// One setting of a runtime config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// `services.iam.url`.
    pub path: String,
    /// `["services", "iam", "url"]`.
    pub segments: Vec<String>,
    pub kind: SettingKind,
    pub required: bool,
    /// The value used when the setting is absent (optional settings only).
    pub default: Option<String>,
    /// The service of a `services.<service>.url` setting.
    pub service: Option<ServiceRef>,
}

impl Setting {
    /// A setting at `path`, with the segments split at `.`.
    #[must_use]
    pub fn new(path: &str, kind: SettingKind, required: bool, default: Option<&str>) -> Self {
        Self {
            path: path.to_string(),
            segments: path.split('.').map(str::to_string).collect(),
            kind,
            required,
            default: default.map(str::to_string),
            service: None,
        }
    }

    /// `services.<name>.url`, required.
    #[must_use]
    pub fn service_url(name: &str, ids: Vec<ServiceId>) -> Self {
        let mut s = Self::new(
            &format!("services.{name}.url"),
            SettingKind::Url,
            true,
            None,
        );
        s.service = Some(ServiceRef {
            name: name.to_string(),
            ids,
        });
        s
    }

    /// `rules.output`, optional, default `strict`.
    #[must_use]
    pub fn rules_output() -> Self {
        Self::new(
            RULES_OUTPUT,
            SettingKind::OneOf(RULE_OUTPUT_VALUES),
            false,
            Some(RULE_OUTPUT_DEFAULT),
        )
    }
}

/// One problem of a runtime config: the path and the message (which starts with the path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

impl Problem {
    fn new(path: &str, message: String) -> Self {
        Self {
            path: path.to_string(),
            message,
        }
    }
}

/// The settings with a value, by path: URLs without trailing `/`, defaults filled in. Optional
/// settings without a default and without a value are absent.
pub type Values = BTreeMap<String, String>;

// ---------------------------------------------------------------------------
// Messages (section 4). No generator types one of these itself.
// ---------------------------------------------------------------------------

/// `<path> is missing`.
#[must_use]
pub fn missing(path: &str) -> String {
    format!("{path} is missing")
}

/// `<path> must be an http or https URL`.
#[must_use]
pub fn not_a_url(path: &str) -> String {
    format!("{path} must be an http or https URL")
}

/// `<path> must be "strict", "audit" or "off"`: one value, `"a"`; two, `"a" or "b"`; more,
/// `"a", "b" or "c"`.
#[must_use]
pub fn not_in_list(path: &str, values: &[&str]) -> String {
    let quoted: Vec<String> = values.iter().map(|v| format!("\"{v}\"")).collect();
    let list = match quoted.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
    };
    format!("{path} must be {list}")
}

/// `<path> is not a setting`.
#[must_use]
pub fn not_a_setting(path: &str) -> String {
    format!("{path} is not a setting")
}

/// `<path> is missing (environment variable <NAME> is not set)`.
#[must_use]
pub fn missing_env(path: &str, name: &str) -> String {
    format!("{path} is missing (environment variable {name} is not set)")
}

/// `<path> must be an object`: a value that holds settings (`services`, `rules`, the value
/// itself) is something else. Not in the section 4 table; see section 9 of the spec.
#[must_use]
pub fn not_an_object(path: &str) -> String {
    format!("{path} must be an object")
}

/// The text of the whole error: `invalid runtime config: ` and the messages of `problems` in the
/// order given (the checks return them in path order), separated by `; `.
#[must_use]
pub fn error_text(problems: &[Problem]) -> String {
    let messages: Vec<&str> = problems.iter().map(|p| p.message.as_str()).collect();
    format!("{ERROR_PREFIX}{}", messages.join("; "))
}

// ---------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------

/// A target name in Pascal case: words split at every character that is not a letter or a digit
/// and at a lower-to-upper change, each capitalised (`card-service` is `CardService`, `my_api`
/// is `MyApi`, `v1.api` is `V1Api`, `web2` is `Web2`). A name that would start with a digit gets a
/// `T` first (`3d-app` is `T3dApp`), and a name with no letter or digit is `Target`.
#[must_use]
pub fn pascal_target(name: &str) -> String {
    let pascal = naming::pascal(name);
    match pascal.chars().next() {
        None => "Target".to_string(),
        Some(c) if c.is_ascii_digit() => format!("T{pascal}"),
        Some(_) => pascal,
    }
}

/// `<Target>Runtime`: `bookings` gives `BookingsRuntime`.
#[must_use]
pub fn runtime_type_name(target: &str) -> String {
    format!("{}Runtime", pascal_target(target))
}

// ---------------------------------------------------------------------------
// URLs
// ---------------------------------------------------------------------------

/// The URL rule, literally, so that every language can implement it without a URL library.
/// Returns the URL without its trailing `/` characters, or `None` when it is not valid.
///
/// 1. Every character is visible ASCII (`!` to `~`): no space, no control character, no
///    non-ASCII character (a host name is written in punycode).
/// 2. It starts with `http://` or `https://`, the scheme in any case.
/// 3. The authority is the text after `://` up to the first `/`, `?` or `#`. The host and port
///    are what follows the last `@` of the authority (a user and password are allowed).
/// 4. A host in square brackets (`[::1]`) has a non-empty inside of hexadecimal digits, `:` and
///    `.`, and is followed by nothing or by a port. Any other host is not empty and uses only
///    letters, digits, `-`, `.` and `_`.
/// 5. A port is `:` and one to five digits, at most 65535. `http://host:` is not valid.
/// 6. The result is the text with every trailing `/` removed.
#[must_use]
pub fn normalize_url(text: &str) -> Option<String> {
    if !text.chars().all(|c| ('!'..='~').contains(&c)) {
        return None;
    }
    let rest = after_scheme(text)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or("");
    if !host_and_port_ok(host_port) {
        return None;
    }
    Some(text.trim_end_matches('/').to_string())
}

fn after_scheme(text: &str) -> Option<&str> {
    for scheme in ["https://", "http://"] {
        let head = text.get(..scheme.len());
        if head.is_some_and(|h| h.eq_ignore_ascii_case(scheme)) {
            return text.get(scheme.len()..);
        }
    }
    None
}

fn host_and_port_ok(host_port: &str) -> bool {
    if let Some(inner) = host_port.strip_prefix('[') {
        let Some((addr, after)) = inner.split_once(']') else {
            return false;
        };
        let addr_ok = !addr.is_empty()
            && addr
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.');
        return addr_ok && (after.is_empty() || port_ok(after));
    }
    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (host_port, None),
    };
    let host_ok = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'));
    host_ok && port.is_none_or(|p| port_ok(&format!(":{p}")))
}

/// `:` and one to five digits, at most 65535.
fn port_ok(text: &str) -> bool {
    let Some(digits) = text.strip_prefix(':') else {
        return false;
    };
    !digits.is_empty()
        && digits.len() <= 5
        && digits.chars().all(|c| c.is_ascii_digit())
        && digits.parse::<u32>().is_ok_and(|n| n <= 65535)
}

// ---------------------------------------------------------------------------
// What a target needs
// ---------------------------------------------------------------------------

/// Whether the contract of `services` "has rules" in the sense of `spec/rules.md` 5.3, which
/// decides whether a server has the output mode: an endpoint of the services has a statement or a
/// version guard, or a declaration they emit has an invariant. The declarations are those the
/// services' endpoints reach, every non-event declaration of the services' own packages (reached
/// or not), the prelude, the events of the services and what those reach. It is the predicate the
/// TypeScript and Node generators use for `uses_rules`. The services a target only *consumes* do
/// not count: their clients check their own invariants, and the output mode is about what this
/// server sends.
#[must_use]
pub fn contract_has_rules(ir: &Ir, services: &[ServiceId]) -> bool {
    let endpoint = services.iter().any(|s| {
        ir.service(*s)
            .endpoints
            .iter()
            .any(|ep| !ep.rules.is_empty())
    });
    if endpoint {
        return true;
    }
    let own: BTreeSet<PackageId> = services.iter().map(|s| ir.service(*s).package).collect();
    let mut set: BTreeSet<DeclId> = ir.reachable_decls(services).into_iter().collect();
    set.extend(ir.events_of(services));
    set.extend(ir.event_reachable_decls(services));
    for d in &ir.decls {
        let in_scope =
            ir.package(d.package).kind == PackageKind::Prelude || own.contains(&d.package);
        if in_scope && !d.is_event() {
            set.insert(d.id);
        }
    }
    set.iter().any(|id| !ir.decl(*id).rules().is_empty())
}

/// The settings of a target, in path order; empty when the target has none. A client target has
/// `services.<service>.url` for each service it covers (`services`); a server has one for each
/// service in `consumes` (the request leaves `consumes` empty when `clients = false`) and
/// `rules.output` when [`contract_has_rules`]. One path per workspace service (package), however
/// many `service` blocks it has.
#[must_use]
pub fn settings_for(
    ir: &Ir,
    role: Role,
    services: &[ServiceId],
    consumes: &[ServiceId],
) -> Vec<Setting> {
    let covered = match role {
        Role::Client => services,
        Role::Server => consumes,
    };
    let mut by_package: BTreeMap<String, Vec<ServiceId>> = BTreeMap::new();
    for sid in covered {
        let name = ir.package(ir.service(*sid).package).name.clone();
        let ids = by_package.entry(name).or_default();
        if !ids.contains(sid) {
            ids.push(*sid);
        }
    }
    let mut settings: Vec<Setting> = by_package
        .into_iter()
        .map(|(name, ids)| Setting::service_url(&name, ids))
        .collect();
    if role == Role::Server && contract_has_rules(ir, services) {
        settings.push(Setting::rules_output());
    }
    settings.sort_by(|a, b| a.path.cmp(&b.path));
    settings
}

/// The runtime config of one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeModel {
    target: String,
    settings: Vec<Setting>,
}

impl RuntimeModel {
    /// A model from settings, sorted into path order. For tests and the golden file.
    #[must_use]
    pub fn from_settings(target: &str, mut settings: Vec<Setting>) -> Self {
        settings.sort_by(|a, b| a.path.cmp(&b.path));
        Self {
            target: target.to_string(),
            settings,
        }
    }

    /// The model of target `target` (its name in `veld.toml`), or `None` when the target has no
    /// setting: nothing is generated for it.
    #[must_use]
    pub fn new(
        ir: &Ir,
        target: &str,
        role: Role,
        services: &[ServiceId],
        consumes: &[ServiceId],
    ) -> Option<Self> {
        let settings = settings_for(ir, role, services, consumes);
        (!settings.is_empty()).then(|| Self::from_settings(target, settings))
    }

    /// The model of the target a generator is running for. `Ok(None)` when the target has no
    /// setting (this is decided first, so a caller without a target name keeps working for such a
    /// target).
    ///
    /// # Errors
    /// `E0802` when the target has settings and the core did not supply its name (see
    /// [`Profile::target_name`]): the name is never invented.
    pub fn for_request(
        req: &GenRequest<'_>,
        profile: &Profile,
    ) -> Result<Option<Self>, GenDiagnostic> {
        let settings = settings_for(req.ir, req.role, &req.services, &req.consumes);
        if settings.is_empty() {
            return Ok(None);
        }
        let Some(target) = profile.target_name() else {
            return Err(GenDiagnostic::new(
                "E0802",
                "the target has a runtime config but its name is unknown (the core passes it as the reserved option `target-name`)",
            ));
        };
        Ok(Some(Self::from_settings(target, settings)))
    }

    /// The target's name as the workspace spells it.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// `<Target>Runtime`.
    #[must_use]
    pub fn type_name(&self) -> String {
        runtime_type_name(&self.target)
    }

    /// The settings in path order.
    #[must_use]
    pub fn settings(&self) -> &[Setting] {
        &self.settings
    }

    /// The setting at `path`.
    #[must_use]
    pub fn setting(&self, path: &str) -> Option<&Setting> {
        self.settings.iter().find(|s| s.path == path)
    }

    /// The URL settings, in path order.
    pub fn urls(&self) -> impl Iterator<Item = &Setting> {
        self.settings.iter().filter(|s| s.kind == SettingKind::Url)
    }

    /// The settings that must have a value.
    pub fn required(&self) -> impl Iterator<Item = &Setting> {
        self.settings.iter().filter(|s| s.required)
    }

    /// Whether some setting is required (the server entry then takes the runtime config as a
    /// required argument).
    #[must_use]
    pub fn has_required(&self) -> bool {
        self.settings.iter().any(|s| s.required)
    }
}

// ---------------------------------------------------------------------------
// The reference check
// ---------------------------------------------------------------------------

enum Position {
    Leaf,
    Branch,
    Unknown,
}

impl RuntimeModel {
    fn position(&self, prefix: &[String]) -> Position {
        if self.settings.iter().any(|s| s.segments == prefix) {
            Position::Leaf
        } else if self.settings.iter().any(|s| s.segments.starts_with(prefix)) {
            Position::Branch
        } else {
            Position::Unknown
        }
    }

    /// Checks a nested value (the JSON a TypeScript object, a `serde` source or a YAML file
    /// gives): an object with `services` and `rules` objects. `null` and an absent value are the
    /// same. Returns the values, or every problem in path order.
    ///
    /// # Errors
    /// Every problem of section 4, sorted by path.
    pub fn check(&self, value: &Value) -> Result<Values, Vec<Problem>> {
        let mut problems = Vec::new();
        let mut found = BTreeMap::new();
        match value {
            Value::Object(map) => self.walk(map, &mut Vec::new(), &mut found, &mut problems),
            Value::Null => {}
            Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
                problems.push(Problem::new(ROOT_PATH, not_an_object(ROOT_PATH)));
            }
        }
        self.finish(&found, problems, &BTreeSet::new())
    }

    fn walk(
        &self,
        map: &serde_json::Map<String, Value>,
        prefix: &mut Vec<String>,
        found: &mut BTreeMap<String, Value>,
        problems: &mut Vec<Problem>,
    ) {
        for (key, value) in map {
            prefix.push(key.clone());
            let path = prefix.join(".");
            match self.position(prefix) {
                Position::Leaf => {
                    found.insert(path, value.clone());
                }
                Position::Unknown => problems.push(Problem::new(&path, not_a_setting(&path))),
                Position::Branch => match value {
                    Value::Object(inner) => self.walk(inner, prefix, found, problems),
                    Value::Null => {}
                    Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
                        problems.push(Problem::new(&path, not_an_object(&path)));
                    }
                },
            }
            prefix.pop();
        }
    }

    /// Checks a flat map from path to text (`fromMap` of Java and Rust, a `Properties` file).
    ///
    /// # Errors
    /// Every problem of section 4, sorted by path.
    pub fn check_map(&self, map: &BTreeMap<String, String>) -> Result<Values, Vec<Problem>> {
        let mut problems = Vec::new();
        let mut found = BTreeMap::new();
        for (path, text) in map {
            if self.setting(path).is_some() {
                found.insert(path.clone(), Value::String(text.clone()));
            } else {
                problems.push(Problem::new(path, not_a_setting(path)));
            }
        }
        self.finish(&found, problems, &BTreeSet::new())
    }

    /// `fromEnv`: `names` maps paths to the names of environment variables, `env` holds the
    /// environment. A variable that is not set, or is empty, counts as not set: a required path
    /// then has the problem [`missing_env`], an optional one keeps its default. A required path
    /// that `names` does not list is [`missing`]. A key of `names` that is not a path is
    /// [`not_a_setting`]. What the variables hold is checked like any other value.
    ///
    /// # Errors
    /// Every problem of section 4, sorted by path.
    pub fn check_env(
        &self,
        names: &BTreeMap<String, String>,
        env: &BTreeMap<String, String>,
    ) -> Result<Values, Vec<Problem>> {
        let mut problems = Vec::new();
        let mut found = BTreeMap::new();
        let mut reported = BTreeSet::new();
        for path in names.keys().filter(|p| self.setting(p).is_none()) {
            problems.push(Problem::new(path, not_a_setting(path)));
        }
        for s in &self.settings {
            let Some(name) = names.get(&s.path) else {
                continue;
            };
            match env.get(name).filter(|v| !v.is_empty()) {
                Some(text) => {
                    found.insert(s.path.clone(), Value::String(text.clone()));
                }
                None if s.required => {
                    problems.push(Problem::new(&s.path, missing_env(&s.path, name)));
                    reported.insert(s.path.clone());
                }
                None => {}
            }
        }
        self.finish(&found, problems, &reported)
    }

    /// Checks every setting against the value found for it (the settings already reported are
    /// skipped), and returns the values or all problems sorted by path, then by message.
    fn finish(
        &self,
        found: &BTreeMap<String, Value>,
        mut problems: Vec<Problem>,
        reported: &BTreeSet<String>,
    ) -> Result<Values, Vec<Problem>> {
        let mut values = Values::new();
        for s in self.settings.iter().filter(|s| !reported.contains(&s.path)) {
            match leaf(s, found.get(&s.path)) {
                Ok(Some(v)) => {
                    values.insert(s.path.clone(), v);
                }
                Ok(None) => {}
                Err(message) => problems.push(Problem::new(&s.path, message)),
            }
        }
        if problems.is_empty() {
            return Ok(values);
        }
        problems.sort_by(|a, b| (&a.path, &a.message).cmp(&(&b.path, &b.message)));
        Err(problems)
    }
}

/// The value of one setting: `Ok(None)` for an optional setting with neither a value nor a
/// default, `Err` with the message of its problem. An absent value, `null` and, for a required
/// setting, the empty string are absent. A value that is not a string is the wrong value.
fn leaf(s: &Setting, value: Option<&Value>) -> Result<Option<String>, String> {
    let text = match value {
        None | Some(Value::Null) => return absent(s),
        Some(Value::String(t)) => t,
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
            return Err(wrong_value(s));
        }
    };
    if text.is_empty() && s.required {
        return Err(missing(&s.path));
    }
    match s.kind {
        SettingKind::Url => normalize_url(text).map(Some).ok_or_else(|| wrong_value(s)),
        SettingKind::OneOf(list) if list.contains(&text.as_str()) => Ok(Some(text.clone())),
        SettingKind::OneOf(_) => Err(wrong_value(s)),
    }
}

fn absent(s: &Setting) -> Result<Option<String>, String> {
    if s.required {
        Err(missing(&s.path))
    } else {
        Ok(s.default.clone())
    }
}

fn wrong_value(s: &Setting) -> String {
    match s.kind {
        SettingKind::Url => not_a_url(&s.path),
        SettingKind::OneOf(list) => not_in_list(&s.path, list),
    }
}

#[cfg(test)]
mod tests;

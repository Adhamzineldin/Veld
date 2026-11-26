//! The typed runtime config (`spec/runtime-config.md`, `spec/targets/rust.md` section 15): the
//! `src/runtime_config.rs` module of a target that has settings, with its struct, its check, its
//! readers and the function that builds the clients of the target.
//!
//! Every setting, path and message comes from `veld_codegen::runtime_config`; the engine that
//! runs the check is `runtime/config.rs` (`veld::config`).

use std::collections::BTreeSet;

use veld_codegen::runtime_config::{self as rc, RuntimeModel, Setting, SettingKind};
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic, Role};
use veld_ir::{Ir, Service, ServiceId};

use crate::client::{Transports, client_mod};
use crate::tree::Tree;
use crate::util::{add_file, bare, lit, module, snake, type_name, unsupported};

/// The module of the runtime config.
pub const MODULE: &str = "runtime_config";

/// What the server entry of the target needs to know about its runtime config.
#[derive(Debug, Clone)]
pub struct RuntimeCfg {
    /// `BookingsRuntime`.
    pub type_name: String,
    /// True when no setting is required: the entry takes an `Option`.
    pub optional: bool,
    /// True when the config has `rules.output`: the hooks no longer have `rules_output`.
    pub output: bool,
}

impl RuntimeCfg {
    /// The facts of `model`.
    #[must_use]
    pub fn of(model: &RuntimeModel) -> Self {
        RuntimeCfg {
            type_name: model.type_name(),
            optional: !model.has_required(),
            output: model.setting(rc::RULES_OUTPUT).is_some(),
        }
    }

    /// The parameter of the server entry, after the hooks (and the loaders).
    #[must_use]
    pub fn param(&self) -> String {
        let ty = format!("crate::{MODULE}::{}", self.type_name);
        if self.optional {
            format!("runtime: Option<&{ty}>")
        } else {
            format!("runtime: &{ty}")
        }
    }

    /// The statement that checks the runtime config. `runtime` is the checked copy after it when
    /// the entry reads a setting (`rules.output`).
    #[must_use]
    pub fn check_stmt(&self) -> String {
        let ty = format!("crate::{MODULE}::{}", self.type_name);
        let bind = if self.output { "let runtime = " } else { "" };
        if self.optional {
            format!("{bind}runtime.map_or_else(|| Ok({ty}::default()), {ty}::check)?;")
        } else {
            format!("{bind}runtime.check()?;")
        }
    }

    /// The error of the server entry.
    #[must_use]
    pub fn error_type() -> String {
        format!("crate::{MODULE}::ConfigError")
    }
}

/// A text of the model with a hole at `hole`: the part before it and the part after it.
fn template(make: impl Fn(&str) -> String) -> (String, String) {
    let hole = "\u{0}";
    let text = make(hole);
    match text.split_once(hole) {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (text, String::new()),
    }
}

fn tuple(t: (String, String)) -> String {
    format!("({}, {})", lit(&t.0), lit(&t.1))
}

/// One URL setting: the field of `services`, and the clients it feeds.
struct ServiceEntry<'a> {
    setting: &'a Setting,
    segment: String,
    field: String,
}

fn field_name(segment: &str) -> String {
    snake(segment)
}

fn serde_rename(field: &str, segment: &str) -> Option<String> {
    (bare(field) != segment).then(|| format!("#[serde(rename = {})]", lit(segment)))
}

fn services<'a>(model: &'a RuntimeModel) -> Result<Vec<ServiceEntry<'a>>, Vec<GenDiagnostic>> {
    let mut out: Vec<ServiceEntry<'a>> = Vec::new();
    for s in model.urls() {
        let segment = s.segments.get(1).cloned().unwrap_or_default();
        let field = field_name(&segment);
        if out.iter().any(|o| o.field == field) {
            return Err(unsupported(format!(
                "generator `rust` cannot name two services of the runtime config `{}` the same Rust field (`{field}`); rename one of the services",
                model.type_name()
            )));
        }
        out.push(ServiceEntry {
            setting: s,
            segment,
            field,
        });
    }
    out.sort_by(|a, b| a.segment.cmp(&b.segment));
    Ok(out)
}

/// Emits `src/runtime_config.rs`.
///
/// # Errors
/// `E0801` for two services whose Rust field names clash, or a path problem.
pub fn emit(
    files: &mut FileSet,
    model: &RuntimeModel,
    ir: &Ir,
    role: Role,
    tr: Transports,
) -> Result<(), Vec<GenDiagnostic>> {
    let svcs = services(model)?;
    let prefix = rc::pascal_target(model.target());
    let mut w = CodeWriter::with_unit("    ");
    w.line(format!(
        "//! The runtime config of the target `{}` (`spec/runtime-config.md`): what a deployment chooses.",
        model.target()
    ));
    w.line("//!");
    w.line("//! Fill it from any `serde` source, from a flat map of paths (`from_map`), from environment");
    w.line("//! variables whose names you choose (`from_env`) or from a nested JSON value (`from_value`).");
    w.line("//! Every way ends in `check`, which reports every problem at once.")
        .blank();
    imports(&mut w);
    if model.setting(rc::RULES_OUTPUT).is_some() {
        output_mode(&mut w);
    }
    structs(&mut w, model, &svcs, &prefix);
    table(&mut w, model);
    methods(&mut w, model, &svcs);
    if !svcs.is_empty() {
        clients(&mut w, ir, role, tr, &svcs, (&prefix, &model.type_name()))?;
    }
    add_file(files, &format!("src/{MODULE}.rs"), w.finish())
}

fn imports(w: &mut CodeWriter) {
    w.line("use std::collections::BTreeMap;").blank();
    w.line("use crate::veld::config::{Kind, Setting, Table, Values};");
    w.line("pub use crate::veld::config::{ConfigError, Problem};")
        .blank();
}

fn output_mode(w: &mut CodeWriter) {
    w.line("/// How a server treats a violated invariant of its own output (`rules.output`).");
    w.line("#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]");
    w.line("#[serde(rename_all = \"lowercase\")]");
    w.open("pub enum RuleOutputMode");
    w.line("/// Answer `500 Internal`; the violation goes to the hook. The default.");
    w.line("#[default]");
    w.line("Strict,");
    w.line("/// Send the response unchanged; the violation goes to the hook.");
    w.line("Audit,");
    w.line("/// Do not check the output rules.");
    w.line("Off,");
    w.close().blank();
    w.open("impl RuleOutputMode");
    w.open("fn text(self) -> &'static str");
    w.open("match self");
    w.line("RuleOutputMode::Strict => \"strict\",");
    w.line("RuleOutputMode::Audit => \"audit\",");
    w.line("RuleOutputMode::Off => \"off\",");
    w.close().close().blank();
    w.open("fn parse(text: &str) -> Self");
    w.open("match text");
    w.line("\"audit\" => RuleOutputMode::Audit,");
    w.line("\"off\" => RuleOutputMode::Off,");
    w.line("_ => RuleOutputMode::Strict,");
    w.close().close().close().blank();
    w.open("impl From<RuleOutputMode> for crate::veld::rules::RuleOutput");
    w.open("fn from(mode: RuleOutputMode) -> Self");
    w.open("match mode");
    w.line("RuleOutputMode::Strict => crate::veld::rules::RuleOutput::Strict,");
    w.line("RuleOutputMode::Audit => crate::veld::rules::RuleOutput::Audit,");
    w.line("RuleOutputMode::Off => crate::veld::rules::RuleOutput::Off,");
    w.close().close().close().blank();
}

fn structs(w: &mut CodeWriter, model: &RuntimeModel, svcs: &[ServiceEntry<'_>], prefix: &str) {
    let has_output = model.setting(rc::RULES_OUTPUT).is_some();
    if has_output {
        w.line("/// The `rules` settings.");
        w.line("#[derive(Debug, Clone, Default, serde::Deserialize)]");
        w.line("#[serde(default, deny_unknown_fields)]");
        w.open("pub struct RulesSettings");
        w.line("/// `rules.output`: default `strict`.");
        w.line("pub output: RuleOutputMode,");
        w.close().blank();
    }
    if !svcs.is_empty() {
        w.line("/// The settings of one service.");
        w.line("#[derive(Debug, Clone, serde::Deserialize)]");
        w.line("#[serde(deny_unknown_fields)]");
        w.open("pub struct ServiceSettings");
        w.line("/// `services.<service>.url`: an absolute `http` or `https` URL.");
        w.line("pub url: String,");
        w.close().blank();
        w.line("/// `services`: one entry for each service the target calls.");
        w.line("#[derive(Debug, Clone, serde::Deserialize)]");
        w.line("#[serde(deny_unknown_fields)]");
        w.open(format!("pub struct {prefix}Services"));
        for s in svcs {
            if let Some(attr) = serde_rename(&s.field, &s.segment) {
                w.line(attr);
            }
            w.line(format!("pub {}: ServiceSettings,", s.field));
        }
        w.close().blank();
    }
    w.line(format!(
        "/// The runtime config of the target `{}`.",
        model.target()
    ));
    let default = if model.has_required() {
        ""
    } else {
        " Default,"
    };
    w.line(format!(
        "#[derive(Debug, Clone,{default} serde::Deserialize)]"
    ));
    w.line("#[serde(deny_unknown_fields)]");
    w.open(format!("pub struct {}", model.type_name()));
    if has_output {
        w.line("#[serde(default)]");
        w.line("pub rules: RulesSettings,");
    }
    if !svcs.is_empty() {
        w.line(format!("pub services: {prefix}Services,"));
    }
    w.close().blank();
}

fn setting_literal(s: &Setting) -> String {
    let kind = match s.kind {
        SettingKind::Url => "Kind::Url".to_string(),
        SettingKind::OneOf(list) => {
            let items: Vec<String> = list.iter().map(|v| lit(v)).collect();
            format!("Kind::OneOf(&[{}])", items.join(", "))
        }
    };
    let wrong = match s.kind {
        SettingKind::Url => rc::not_a_url(&s.path),
        SettingKind::OneOf(list) => rc::not_in_list(&s.path, list),
    };
    let default = s
        .default
        .as_deref()
        .map_or("None".to_string(), |d| format!("Some({})", lit(d)));
    let env = tuple(template(|name| rc::missing_env(&s.path, name)));
    format!(
        "Setting {{ path: {}, kind: {kind}, required: {}, default: {default}, missing: {}, wrong: {}, env_missing: {env} }},",
        lit(&s.path),
        s.required,
        lit(&rc::missing(&s.path)),
        lit(&wrong)
    )
}

fn table(w: &mut CodeWriter, model: &RuntimeModel) {
    w.line("/// The settings, in path order, and the text of every message.");
    w.open_with("const SETTINGS: &[Setting] = &", "[");
    for s in model.settings() {
        w.line(setting_literal(s));
    }
    w.close_with("];").blank();
    w.open("const TABLE: Table = Table");
    w.line("settings: SETTINGS,");
    w.line(format!("error_prefix: {},", lit(rc::ERROR_PREFIX)));
    w.line(format!(
        "not_a_setting: {},",
        tuple(template(rc::not_a_setting))
    ));
    w.line(format!(
        "not_an_object: {},",
        tuple(template(rc::not_an_object))
    ));
    w.line(format!(
        "root_not_an_object: {},",
        lit(&rc::not_an_object(rc::ROOT_PATH))
    ));
    w.close_with("};").blank();
}

fn methods(w: &mut CodeWriter, model: &RuntimeModel, svcs: &[ServiceEntry<'_>]) {
    let ty = model.type_name();
    w.open(format!("impl {ty}"));
    values_fn(w, model, svcs);
    from_values_fn(w, model, svcs);
    w.line("/// Checks the values of this config and returns the checked copy: URLs without a trailing `/`.");
    w.line("///");
    w.line("/// # Errors");
    w.line("/// [`ConfigError`], with every problem, in path order.");
    w.open("pub fn check(&self) -> Result<Self, ConfigError>");
    w.line("TABLE.check_map(&self.values()).map(|values| Self::from_values(&values))");
    w.close().blank();
    w.line("/// Builds the config from a nested JSON value (`{ \"services\": { \"iam\": { \"url\": \"...\" } } }`) and checks it.");
    w.line("/// Unlike a plain `serde` deserialization, it reports every problem with the text of the spec.");
    w.line("///");
    w.line("/// # Errors");
    w.line("/// [`ConfigError`], with every problem, in path order.");
    w.open("pub fn from_value(value: &serde_json::Value) -> Result<Self, ConfigError>");
    w.line("TABLE.check_value(value).map(|values| Self::from_values(&values))");
    w.close().blank();
    w.line("/// Builds the config from a flat map from path (`services.iam.url`) to text, and checks it.");
    w.line("///");
    w.line("/// # Errors");
    w.line("/// [`ConfigError`], with every problem, in path order.");
    w.open("pub fn from_map(map: &BTreeMap<String, String>) -> Result<Self, ConfigError>");
    w.line("TABLE.check_map(map).map(|values| Self::from_values(&values))");
    w.close().blank();
    w.line("/// Builds the config from environment variables: `names` pairs a path with the name of the");
    w.line(
        "/// variable that holds it. A variable that is not set, or is empty, counts as not set.",
    );
    w.line("///");
    w.line("/// # Errors");
    w.line("/// [`ConfigError`], with every problem, in path order.");
    w.open("pub fn from_env(names: &[(&str, &str)]) -> Result<Self, ConfigError>");
    w.line("Self::from_env_with(names, &|name| std::env::var(name).ok())");
    w.close().blank();
    w.line("/// Like [`from_env`](Self::from_env), reading the variables through `get`.");
    w.line("///");
    w.line("/// # Errors");
    w.line("/// [`ConfigError`], with every problem, in path order.");
    w.open("pub fn from_env_with(names: &[(&str, &str)], get: &dyn Fn(&str) -> Option<String>) -> Result<Self, ConfigError>");
    w.line("TABLE.check_env(names, get).map(|values| Self::from_values(&values))");
    w.close();
    if model.setting(rc::RULES_OUTPUT).is_some() {
        w.blank();
        w.line("/// The rule output mode of the server.");
        w.open("pub fn rule_output(&self) -> crate::veld::rules::RuleOutput");
        w.line("self.rules.output.into()");
        w.close();
    }
    w.close().blank();
}

fn values_fn(w: &mut CodeWriter, model: &RuntimeModel, svcs: &[ServiceEntry<'_>]) {
    w.line("/// The value of every setting, by path: defaults filled in.");
    w.open("pub fn values(&self) -> Values");
    w.line("let mut values = Values::new();");
    for s in model.settings() {
        let expr = match svcs.iter().find(|o| o.setting.path == s.path) {
            Some(o) => format!("self.services.{}.url.clone()", o.field),
            None => "self.rules.output.text().to_string()".to_string(),
        };
        w.line(format!(
            "values.insert({}.to_string(), {expr});",
            lit(&s.path)
        ));
    }
    w.line("values");
    w.close().blank();
}

fn from_values_fn(w: &mut CodeWriter, model: &RuntimeModel, svcs: &[ServiceEntry<'_>]) {
    let prefix = rc::pascal_target(model.target());
    w.open("fn from_values(values: &Values) -> Self");
    w.line("let get = |path: &str| values.get(path).cloned().unwrap_or_default();");
    w.open("Self");
    if model.setting(rc::RULES_OUTPUT).is_some() {
        w.line("rules: RulesSettings { output: RuleOutputMode::parse(&get(\"rules.output\")) },");
    }
    if !svcs.is_empty() {
        w.open(format!("services: {prefix}Services"));
        for s in svcs {
            w.line(format!(
                "{}: ServiceSettings {{ url: get({}) }},",
                s.field,
                lit(&s.setting.path)
            ));
        }
        w.close_with("},");
    }
    w.close().close().blank();
}

/// One client of the `clients` function.
struct ClientRef {
    /// The field of the clients struct.
    field: String,
    /// The path of the client type.
    path: String,
    /// The setting that gives its URL.
    url_path: String,
}

fn client_refs(
    ir: &Ir,
    role: Role,
    svcs: &[ServiceEntry<'_>],
) -> Result<Vec<ClientRef>, Vec<GenDiagnostic>> {
    let mut out: Vec<ClientRef> = Vec::new();
    let mut seen = BTreeSet::new();
    for s in svcs {
        let ids: &[ServiceId] = s.setting.service.as_ref().map_or(&[], |r| &r.ids);
        for sid in ids {
            let svc = ir.service(*sid);
            let field = snake(&svc.name);
            if !seen.insert(field.clone()) {
                return Err(unsupported(format!(
                    "generator `rust` cannot name two clients of the runtime config the same Rust field (`{field}`); rename one of the services"
                )));
            }
            out.push(ClientRef {
                field,
                path: client_path(ir, role, svc, *sid),
                url_path: s.setting.path.clone(),
            });
        }
    }
    Ok(out)
}

fn client_path(ir: &Ir, role: Role, svc: &Service, sid: ServiceId) -> String {
    let root = match role {
        Role::Client => "crate".to_string(),
        Role::Server => format!("crate::clients::{}", module(&svc.name)),
    };
    let tree = Tree::new(ir, &root, "src", vec![sid], false);
    format!(
        "{root}::{}::{}::{}Client",
        tree.pkg_mod(svc.package),
        client_mod(svc),
        type_name(&svc.name)
    )
}

fn clients(
    w: &mut CodeWriter,
    ir: &Ir,
    role: Role,
    tr: Transports,
    svcs: &[ServiceEntry<'_>],
    (prefix, ty): (&str, &str),
) -> Result<(), Vec<GenDiagnostic>> {
    let refs = client_refs(ir, role, svcs)?;
    let name = format!("{prefix}Clients");
    w.line("/// A client for each service the target calls.");
    w.line("#[derive(Debug, Clone)]");
    w.open(format!(
        "pub struct {name}<T: crate::veld::client::Transport>"
    ));
    for r in &refs {
        w.line(format!("pub {}: {}<T>,", r.field, r.path));
    }
    w.close().blank();
    w.line("/// Builds every client of the target from the runtime config, each with its URL, and passes");
    w.line("/// `options` to all of them. `transport` makes the transport of one client.");
    w.line("///");
    w.line("/// # Errors");
    w.line("/// [`ConfigError`] when the runtime config does not pass `check`.");
    w.open(format!(
        "pub fn clients_with<T: crate::veld::client::Transport>(runtime: &{ty}, options: crate::veld::client::ClientOptions, mut transport: impl FnMut() -> T) -> Result<{name}<T>, ConfigError>"
    ));
    w.line("let urls = runtime.check()?.values();");
    w.open(format!("Ok({name}"));
    for r in &refs {
        w.line(format!(
            "{}: {}::new(transport(), options.clone().base_url(urls.get({}).cloned().unwrap_or_default())).map_err(|e| client_error({}, &e))?,",
            r.field,
            r.path,
            lit(&r.url_path),
            lit(&r.url_path)
        ));
    }
    w.close_with("})");
    w.close().blank();
    w.open("fn client_error(path: &str, error: &crate::veld::Error) -> ConfigError");
    w.line(format!(
        "ConfigError::single({}, path, format!(\"{{path}}: {{error}}\"))",
        lit(rc::ERROR_PREFIX)
    ));
    w.close().blank();
    default_clients(w, tr, &name, ty);
    Ok(())
}

fn default_clients(w: &mut CodeWriter, tr: Transports, name: &str, ty: &str) {
    let transport = if tr.reqwest {
        "crate::veld::transport_reqwest::ReqwestTransport"
    } else if tr.ureq {
        "crate::veld::transport_ureq::UreqTransport"
    } else {
        return;
    };
    w.line("/// Like [`clients_with`], on the default transport of the crate.");
    w.line("///");
    w.line("/// # Errors");
    w.line("/// [`ConfigError`] when the runtime config does not pass `check`.");
    w.open(format!(
        "pub fn clients(runtime: &{ty}, options: crate::veld::client::ClientOptions) -> Result<{name}<{transport}>, ConfigError>"
    ));
    w.line(format!("clients_with(runtime, options, {transport}::new)"));
    w.close().blank();
}

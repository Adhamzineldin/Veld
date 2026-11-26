//! The typed runtime config (`spec/runtime-config.md`, `spec/targets/java.md` section 14):
//! `<Target>Runtime`, `<Target>Clients` and, for the `spring` framework, `<Target>RuntimeProperties`.
//!
//! Every path, default and message comes from the shared model (`veld_codegen::runtime_config`);
//! nothing here types one.

use veld_codegen::runtime_config::{
    ERROR_PREFIX, ROOT_PATH, RULES_OUTPUT, RuntimeModel, Setting, SettingKind, missing,
    missing_env, not_a_setting, not_a_url, not_an_object, not_in_list,
};
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic, GenRequest, Role};

use crate::tree::{FileCtx, Tree, java_path};
use crate::util::{add_file, doc_comment, java_str, member_ident, unique};

/// The default of the target option `runtime-prefix`.
pub const DEFAULT_PREFIX: &str = "veld";

/// The runtime config of one target, as the generator needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// The settings and the type name.
    pub model: RuntimeModel,
    /// The Java package of `<Target>Runtime` and `<Target>Clients`.
    pub package: String,
    /// The package of the adapters (`<Target>RuntimeProperties` lives there).
    pub adapter_package: String,
    /// The prefix of the Spring properties.
    pub prefix: String,
}

impl Info {
    /// The runtime config of a target whose output goes below `base`.
    #[must_use]
    pub fn new(model: RuntimeModel, base: &str, prefix: &str) -> Self {
        Self {
            model,
            package: package(base),
            adapter_package: crate::runtime::adapter_package(base),
            prefix: prefix.to_string(),
        }
    }

    /// `<Target>Runtime`.
    #[must_use]
    pub fn type_name(&self) -> String {
        self.model.type_name()
    }

    /// The fully qualified name of `<Target>Runtime`.
    #[must_use]
    pub fn type_fqn(&self) -> String {
        format!("{}.{}", self.package, self.type_name())
    }

    /// `<Target>Clients`.
    #[must_use]
    pub fn clients_name(&self) -> String {
        format!("{}Clients", self.type_name().trim_end_matches("Runtime"))
    }

    /// `<Target>RuntimeProperties`.
    #[must_use]
    pub fn properties_name(&self) -> String {
        format!("{}Properties", self.type_name())
    }

    /// Whether the target has the rule output mode.
    #[must_use]
    pub fn has_rules_output(&self) -> bool {
        self.model.setting(RULES_OUTPUT).is_some()
    }
}

/// The Java package of the generated runtime config classes.
#[must_use]
pub fn package(base: &str) -> String {
    format!("{base}.runtimeconfig")
}

/// The member name of a setting: its path in camel case without `services` (`iamUrl`,
/// `rulesOutput`).
pub fn member_names(model: &RuntimeModel) -> Vec<String> {
    unique(
        model
            .settings()
            .iter()
            .map(|s| {
                let segments: Vec<&str> = s
                    .segments
                    .iter()
                    .map(String::as_str)
                    .filter(|seg| *seg != "services")
                    .collect();
                member_ident(&segments.join(" "))
            })
            .collect(),
    )
}

/// The Java type of a setting's value, and whether it is the rule output enum.
fn is_output(s: &Setting) -> bool {
    s.path == RULES_OUTPUT
}

/// `new RuntimeCheck.Setting(...)` for one setting.
fn setting_literal(s: &Setting, check: &str, list: &str) -> String {
    let (url, values) = match s.kind {
        SettingKind::Url => ("true", format!("{list}.of()")),
        SettingKind::OneOf(values) => (
            "false",
            format!(
                "{list}.of({})",
                values
                    .iter()
                    .map(|v| java_str(v))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
    };
    let wrong = match s.kind {
        SettingKind::Url => not_a_url(&s.path),
        SettingKind::OneOf(list) => not_in_list(&s.path, list),
    };
    let fallback = s
        .default
        .as_deref()
        .map_or_else(|| "null".to_string(), java_str);
    format!(
        "new {check}.Setting({}, {url}, {}, {fallback}, {values}, {}, {})",
        java_str(&s.path),
        s.required,
        java_str(&missing(&s.path)),
        java_str(&wrong)
    )
}

/// What `<Target>Clients` builds: one client per service of the runtime config.
#[derive(Debug, Clone)]
pub struct ClientRef {
    /// The setting (`services.iam.url`) that gives the client its URL.
    pub path: String,
    /// The fully qualified name of the client class.
    pub class: String,
    /// The service's name, for the getter.
    pub service: String,
}

/// The clients of a target, in path order of their URL setting, then in service order.
#[must_use]
pub fn client_refs(
    req: &GenRequest<'_>,
    model: &RuntimeModel,
    base: &str,
    runtime_package: &str,
) -> Vec<ClientRef> {
    let mut out = Vec::new();
    for s in model.urls() {
        let Some(service) = &s.service else { continue };
        for sid in &service.ids {
            let svc = req.ir.service(*sid);
            let tree_base = match req.role {
                Role::Client => base.to_string(),
                Role::Server => crate::consumed_base(base, svc),
            };
            let tree = Tree::new(req.ir, &tree_base, runtime_package, vec![*sid]);
            out.push(ClientRef {
                path: s.path.clone(),
                class: format!(
                    "{}.{}",
                    tree.api_package(svc.package, "client"),
                    crate::client::class_name(svc)
                ),
                service: svc.name.clone(),
            });
        }
    }
    out
}

/// Adds `<Target>Runtime` and `<Target>Clients`.
pub fn emit(
    files: &mut FileSet,
    tree: &Tree<'_>,
    info: &Info,
    clients: &[ClientRef],
) -> Result<(), Vec<GenDiagnostic>> {
    let class = info.type_name();
    let mut fc = FileCtx::new(tree, &info.package, &class);
    let mut w = CodeWriter::new();
    write_runtime(&mut w, &mut fc, info, &class);
    add_file(
        files,
        &java_path(&info.package, &class),
        fc.finish(&w.finish()),
    )?;
    let class = info.clients_name();
    let mut fc = FileCtx::new(tree, &info.package, &class);
    let mut w = CodeWriter::new();
    write_clients(&mut w, &mut fc, info, clients, &class);
    add_file(
        files,
        &java_path(&info.package, &class),
        fc.finish(&w.finish()),
    )?;
    Ok(())
}

fn write_runtime(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, info: &Info, class: &str) {
    let model = &info.model;
    let names = member_names(model);
    let (check, exception) = (fc.rt("RuntimeCheck"), fc.rt("VeldConfigException"));
    let (list, map, tree_map) = (
        fc.name("java.util.List"),
        fc.name("java.util.Map"),
        fc.name("java.util.TreeMap"),
    );
    doc_comment(
        w,
        Some(&format!(
            "The runtime config of the target `{}`: the values a deployment chooses (spec/runtime-config.md). Immutable; build one with `builder()`, `fromMap`, `fromEnv` or `fromValue`. Every way of building it runs the same check and throws a `{exception}` with every problem.",
            model.target()
        )),
        &[],
        None,
    );
    w.open(format!("public final class {class}"));
    write_check_constant(w, model, &check, &list);
    w.blank();
    for (s, n) in model.settings().iter().zip(&names) {
        let ty = value_type(fc, s);
        w.line(format!("private final {ty} {n};"));
    }
    w.blank();
    w.open(format!("private {class}({map}<String, String> values)"));
    for (s, n) in model.settings().iter().zip(&names) {
        w.line(format!("this.{n} = {};", read_value(fc, s)));
    }
    w.close();
    write_factories(w, &(class, &map, &exception));
    for (s, n) in model.settings().iter().zip(&names) {
        w.blank();
        let ty = value_type(fc, s);
        w.line(format!("/** The value of `{}`. */", s.path));
        w.open(format!("public {ty} {n}()"));
        w.line(format!("return {n};"));
        w.close();
    }
    w.blank();
    write_builder(w, fc, model, &names, (class, &tree_map, &map));
    w.close();
}

/// `private static final RuntimeCheck CHECK = new RuntimeCheck(...)`.
fn write_check_constant(w: &mut CodeWriter, model: &RuntimeModel, check: &str, list: &str) {
    crate::util::open_call(
        w,
        format!("private static final {check} CHECK = new {check}"),
    );
    crate::util::open_call(w, format!("{list}.of"));
    let count = model.settings().len();
    for (i, s) in model.settings().iter().enumerate() {
        let comma = if i + 1 == count { ")," } else { "," };
        w.line(format!("{}{comma}", setting_literal(s, check, list)));
    }
    crate::util::end_call(w);
    w.line(format!(
        "new {check}.Texts({}, {}, {}, {}, {}));",
        java_str(&not_a_setting("{path}")),
        java_str(&not_an_object("{path}")),
        java_str(&missing_env("{path}", "{name}")),
        java_str(ERROR_PREFIX),
        java_str(ROOT_PATH)
    ));
    crate::util::end_call(w);
}

/// The Java type that holds a setting.
fn value_type(fc: &mut FileCtx<'_, '_>, s: &Setting) -> String {
    if is_output(s) {
        format!("{}.OutputMode", fc.rt("RuleWalk"))
    } else {
        "String".to_string()
    }
}

/// The expression that reads a setting out of the checked `values` map.
fn read_value(fc: &mut FileCtx<'_, '_>, s: &Setting) -> String {
    let get = format!("values.get({})", java_str(&s.path));
    if is_output(s) {
        let (walk, locale) = (fc.rt("RuleWalk"), fc.name("java.util.Locale"));
        format!("{walk}.OutputMode.valueOf({get}.toUpperCase({locale}.ROOT))")
    } else {
        get
    }
}

fn write_factories(w: &mut CodeWriter, (class, map, exception): &(&str, &str, &str)) {
    w.blank();
    w.line("/** Starts a builder. */");
    w.open("public static Builder builder()");
    w.line("return new Builder();");
    w.close();
    w.blank();
    w.line("/** Builds the runtime config from a flat map from path to text, such as a `Properties` file. */");
    w.open(format!(
        "public static {class} fromMap({map}<String, String> values)"
    ));
    w.line(format!("return new {class}(CHECK.map(values));"));
    w.close();
    w.blank();
    w.line(format!(
        "/** Builds the runtime config from a nested value: the maps and strings a JSON or YAML parser gives (`services.iam.url` is `{{services: {{iam: {{url: ...}}}}}}`). */"
    ));
    w.open(format!("public static {class} fromValue(Object value)"));
    w.line(format!("return new {class}(CHECK.value(value));"));
    w.close();
    w.blank();
    w.line("/** Builds the runtime config from environment variables: `names` maps each path to the name of its variable, and `System.getenv()` is read. */");
    w.open(format!(
        "public static {class} fromEnv({map}<String, String> names)"
    ));
    w.line("return fromEnv(names, System.getenv());");
    w.close();
    w.blank();
    w.line("/** Builds the runtime config from environment variables read from `env`. A variable that is not set and one that is empty are the same. */");
    w.open(format!(
        "public static {class} fromEnv({map}<String, String> names, {map}<String, String> env)"
    ));
    w.line(format!("return new {class}(CHECK.env(names, env));"));
    w.close();
    w.blank();
    w.line(format!(
        "/** `runtime`, or, when it is `null`, the config that has no value: it throws a `{exception}` when a path is required. Every generated function that takes a runtime config calls it first. */"
    ));
    w.open(format!("public static {class} ensure({class} runtime)"));
    w.line("return runtime != null ? runtime : builder().build();");
    w.close();
}

fn write_builder(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    model: &RuntimeModel,
    names: &[String],
    (class, tree_map, map): (&str, &str, &str),
) {
    w.line("/** Builds the runtime config; `build()` runs the check. */");
    w.open("public static final class Builder");
    for (s, n) in model.settings().iter().zip(names) {
        let ty = value_type(fc, s);
        w.line(format!("private {ty} {n};"));
    }
    w.blank();
    w.line("private Builder() {}");
    for (s, n) in model.settings().iter().zip(names) {
        w.blank();
        let ty = value_type(fc, s);
        w.line(format!("/** Sets `{}`. */", s.path));
        w.open(format!("public Builder {n}({ty} value)"));
        w.line(format!("this.{n} = value;"));
        w.line("return this;");
        w.close();
    }
    w.blank();
    w.line("/** Runs the check and builds the runtime config. */");
    w.open(format!("public {class} build()"));
    w.line(format!("{map}<String, String> given = new {tree_map}<>();"));
    for (s, n) in model.settings().iter().zip(names) {
        let text = if is_output(s) {
            let locale = fc.name("java.util.Locale");
            format!("{n}.name().toLowerCase({locale}.ROOT)")
        } else {
            n.clone()
        };
        w.line(format!(
            "if ({n} != null) {{ given.put({}, {text}); }}",
            java_str(&s.path)
        ));
    }
    w.line(format!("return {class}.fromMap(given);"));
    w.close();
    w.close();
}

fn write_clients(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    info: &Info,
    clients: &[ClientRef],
    class: &str,
) {
    let runtime = info.type_name();
    let options = fc.rt("ClientOptions");
    let getters = unique(clients.iter().map(|c| member_ident(&c.service)).collect());
    let types: Vec<String> = clients.iter().map(|c| fc.name(&c.class)).collect();
    doc_comment(
        w,
        Some(&format!(
            "Every client of the target `{}`, each with its URL from the runtime config.",
            info.model.target()
        )),
        &[],
        None,
    );
    w.open(format!("public final class {class}"));
    for (ty, g) in types.iter().zip(&getters) {
        w.line(format!("private final {ty} {g};"));
    }
    w.blank();
    let params: Vec<String> = types
        .iter()
        .zip(&getters)
        .map(|(ty, g)| format!("{ty} {g}"))
        .collect();
    w.open(format!("private {class}({})", params.join(", ")));
    for g in &getters {
        w.line(format!("this.{g} = {g};"));
    }
    w.close();
    w.blank();
    w.line("/** Builds every client with `options` (default options when `null`), each with its own base URL. The runtime config is checked first. */");
    w.open(format!(
        "public static {class} create({runtime} runtime, {options} options)"
    ));
    w.line(format!("{runtime} config = {runtime}.ensure(runtime);"));
    w.line(format!(
        "{options} given = options != null ? options : {options}.defaults();"
    ));
    let mut args = Vec::new();
    for (c, (ty, g)) in clients.iter().zip(types.iter().zip(&getters)) {
        let url = url_getter(&info.model, &c.path);
        w.line(format!(
            "{ty} {g}Client = new {ty}(given.withBaseUrl(config.{url}()));"
        ));
        args.push(format!("{g}Client"));
    }
    w.line(format!("return new {class}({});", args.join(", ")));
    w.close();
    w.blank();
    w.line("/** Builds every client with default options. */");
    w.open(format!("public static {class} create({runtime} runtime)"));
    w.line(format!("return create(runtime, {options}.defaults());"));
    w.close();
    for (ty, g) in types.iter().zip(&getters) {
        w.blank();
        w.line("/** The client. */");
        w.open(format!("public {ty} {g}()"));
        w.line(format!("return {g};"));
        w.close();
    }
    w.close();
}

/// The getter of the setting at `path` in `<Target>Runtime`.
fn url_getter(model: &RuntimeModel, path: &str) -> String {
    let names = member_names(model);
    model
        .settings()
        .iter()
        .zip(names)
        .find(|(s, _)| s.path == path)
        .map(|(_, n)| n)
        .unwrap_or_default()
}

/// The kebab-case spelling check of the `runtime-prefix` option: lower-case words of letters and
/// digits, joined by `.` or `-`, which is what Spring accepts as a configuration prefix.
#[must_use]
pub fn valid_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix.split(['.', '-']).all(|w| {
            !w.is_empty()
                && w.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
        && prefix
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
}

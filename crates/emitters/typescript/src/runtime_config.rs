//! `runtime.ts`: the typed runtime config of a target (`spec/runtime-config.md` section 3.1).
//!
//! The settings, their order, their defaults and every message come from
//! `veld_codegen::runtime_config`; nothing here types a message or a path. The checking code is
//! the same for every target (`runtime/config.ts.in`) and reads a table of the target's settings.

use std::collections::BTreeMap;

use veld_codegen::runtime_config::{
    ERROR_PREFIX, ROOT_PATH, RuntimeModel, Setting, SettingKind, missing, missing_env,
    not_a_setting, not_a_url, not_an_object, not_in_list,
};
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::ServiceId;

use crate::tree::{FileCtx, Tree};
use crate::util::{add_file, is_ident, prop_key, ts_str};

/// Root-relative module path of the file.
pub const MODULE: &str = "runtime";

/// The checking code, shared by every target.
const CHECKER: &str = include_str!("../runtime/config.ts.in");

/// One client `createClients` builds.
#[derive(Debug, Clone)]
pub struct ClientEntry {
    /// The key of the result (`iam`).
    pub key: String,
    /// The client class (`IamClient`).
    pub class: String,
    /// Root-relative module that exports the class.
    pub module: String,
    /// The service the client calls: the URL is the `services.<service>.url` setting that lists it.
    pub service: ServiceId,
}

/// The settings as nested members, by key.
enum Node<'a> {
    Leaf(&'a Setting),
    Branch(BTreeMap<String, Node<'a>>),
}

fn nest<'a>(settings: &'a [Setting]) -> BTreeMap<String, Node<'a>> {
    let mut root: BTreeMap<String, Node<'a>> = BTreeMap::new();
    for s in settings {
        insert(&mut root, &s.segments, s);
    }
    root
}

fn insert<'a>(map: &mut BTreeMap<String, Node<'a>>, segments: &[String], s: &'a Setting) {
    match segments {
        [] => {}
        [last] => {
            map.insert(last.clone(), Node::Leaf(s));
        }
        [first, rest @ ..] => {
            let entry = map
                .entry(first.clone())
                .or_insert_with(|| Node::Branch(BTreeMap::new()));
            if let Node::Branch(inner) = entry {
                insert(inner, rest, s);
            }
        }
    }
}

fn node_required(node: &Node<'_>) -> bool {
    match node {
        Node::Leaf(s) => s.required,
        Node::Branch(map) => map.values().any(node_required),
    }
}

fn leaf_type(s: &Setting) -> String {
    match s.kind {
        SettingKind::Url => "string".to_string(),
        SettingKind::OneOf(values) => values
            .iter()
            .map(|v| ts_str(v))
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

fn members(map: &BTreeMap<String, Node<'_>>) -> Vec<String> {
    map.iter()
        .map(|(key, node)| {
            let opt = if node_required(node) { "" } else { "?" };
            let ty = match node {
                Node::Leaf(s) => leaf_type(s),
                Node::Branch(inner) => format!("{{ {} }}", members(inner).join("; ")),
            };
            format!("{}{opt}: {ty}", prop_key(key))
        })
        .collect()
}

/// A TypeScript expression for the message text `f` builds from a path and a name: the pieces
/// of the text joined with the two parameters, so that the text itself is the module's.
fn template(f: impl Fn(&str, &str) -> String) -> String {
    let text = f("\u{1}", "\u{2}");
    let mut parts: Vec<String> = Vec::new();
    let mut literal = String::new();
    for c in text.chars() {
        let param = match c {
            '\u{1}' => "path",
            '\u{2}' => "name",
            c => {
                literal.push(c);
                continue;
            }
        };
        if !literal.is_empty() {
            parts.push(ts_str(&std::mem::take(&mut literal)));
        }
        parts.push(param.to_string());
    }
    if !literal.is_empty() {
        parts.push(ts_str(&literal));
    }
    parts.join(" + ")
}

fn write_types(w: &mut CodeWriter, model: &RuntimeModel, tree_map: &BTreeMap<String, Node<'_>>) {
    let name = model.type_name();
    w.line(format!(
        "/** The settings of the target `{}`: what differs per deployment (`services.<service>.url`, `rules.output`). */",
        model.target().replace("*/", "* /")
    ));
    w.open(format!("export interface {name}"));
    for m in members(tree_map) {
        w.line(format!("{m};"));
    }
    w.close();
    w.blank();
    w.line("/** `fromEnv`: the name of the environment variable that holds each path. A name is required for every required path. */");
    w.open(format!("export interface {name}Env"));
    for s in model.settings() {
        let opt = if s.required { "" } else { "?" };
        w.line(format!("{}{opt}: string;", ts_str(&s.path)));
    }
    w.close();
    w.blank();
}

fn write_table(w: &mut CodeWriter, model: &RuntimeModel) {
    w.line(format!("const PREFIX = {};", ts_str(ERROR_PREFIX)));
    w.line(format!("const ROOT = {};", ts_str(ROOT_PATH)));
    let not_setting = template(|p, _| not_a_setting(p));
    let not_object = template(|p, _| not_an_object(p));
    let env = template(missing_env);
    w.line(format!(
        "const notASetting = (path: string): string => {not_setting};"
    ));
    w.line(format!(
        "const notAnObject = (path: string): string => {not_object};"
    ));
    w.line(format!(
        "const missingEnv = (path: string, name: string): string => {env};"
    ));
    w.blank();
    w.open_with("const SETTINGS: readonly Setting[] =", " [");
    for s in model.settings() {
        w.line(format!("{},", setting_expr(s)));
    }
    w.close_with("];");
    w.blank();
}

fn setting_expr(s: &Setting) -> String {
    let segments: Vec<String> = s.segments.iter().map(|k| ts_str(k)).collect();
    let (kind, invalid, values) = match s.kind {
        SettingKind::Url => ("url", not_a_url(&s.path), None),
        SettingKind::OneOf(list) => {
            let quoted: Vec<String> = list.iter().map(|v| ts_str(v)).collect();
            ("list", not_in_list(&s.path, list), Some(quoted.join(", ")))
        }
    };
    let mut parts = vec![
        format!("path: {}", ts_str(&s.path)),
        format!("segments: [{}]", segments.join(", ")),
        format!("kind: '{kind}'"),
        format!("required: {}", s.required),
    ];
    if let Some(v) = values {
        parts.push(format!("values: [{v}]"));
    }
    if let Some(d) = &s.default {
        parts.push(format!("default: {}", ts_str(d)));
    }
    parts.push(format!("missing: {}", ts_str(&missing(&s.path))));
    parts.push(format!("invalid: {}", ts_str(&invalid)));
    format!("{{ {} }}", parts.join(", "))
}

fn write_api(w: &mut CodeWriter, model: &RuntimeModel) {
    let name = model.type_name();
    w.line("/** Checks a runtime config and returns it with the defaults filled in. Fails once, with every problem. */");
    w.open(format!(
        "export function checkRuntime(value: unknown): {name}"
    ));
    w.line(format!("return nest(checked(value)) as {name};"));
    w.close();
    w.blank();
    w.line("/** Builds the runtime config from environment variables, whose names the application chooses. `env` defaults to `process.env`. */");
    w.open(format!(
        "export function fromEnv(names: {name}Env, env: Record<string, string | undefined> = processEnv()): {name}"
    ));
    w.line(format!("return nest(fromNames(names, env)) as {name};"));
    w.close();
}

/// The URL of a checked config for `service`, from the setting that lists it (`None`: the
/// model has no URL for the service).
fn url_access(model: &RuntimeModel, service: ServiceId) -> Option<String> {
    let setting = model
        .urls()
        .find(|s| s.service.as_ref().is_some_and(|r| r.ids.contains(&service)))?;
    let mut out = "config".to_string();
    for key in &setting.segments {
        if is_ident(key) {
            out.push('.');
            out.push_str(key);
        } else {
            out.push_str(&format!("[{}]", ts_str(key)));
        }
    }
    Some(out)
}

fn write_clients(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    model: &RuntimeModel,
    list: &[ClientEntry],
) {
    let name = model.type_name();
    let opts = fc.rt("ClientOptions");
    for e in list {
        fc.import(&e.module, &e.class, true);
    }
    w.blank();
    w.line("/** The clients of every service of the target, by service name. */");
    w.open("export interface Clients");
    for e in list {
        w.line(format!("{}: {};", prop_key(&e.key), e.class));
    }
    w.close();
    w.blank();
    w.line("/** One client for every service, each with its URL from `services` and the same `options`. */");
    w.open(format!(
        "export function createClients(runtime: {name}, options: Omit<{opts}, 'baseUrl'> = {{}}): Clients"
    ));
    w.line("const config = checkRuntime(runtime);");
    w.open_with("return", " {");
    for e in list {
        let Some(url) = url_access(model, e.service) else {
            continue;
        };
        w.line(format!(
            "{}: new {}({{ ...options, baseUrl: {url} }}),",
            prop_key(&e.key),
            e.class
        ));
    }
    w.close_with("};");
    w.close();
}

/// Writes `runtime.ts` and returns its module path. `clients` are the clients `createClients`
/// builds (none: the function is not written).
///
/// # Errors
/// A path is already taken.
pub fn emit(
    files: &mut FileSet,
    tree: &Tree<'_>,
    model: &RuntimeModel,
    clients: &[ClientEntry],
) -> Result<String, Vec<GenDiagnostic>> {
    let mut fc = FileCtx::new(tree, MODULE);
    let mut w = CodeWriter::new();
    let map = nest(model.settings());
    write_types(&mut w, model, &map);
    write_table(&mut w, model);
    let mut body = w.finish();
    if !body.ends_with("\n\n") {
        body.push('\n');
    }
    body.push_str(CHECKER);
    body.push('\n');
    let mut api = CodeWriter::new();
    write_api(&mut api, model);
    if !clients.is_empty() {
        write_clients(&mut api, &mut fc, model, clients);
    }
    body.push_str(&api.finish());
    add_file(files, &format!("{MODULE}.ts"), fc.finish(&body))?;
    Ok(MODULE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_template_keeps_the_text_of_the_module() {
        assert_eq!(
            template(|p, _| not_a_setting(p)),
            "path + ' is not a setting'"
        );
        assert_eq!(
            template(missing_env),
            "path + ' is missing (environment variable ' + name + ' is not set)'"
        );
    }

    #[test]
    fn the_nested_type_is_sorted_by_key_and_optional_without_a_required_path() {
        let model = RuntimeModel::from_settings(
            "bookings",
            vec![
                Setting::service_url("payments", Vec::new()),
                Setting::service_url("iam", Vec::new()),
                Setting::rules_output(),
            ],
        );
        let map = nest(model.settings());
        assert_eq!(
            members(&map),
            [
                "rules?: { output?: 'strict' | 'audit' | 'off' }",
                "services: { iam: { url: string }; payments: { url: string } }",
            ]
        );
    }

    #[test]
    fn the_checker_reads_the_table_and_the_messages_the_module_gives() {
        assert!(CHECKER.contains("SETTINGS") && CHECKER.contains("notASetting"));
        assert!(!CHECKER.contains("is not a setting"));
        assert!(!CHECKER.contains("must be an http"));
    }
}

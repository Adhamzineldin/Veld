//! Translates a v1 `veld.config.json` into a v2 `veld.toml`. Pure: no file access.

use std::collections::BTreeSet;

use serde_json::Value;

/// Where one package's v1 contracts live and where the v2 files go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePlan {
    /// Source directory, relative to the v1 config directory (`.` for the directory itself).
    pub source: String,
    /// Destination directory, relative to the v1 config directory.
    pub dest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPlan {
    pub toml: String,
    pub packages: Vec<PackagePlan>,
    pub warnings: Vec<String>,
}

/// One unit of the v1 config: the whole project, or one `workspace` entry.
struct Entry {
    name: String,
    input: String,
    backend: Option<(String, Option<String>)>,
    frontend: Option<String>,
    backend_out: Option<String>,
    frontend_out: Option<String>,
    base_url: Option<String>,
    consumes: Vec<String>,
}

pub fn plan(v1: &Value, project_name: &str, contracts: &str) -> ConfigPlan {
    let mut w = Vec::new();
    warn_ignored(v1, &mut w);
    let entries = match v1.get("workspace").and_then(Value::as_array) {
        Some(list) if !list.is_empty() => list.iter().map(|e| entry(e, &mut w)).collect(),
        _ => vec![single(v1, project_name, &mut w)],
    };
    let workspace = v1
        .get("workspace")
        .is_some_and(|x| x.as_array().is_some_and(|a| !a.is_empty()));
    build(&entries, workspace, project_name, contracts, w)
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && *s != "none")
}

fn nested_str<'a>(v: &'a Value, section: &str, key: &str) -> Option<&'a str> {
    v.get(section).and_then(|s| str_of(s, key))
}

fn warn_ignored(v1: &Value, w: &mut Vec<String>) {
    for (key, why) in [
        (
            "tools",
            "auxiliary generators are replaced by `veld export openapi`",
        ),
        (
            "hooks",
            "post-generate hooks have no v2 equivalent; run the command from your build",
        ),
        (
            "postGenerate",
            "post-generate hooks have no v2 equivalent; run the command from your build",
        ),
        (
            "aliases",
            "imports are rewritten as `use` lines, so aliases are not needed",
        ),
        (
            "registry",
            "registry settings are not migrated; see `[registry]` and `[dependencies]` in spec/config.md",
        ),
        (
            "dependencies",
            "registry dependencies are not migrated; see `[dependencies]` in spec/config.md",
        ),
        ("plugins", "v1 plugins are not migrated"),
    ] {
        if v1.get(key).is_some() {
            w.push(format!("v1 config key `{key}` was not migrated: {why}"));
        }
    }
}

fn single(v1: &Value, project_name: &str, w: &mut Vec<String>) -> Entry {
    let backend = nested_str(v1, "backendConfig", "target").or_else(|| str_of(v1, "backend"));
    let frontend = nested_str(v1, "frontendConfig", "target").or_else(|| str_of(v1, "frontend"));
    let (backend, frontend) = match (backend, frontend) {
        (None, None) => (Some("node"), Some("typescript")),
        other => other,
    };
    let framework =
        nested_str(v1, "backendConfig", "framework").or_else(|| str_of(v1, "backendFramework"));
    let shared = str_of(v1, "out").unwrap_or("generated");
    let backend_out = nested_str(v1, "backendConfig", "out").or_else(|| str_of(v1, "backendOut"));
    let frontend_out =
        nested_str(v1, "frontendConfig", "out").or_else(|| str_of(v1, "frontendOut"));
    let (b_out, f_out) = shared_outputs(
        backend_out,
        frontend_out,
        shared,
        backend.is_some(),
        frontend.is_some(),
        w,
    );
    Entry {
        name: sanitize(project_name, w),
        input: String::new(),
        backend: backend.map(|b| (b.to_string(), framework.map(str::to_string))),
        frontend: frontend.map(str::to_string),
        backend_out: b_out,
        frontend_out: f_out,
        base_url: str_of(v1, "baseUrl").map(str::to_string),
        consumes: Vec::new(),
    }
}

/// v1 let both sides share `out`; v2 forbids two targets in one directory.
fn shared_outputs(
    backend: Option<&str>,
    frontend: Option<&str>,
    shared: &str,
    has_backend: bool,
    has_frontend: bool,
    w: &mut Vec<String>,
) -> (Option<String>, Option<String>) {
    let b = backend.unwrap_or(shared).to_string();
    let f = frontend.unwrap_or(shared).to_string();
    if has_backend && has_frontend && b == f {
        w.push(format!("backend and frontend both wrote to `{b}` in v1; v2 needs separate directories, so `{b}/server` and `{b}/client` are used"));
        return (Some(format!("{b}/server")), Some(format!("{f}/client")));
    }
    (Some(b), Some(f))
}

fn entry(e: &Value, w: &mut Vec<String>) -> Entry {
    let name = sanitize(str_of(e, "name").unwrap_or("service"), w);
    let backend = nested_str(e, "backendConfig", "target").or_else(|| str_of(e, "backend"));
    let framework =
        nested_str(e, "backendConfig", "framework").or_else(|| str_of(e, "backendFramework"));
    let frontend = nested_str(e, "frontendConfig", "target").or_else(|| str_of(e, "frontend"));
    let out = |section: &str, flat: &str| {
        nested_str(e, section, "out")
            .or_else(|| str_of(e, flat))
            .or_else(|| str_of(e, "out"))
            .map(str::to_string)
    };
    let consumes = e
        .get("consumes")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Entry {
        name,
        input: str_of(e, "input").unwrap_or_default().to_string(),
        backend: backend.map(|b| (b.to_string(), framework.map(str::to_string))),
        frontend: frontend.map(str::to_string),
        backend_out: out("backendConfig", "backendOut"),
        frontend_out: out("frontendConfig", "frontendOut"),
        base_url: str_of(e, "baseUrl").map(str::to_string),
        consumes,
    }
}

fn sanitize(name: &str, w: &mut Vec<String>) -> String {
    let mut out: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'p');
    }
    if out != name {
        w.push(format!(
            "name `{name}` was renamed to `{out}` (letters, digits and `_` only)"
        ));
    }
    out
}

/// The directory that holds a service's contracts: the input file's directory, one level up
/// when it is a conventional `modules` or `models` folder.
fn source_dir(input: &str) -> String {
    let norm = input.replace('\\', "/");
    let mut parts: Vec<&str> = norm
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    parts.pop();
    if parts
        .last()
        .is_some_and(|l| matches!(*l, "modules" | "models" | "module"))
    {
        parts.pop();
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn build(
    entries: &[Entry],
    workspace: bool,
    project: &str,
    contracts: &str,
    mut w: Vec<String>,
) -> ConfigPlan {
    let mut t = String::new();
    let mut packages = Vec::new();
    let services: Vec<&Entry> = if workspace {
        entries.iter().filter(|e| !e.input.is_empty()).collect()
    } else {
        entries.iter().collect()
    };
    let names: Vec<String> = services.iter().map(|e| e.name.clone()).collect();
    t.push_str(&format!(
        "[project]\nname = {}\n",
        q(&sanitize(project, &mut Vec::new()))
    ));
    if workspace
        || services
            .first()
            .is_some_and(|e| e.base_url.as_deref().is_some_and(is_url))
    {
        t.push('\n');
        for e in &services {
            let dest = if workspace {
                format!("{contracts}/{}", e.name)
            } else {
                contracts.to_string()
            };
            packages.push(PackagePlan {
                source: if workspace {
                    source_dir(&e.input)
                } else {
                    ".".into()
                },
                dest: dest.clone(),
            });
            service_section(&mut t, e, &dest, &names, &mut w);
        }
    } else {
        t.push_str(&format!("contracts = {}\n\n", q(contracts)));
        packages.push(PackagePlan {
            source: ".".into(),
            dest: contracts.to_string(),
        });
        if let Some(url) = services.first().and_then(|e| e.base_url.as_deref()) {
            w.push(format!(
                "baseUrl `{url}` is a path prefix, not a service URL; it was not migrated"
            ));
        }
    }
    for e in entries {
        targets(
            &mut t,
            e,
            workspace && !e.input.is_empty() || !workspace,
            &names,
            &mut w,
        );
    }
    ConfigPlan {
        toml: t,
        packages,
        warnings: w,
    }
}

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

fn service_section(t: &mut String, e: &Entry, dest: &str, names: &[String], w: &mut Vec<String>) {
    t.push_str(&format!("[services.{}]\ncontracts = {}\n", e.name, q(dest)));
    match e.base_url.as_deref() {
        Some(u) if is_url(u) => t.push_str(&format!("base-url = {}\n", q(u))),
        Some(u) => w.push(format!(
            "service `{}`: baseUrl `{u}` is not a URL; it was not migrated",
            e.name
        )),
        None => {}
    }
    let consumes = expand(&e.consumes, names, &e.name, w);
    if !consumes.is_empty() {
        t.push_str(&format!(
            "consumes = [{}]\n",
            consumes.iter().map(|c| q(c)).collect::<Vec<_>>().join(", ")
        ));
    }
    t.push('\n');
}

/// `*` means every other service; unknown names are dropped with a warning.
fn expand(list: &[String], names: &[String], owner: &str, w: &mut Vec<String>) -> Vec<String> {
    let mut out = BTreeSet::new();
    for c in list {
        if c == "*" {
            out.extend(names.iter().filter(|n| *n != owner).cloned());
        } else if names.contains(c) {
            out.insert(c.clone());
        } else {
            w.push(format!("`{owner}` consumes unknown service `{c}`; dropped"));
        }
    }
    out.into_iter().collect()
}

fn targets(t: &mut String, e: &Entry, is_service: bool, names: &[String], w: &mut Vec<String>) {
    if let Some((target, framework)) = &e.backend {
        let out = e.backend_out.clone().unwrap_or_else(|| "generated".into());
        let mut lines = vec![
            "[[targets]]".to_string(),
            format!("name = {}", q(&format!("{}-api", e.name))),
            "role = \"server\"".to_string(),
        ];
        let known = matches!(target.as_str(), "node" | "node-ts" | "node-js" | "java");
        let emitter = if target == "java" { "java" } else { "node" };
        lines.push(format!("emitter = {}", q(emitter)));
        if known && is_service {
            lines.push(format!("service = {}", q(&e.name)));
        }
        framework_line(&mut lines, emitter, framework.as_deref(), w);
        lines.push(format!("out = {}", q(&out)));
        emit_target(
            t,
            &lines,
            known && is_service,
            &format!("backend target `{target}` of `{}`", e.name),
            w,
        );
    }
    if let Some(target) = &e.frontend {
        client_target(t, e, target, names, w);
    }
}

fn framework_line(
    lines: &mut Vec<String>,
    emitter: &str,
    framework: Option<&str>,
    w: &mut Vec<String>,
) {
    match framework {
        Some(f) if emitter == "node" && matches!(f, "express" | "fastify" | "hono" | "none") => {
            lines.push(format!("framework = {}", q(f)));
        }
        Some(f) if emitter == "java" => lines.push(format!("framework = {}", q(f))),
        Some(f) => w.push(format!(
            "backend framework `{f}` has no v2 `{emitter}` equivalent; omitted"
        )),
        None => {}
    }
}

fn client_target(t: &mut String, e: &Entry, target: &str, names: &[String], w: &mut Vec<String>) {
    let known = matches!(target, "typescript" | "react" | "ts");
    if target == "react" {
        w.push(format!("frontend target `react` of `{}`: v2 generates the plain `typescript` client; React hooks are not available yet", e.name));
    }
    let out = e.frontend_out.clone().unwrap_or_else(|| "generated".into());
    let mut lines = vec![
        "[[targets]]".to_string(),
        format!("name = {}", q(&format!("{}-client", e.name))),
        "role = \"client\"".to_string(),
        "emitter = \"typescript\"".to_string(),
    ];
    let services = expand(&e.consumes, names, &e.name, w);
    if !services.is_empty() && e.input.is_empty() && !e.consumes.iter().any(|c| c == "*") {
        lines.push(format!(
            "services = [{}]",
            services.iter().map(|s| q(s)).collect::<Vec<_>>().join(", ")
        ));
    }
    lines.push(format!("out = {}", q(&out)));
    emit_target(
        t,
        &lines,
        known,
        &format!("frontend target `{target}` of `{}`", e.name),
        w,
    );
}

/// Writes a target, or comments it out with a warning when v2 has no matching emitter.
fn emit_target(t: &mut String, lines: &[String], supported: bool, what: &str, w: &mut Vec<String>) {
    if supported {
        for l in lines {
            t.push_str(l);
            t.push('\n');
        }
    } else {
        w.push(format!(
            "{what} has no v2 emitter; written as a commented-out target"
        ));
        t.push_str(&format!(
            "# TODO(veld migrate): {what} has no v2 emitter.\n"
        ));
        for l in lines {
            t.push_str(&format!("# {l}\n"));
        }
    }
    t.push('\n');
}

fn q(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use veld_project::config::load_str;

    fn load(text: &str) -> veld_project::config::Project {
        load_str(std::path::Path::new("/p"), text).unwrap_or_else(|e| panic!("{e}\n{text}"))
    }

    #[test]
    fn flat_node_react_config() {
        let v1 = json!({"input":"app.veld","backend":"node","frontend":"react","out":"../generated",
            "backendOut":"../backend/src/generated","frontendOut":"../frontend/src/generated"});
        let p = plan(&v1, "todo", "contracts");
        let project = load(&p.toml);
        assert_eq!(project.targets.len(), 2);
        assert_eq!(
            p.packages,
            [PackagePlan {
                source: ".".into(),
                dest: "contracts".into()
            }]
        );
        assert!(p.warnings.iter().any(|w| w.contains("react")));
    }

    #[test]
    fn workspace_translates_services_consumes_and_unknown_targets() {
        let v1 = json!({"workspace":[
            {"name":"iam","input":"services/iam/modules/iam.veld","backendConfig":{"target":"node-ts","framework":"express"},
             "out":"../iam","baseUrl":"http://iam:1"},
            {"name":"tx","input":"services/tx/modules/tx.veld","backendConfig":{"target":"python"},"out":"../tx","consumes":["iam"]},
            {"name":"frontend","input":"","frontendConfig":{"target":"react"},"out":"../web","consumes":["*"]}]});
        let p = plan(&v1, "bank", "contracts");
        let project = load(&p.toml);
        assert_eq!(project.services().count(), 2);
        assert_eq!(project.package("tx").unwrap().consumes, ["iam"]);
        assert_eq!(
            project.package("iam").unwrap().base_url.as_deref(),
            Some("http://iam:1")
        );
        let names: Vec<&str> = project.targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["iam-api", "frontend-client"]);
        assert!(p.toml.contains("# [[targets]]"), "{}", p.toml);
        assert!(p.warnings.iter().any(|w| w.contains("python")));
        assert_eq!(p.packages[0].source, "services/iam");
        assert_eq!(p.packages[1].dest, "contracts/tx");
    }

    #[test]
    fn shared_out_is_split_into_two_directories() {
        let p = plan(
            &json!({"backend":"node","frontend":"typescript","out":"gen"}),
            "x",
            "contracts",
        );
        let project = load(&p.toml);
        let outs: Vec<String> = project
            .targets
            .iter()
            .map(|t| t.out.display().to_string().replace('\\', "/"))
            .collect();
        assert!(
            outs[0].ends_with("gen/server") && outs[1].ends_with("gen/client"),
            "{outs:?}"
        );
    }

    #[test]
    fn names_are_sanitized_and_ignored_keys_warned() {
        let p = plan(
            &json!({"tools":{"openapi":true},"hooks":{}}),
            "My-App",
            "contracts",
        );
        assert!(p.toml.contains("name = \"my_app\""), "{}", p.toml);
        assert!(
            p.warnings.iter().any(|w| w.contains("tools"))
                && p.warnings.iter().any(|w| w.contains("hooks"))
        );
    }

    #[test]
    fn source_dir_strips_conventional_folders() {
        assert_eq!(source_dir("services/iam/modules/iam.veld"), "services/iam");
        assert_eq!(source_dir("app.veld"), ".");
        assert_eq!(source_dir("./svc/app.veld"), "svc");
    }
}

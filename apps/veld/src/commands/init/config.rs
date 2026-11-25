//! The `veld.toml` files `veld init` writes (`spec/workspace.md`): one file for a single
//! service, a root and one file per member for a workspace.

use super::answers::{Answers, Client, Server, sanitize};
use super::render::File;

/// One target as the answers describe it, with paths relative to the project directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub name: String,
    /// `server` or `client`.
    pub role: &'static str,
    pub emitter: &'static str,
    pub framework: Option<String>,
    /// The member directory that holds the target's `veld.toml` (empty: the project directory).
    pub dir: String,
    /// The output folder, relative to the project directory, overrides applied.
    pub out: String,
    pub options: Vec<(&'static str, String)>,
    /// The service a server serves.
    pub service: Option<String>,
}

impl Planned {
    /// A TypeScript target (it needs a `package.json` beside its output).
    pub fn is_npm(&self) -> bool {
        matches!(self.emitter, "node" | "typescript")
    }
}

/// Every target of the answers, servers first, in the order they are written.
pub fn planned(a: &Answers) -> Vec<Planned> {
    let mut out: Vec<Planned> = a
        .servers
        .iter()
        .filter_map(|(svc, s)| server(a, svc, s))
        .collect();
    out.extend(a.clients.iter().filter_map(|c| client(a, c)));
    out
}

fn server(a: &Answers, svc: &str, s: &Server) -> Option<Planned> {
    let ws = a.is_workspace();
    let name = if ws {
        format!("{svc}-api")
    } else {
        "api".to_string()
    };
    let flat = format!("services/{svc}");
    let lower = sanitize(svc).to_ascii_lowercase();
    let (emitter, fw, default_out, options) = match s {
        Server::None => return None,
        Server::Node(fw) => (
            "node",
            fw,
            if ws {
                format!("{flat}/generated")
            } else {
                "generated/server".to_string()
            },
            Vec::new(),
        ),
        Server::Rust(fw) => (
            "rust",
            fw,
            if ws {
                format!("{flat}/generated")
            } else {
                "generated/rust-server".to_string()
            },
            vec![("crate-name", format!("{lower}-server"))],
        ),
        Server::Java(fw) => (
            "java",
            fw,
            if ws {
                format!("{flat}/src/main/java")
            } else {
                "java/src/main/java".to_string()
            },
            vec![("package", format!("com.example.{lower}"))],
        ),
    };
    Some(Planned {
        out: a.out(&name, &default_out),
        name,
        role: "server",
        emitter,
        framework: Some(fw.clone()),
        dir: if ws { flat } else { String::new() },
        options,
        service: Some(svc.to_string()),
    })
}

fn client(a: &Answers, client: &Client) -> Option<Planned> {
    let ws = a.is_workspace();
    let lower = sanitize(&a.name).to_ascii_lowercase();
    let (name, emitter, framework, dir, default_out, options) = match client {
        Client::None => return None,
        Client::Ts(fw) => (
            "web",
            "typescript",
            fw.clone(),
            "web",
            if ws { "web/generated" } else { "generated/client" },
            Vec::new(),
        ),
        Client::Rust(fw) => (
            "rust-client",
            "rust",
            Some(fw.clone()),
            "clients/rust",
            if ws { "clients/rust/generated" } else { "generated/rust-client" },
            vec![("crate-name", format!("{lower}-client"))],
        ),
        Client::Java => (
            "java-client",
            "java",
            None,
            "clients/java",
            if ws {
                "clients/java/src/main/java"
            } else {
                "java-client/src/main/java"
            },
            vec![("package", format!("com.example.{lower}.client"))],
        ),
    };
    Some(Planned {
        out: a.out(name, default_out),
        name: name.to_string(),
        role: "client",
        emitter,
        framework,
        dir: if ws { dir.to_string() } else { String::new() },
        options,
        service: None,
    })
}

/// A TOML basic string.
pub fn toml_string(text: &str) -> String {
    let mut s = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            c if c.is_control() => s.push(' '),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// `out` (relative to the project directory) as the member in `dir` writes it.
pub fn rel_out(dir: &str, out: &str) -> String {
    let from: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    let to: Vec<&str> = out
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend(&to[common..]);
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// The text of one target table. `array` writes `[[role]]`, else `[role]`.
fn target_text(t: &Planned, array: bool) -> String {
    let head = if array {
        format!("[[{}]]", t.role)
    } else {
        format!("[{}]", t.role)
    };
    let mut s = format!("{head}\nname = {}\nemitter = \"{}\"\n", toml_string(&t.name), t.emitter);
    if let Some(fw) = &t.framework {
        s.push_str(&format!("framework = \"{fw}\"\n"));
    }
    s.push_str(&format!("out = {}\n", toml_string(&rel_out(&t.dir, &t.out))));
    if !t.options.is_empty() {
        s.push_str(&format!("\n[{}.options]\n", t.role));
        for (k, v) in &t.options {
            s.push_str(&format!("{k} = {}\n", toml_string(v)));
        }
    }
    s
}

/// The target tables of one member: servers, then clients (an array when there are several).
fn targets_text(targets: &[&Planned]) -> String {
    let mut parts = Vec::new();
    for role in ["server", "client"] {
        let of_role: Vec<&&Planned> = targets.iter().filter(|t| t.role == role).collect();
        let array = of_role.len() > 1;
        parts.extend(of_role.iter().map(|t| target_text(t, array)));
    }
    parts.join("\n")
}

fn file(path: String, text: String) -> File {
    File {
        path,
        text,
        optional: false,
    }
}

/// Every `veld.toml` of the answers; the root (or only) one is first.
pub fn config_files(a: &Answers) -> Vec<File> {
    let targets = planned(a);
    if !a.is_workspace() {
        return vec![single(a, &targets)];
    }
    let mut files = vec![file("veld.toml".to_string(), root(a, &targets))];
    for svc in &a.services {
        files.push(service_file(a, svc, &targets));
    }
    for dir in client_dirs(&targets) {
        files.push(app_file(&dir, &targets));
    }
    files
}

/// The single file form: the service and its targets in one `veld.toml`.
fn single(a: &Answers, targets: &[Planned]) -> File {
    let mut t = format!("[service]\nname = \"{}\"\n", a.name);
    if !a.description.is_empty() {
        t.push_str(&format!("description = {}\n", toml_string(&a.description)));
    }
    let refs: Vec<&Planned> = targets.iter().collect();
    if !refs.is_empty() {
        t.push('\n');
        t.push_str(&targets_text(&refs));
    }
    file("veld.toml".to_string(), t)
}

fn client_dirs(targets: &[Planned]) -> Vec<String> {
    let mut dirs: Vec<String> = targets
        .iter()
        .filter(|t| t.role == "client")
        .map(|t| t.dir.clone())
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

fn root(a: &Answers, targets: &[Planned]) -> String {
    let mut members = vec!["\"services/*\"".to_string()];
    members.extend(client_dirs(targets).iter().map(|d| toml_string(d)));
    let mut t = format!("[workspace]\nname = \"{}\"\n", a.name);
    if !a.description.is_empty() {
        t.push_str(&format!("description = {}\n", toml_string(&a.description)));
    }
    t.push_str(&format!("members = [{}]\n", members.join(", ")));
    t
}

fn service_file(a: &Answers, svc: &str, targets: &[Planned]) -> File {
    let mut t = format!(
        "[service]\nbase-url = {}\n",
        toml_string(&a.base_url(svc))
    );
    let consumed = a.consumes_of(svc);
    if !consumed.is_empty() {
        let list: Vec<String> = consumed.iter().map(|c| toml_string(c)).collect();
        t.push_str(&format!(
            "consumes = [{}]   # {svc} gets a generated client for each\n",
            list.join(", ")
        ));
    }
    let mine: Vec<&Planned> = targets
        .iter()
        .filter(|x| x.service.as_deref() == Some(svc))
        .collect();
    if !mine.is_empty() {
        t.push('\n');
        t.push_str(&targets_text(&mine));
    }
    file(format!("services/{svc}/veld.toml"), t)
}

fn app_file(dir: &str, targets: &[Planned]) -> File {
    let mine: Vec<&Planned> = targets.iter().filter(|t| t.dir == dir).collect();
    let mut t = String::from("[app]\n");
    t.push('\n');
    t.push_str(&targets_text(&mine));
    file(format!("{dir}/veld.toml"), t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_is_written_relative_to_the_member() {
        assert_eq!(rel_out("", "generated/server"), "generated/server");
        assert_eq!(rel_out("services/users", "services/users/generated"), "generated");
        assert_eq!(rel_out("web", "web/generated"), "generated");
        assert_eq!(rel_out("web", "ui/gen"), "../ui/gen");
        assert_eq!(rel_out("services/users", "web/generated"), "../../web/generated");
        assert_eq!(rel_out("web", "web"), ".");
    }

    #[test]
    fn toml_strings_are_escaped() {
        assert_eq!(toml_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}

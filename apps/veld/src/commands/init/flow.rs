//! The interactive questions of `veld init`, asked through a [`Prompter`].

use anyhow::Result;

use super::answers::{
    Answers, Client, JAVA_FRAMEWORKS, NODE_FRAMEWORKS, Server, TS_FRAMEWORKS, parse_services,
    sanitize,
};
use super::render;
use crate::error::user;
use crate::ui::prompt::{Choice, Prompter, Text, cancelled};
use crate::ui::render as ui_render;
use crate::ui::theme::Theme;

fn valid_name(text: &str) -> std::result::Result<(), String> {
    let s = sanitize(text);
    if s.is_empty() {
        Err("use letters, digits or `_`".to_string())
    } else if s.len() > 64 {
        Err("at most 64 characters".to_string())
    } else {
        Ok(())
    }
}

fn valid_services(text: &str) -> std::result::Result<(), String> {
    parse_services(text).map(|_| ()).map_err(|e| e.to_string())
}

/// A relative folder inside the project.
pub fn parse_out(text: &str) -> Result<String> {
    let t = text.trim().trim_end_matches('/');
    let bad =
        t.is_empty() || t.starts_with('/') || t.contains(':') || t.split('/').any(|p| p == "..");
    if bad {
        return Err(user(
            "use a relative folder inside the project, for example `server/src/generated`",
        ));
    }
    Ok(t.to_string())
}

fn valid_out(text: &str) -> std::result::Result<(), String> {
    parse_out(text).map(|_| ()).map_err(|e| e.to_string())
}

fn any_text(_: &str) -> std::result::Result<(), String> {
    Ok(())
}

fn frameworks(list: &[&str], hint: fn(&str) -> &'static str) -> Vec<Choice> {
    list.iter().map(|f| Choice::new(f, hint(f))).collect()
}

fn node_hint(f: &str) -> &'static str {
    match f {
        "express" => "the classic; widest middleware ecosystem",
        "fastify" => "fast, schema friendly",
        "hono" => "tiny, runs anywhere",
        "nest" => "NestJS controllers and modules",
        _ => "no framework: node:http only",
    }
}

fn java_hint(f: &str) -> &'static str {
    match f {
        "jdk" => "no framework: the JDK's built-in HttpServer",
        _ => "Spring Boot controllers",
    }
}

fn ts_hint(f: &str) -> &'static str {
    match f {
        "react" => "hooks on top of the client",
        _ => "composables on top of the client",
    }
}

/// Language first, then its framework.
fn ask_server(p: &mut dyn Prompter, label: &str) -> Result<Server> {
    let langs = [
        Choice::new("Node", "TypeScript server"),
        Choice::new("Java", "Java 17+ server"),
        Choice::new("None", "contracts and clients only"),
    ];
    match p.select(label, &langs, 0)? {
        0 => {
            let fws = frameworks(&NODE_FRAMEWORKS, node_hint);
            let i = p.select("Node framework", &fws, 0)?;
            Ok(Server::Node(NODE_FRAMEWORKS[i].to_string()))
        }
        1 => {
            let fws = frameworks(&JAVA_FRAMEWORKS, java_hint);
            let i = p.select("Java framework", &fws, 0)?;
            Ok(Server::Java(JAVA_FRAMEWORKS[i].to_string()))
        }
        _ => Ok(Server::None),
    }
}

fn ask_clients(p: &mut dyn Prompter) -> Result<Vec<Client>> {
    let items = [
        Choice::new("TypeScript", "typed fetch client"),
        Choice::new("Java", "typed client for JVM apps"),
    ];
    let picked = p.multiselect("Client stacks (space selects)", &items, &[0])?;
    let mut clients = Vec::new();
    for i in picked {
        if i == 0 {
            let mut fws = vec![Choice::new("None", "plain client, no UI framework")];
            fws.extend(frameworks(&TS_FRAMEWORKS, ts_hint));
            let k = p.select("TypeScript UI framework", &fws, 0)?;
            clients.push(Client::Ts(
                k.checked_sub(1).map(|j| TS_FRAMEWORKS[j].to_string()),
            ));
        } else {
            clients.push(Client::Java);
        }
    }
    Ok(clients)
}

fn ask_servers(p: &mut dyn Prompter, names: &[String]) -> Result<Vec<(String, Server)>> {
    if names.len() > 1 {
        let same = p.confirm(
            &format!(
                "Use the same server stack for all {} services?",
                names.len()
            ),
            true,
        )?;
        if same {
            let s = ask_server(p, "Server stack for all services")?;
            return Ok(names.iter().map(|n| (n.clone(), s.clone())).collect());
        }
    }
    let mut out = Vec::new();
    for svc in names {
        let label = if names.len() == 1 {
            "Server stack".to_string()
        } else {
            format!("Server stack for `{svc}`")
        };
        out.push((svc.clone(), ask_server(p, &label)?));
    }
    Ok(out)
}

/// `(target name, default out)` of the config the answers render.
pub fn default_outs(a: &Answers) -> Vec<(String, String)> {
    let text = render::files(a)
        .into_iter()
        .next()
        .map(|f| f.text)
        .unwrap_or_default();
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(toml::Value::Array(targets)) = table.get("targets") else {
        return Vec::new();
    };
    targets
        .iter()
        .filter_map(|t| {
            let name = t.get("name")?.as_str()?.to_string();
            let out = t.get("out")?.as_str()?.to_string();
            Some((name, out))
        })
        .collect()
}

fn ask_outs(p: &mut dyn Prompter, answers: &mut Answers) -> Result<()> {
    let outs = default_outs(answers);
    if outs.is_empty() {
        return Ok(());
    }
    let w = outs.iter().map(|(t, _)| t.len()).max().unwrap_or(0);
    let body: Vec<String> = outs.iter().map(|(t, o)| format!("{t:<w$}  {o}")).collect();
    p.note("Output folders (defaults)", &body)?;
    if !p.confirm("Change any output folder?", false)? {
        return Ok(());
    }
    for (target, default) in outs {
        let label = format!("Output folder of `{target}`");
        let out = p.text(&Text {
            label: &label,
            default: &default,
            placeholder: "",
            required: true,
            validate: valid_out,
        })?;
        let out = parse_out(&out)?;
        if out != default {
            answers.outs.push((target, out));
        }
    }
    Ok(())
}

/// The summary rows shown before the final confirmation. Plain text: the prompter styles it.
#[must_use]
pub fn summary(a: &Answers) -> Vec<String> {
    let mut rows: Vec<(&str, String)> = vec![("project", a.name.clone())];
    rows.push((
        "layout",
        if a.is_workspace() {
            format!("microservices workspace ({} services)", a.services.len())
        } else {
            "single service".to_string()
        },
    ));
    for (i, (svc, s)) in a.servers.iter().enumerate() {
        let what = match s {
            Server::Node(fw) => format!("Node + {fw}"),
            Server::Java(fw) => format!("Java + {fw}"),
            Server::None => "none".to_string(),
        };
        let label = if i == 0 { "servers" } else { "" };
        rows.push((label, format!("{svc}: {what}")));
    }
    let clients: Vec<String> = a
        .clients
        .iter()
        .map(|c| match c {
            Client::Ts(Some(fw)) => format!("TypeScript + {fw}"),
            Client::Ts(None) => "TypeScript".to_string(),
            Client::Java => "Java".to_string(),
            Client::None => "none".to_string(),
        })
        .collect();
    rows.push((
        "clients",
        if clients.is_empty() {
            "none".to_string()
        } else {
            clients.join(", ")
        },
    ));
    for (i, (t, o)) in default_outs(a).iter().enumerate() {
        let label = if i == 0 { "output" } else { "" };
        rows.push((label, format!("{t} -> {}", a.out(t, o))));
    }
    if !a.description.is_empty() {
        rows.push(("about", a.description.clone()));
    }
    let n = render::files(a).len();
    rows.push(("files", format!("{n} to create")));
    ui_render::rows(&Theme::PLAIN, 0, &rows)
}

/// The questions, in order: name, layout, servers, clients, outputs, description, confirm.
pub fn ask_answers(p: &mut dyn Prompter, dir_name: &str) -> Result<Answers> {
    let name = p.text(&Text {
        label: "Project name",
        default: dir_name,
        placeholder: "",
        required: true,
        validate: valid_name,
    })?;
    let kinds = [
        Choice::new("Single service", "one API, one contracts folder"),
        Choice::new(
            "Microservices workspace",
            "several services that can call each other",
        ),
    ];
    let workspace = p.select("Project kind", &kinds, 0)? == 1;
    let services = if workspace {
        let list = p.text(&Text {
            label: "Service names (comma-separated)",
            default: "users,orders",
            placeholder: "",
            required: true,
            validate: valid_services,
        })?;
        parse_services(&list)?
    } else {
        Vec::new()
    };
    let name = match sanitize(&name) {
        n if n.is_empty() => dir_name.to_string(),
        n => n,
    };
    let names = if workspace {
        services.clone()
    } else {
        vec![name.clone()]
    };
    let servers = ask_servers(p, &names)?;
    let clients = ask_clients(p)?;
    let mut answers = Answers {
        name,
        services,
        servers,
        clients,
        outs: Vec::new(),
        description: String::new(),
    };
    ask_outs(p, &mut answers)?;
    answers.description = p
        .text(&Text {
            label: "Description (optional)",
            default: "",
            placeholder: "what does this API do?",
            required: false,
            validate: any_text,
        })?
        .trim()
        .to_string();
    p.note("Summary", &summary(&answers))?;
    if !p.confirm("Create this project?", true)? {
        return Err(cancelled());
    }
    Ok(answers)
}

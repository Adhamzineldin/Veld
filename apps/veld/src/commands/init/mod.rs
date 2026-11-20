//! `veld init`: scaffold `veld.toml`, starter contracts and `package.json` files.
//!
//! With no flags on a terminal it asks questions (see [`ask_answers`]); every answer also has a
//! flag, so CI never needs a TTY. It never overwrites anything.

mod answers;
mod render;

use std::fs;
use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

use anyhow::{Context, Result};
use veld_project::config::CONFIG_FILE;

use crate::cli::InitArgs;
use crate::error::{Exit, user};
use crate::ui::Ui;
use answers::{Answers, Client, Server, assign_servers, parse_services, sanitize};
use render::File;

/// A source of answers: the terminal in the CLI, a script in tests.
pub trait Prompter {
    /// Asks `question` and returns the answer, or `default` when the answer is empty.
    fn ask(&mut self, question: &str, default: &str) -> Result<String>;
}

/// Reads answers from stdin and prints the questions to stdout.
struct Terminal;

impl Prompter for Terminal {
    fn ask(&mut self, question: &str, default: &str) -> Result<String> {
        let mut out = std::io::stdout().lock();
        write!(out, "{question} [{default}]: ").context("cannot write to the terminal")?;
        out.flush().context("cannot write to the terminal")?;
        let mut line = String::new();
        let n = std::io::stdin()
            .lock()
            .read_line(&mut line)
            .context("cannot read from the terminal")?;
        let line = line.trim();
        Ok(if n == 0 || line.is_empty() {
            default.to_string()
        } else {
            line.to_string()
        })
    }
}

pub fn run(ui: &Ui, args: &InitArgs) -> Result<Exit> {
    let dir = std::env::current_dir().context("cannot read the current directory")?;
    let tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut terminal = Terminal;
    let prompter: Option<&mut dyn Prompter> = if tty && args.is_bare() {
        Some(&mut terminal)
    } else {
        None
    };
    init_in(ui, &dir, args, prompter)
}

/// `init` in `dir`. `prompter` is `Some` for the interactive flow.
pub fn init_in(
    ui: &Ui,
    dir: &Path,
    args: &InitArgs,
    prompter: Option<&mut dyn Prompter>,
) -> Result<Exit> {
    if dir.join(CONFIG_FILE).exists() {
        return Err(user(format!(
            "{CONFIG_FILE} already exists here; nothing was changed"
        )));
    }
    let dir_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(sanitize)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "app".to_string());
    let answers = match prompter {
        Some(p) => ask_answers(p, &dir_name)?,
        None => flag_answers(args, &dir_name)?,
    };
    let written = scaffold(dir, &render::files(&answers))?;
    gitignore(ui, dir)?;
    ui.info(&format!("created {}", written.join(", ")));
    next_steps(ui, &answers);
    Ok(Exit::Ok)
}

/// The answers the flags describe, with the defaults of `--yes` for anything not given.
fn flag_answers(args: &InitArgs, dir_name: &str) -> Result<Answers> {
    let name = match &args.name {
        Some(n) => {
            let n = sanitize(n);
            if n.is_empty() {
                dir_name.to_string()
            } else {
                n
            }
        }
        None => dir_name.to_string(),
    };
    let services = match &args.workspace {
        Some(list) => parse_services(list)?,
        None => Vec::new(),
    };
    let names: Vec<String> = if services.is_empty() {
        vec![name.clone()]
    } else {
        services.clone()
    };
    let client = match &args.client {
        Some(c) => Client::parse(c)?,
        None => Client::Ts(None),
    };
    Ok(Answers {
        servers: assign_servers(&names, &args.server)?,
        name,
        services,
        client,
        outs: Vec::new(),
    })
}

/// Asks until `parse` accepts the answer (three tries).
fn ask_parsed<T>(
    p: &mut dyn Prompter,
    question: &str,
    default: &str,
    parse: impl Fn(&str) -> Result<T>,
) -> Result<T> {
    let mut last = None;
    for _ in 0..3 {
        let text = p.ask(question, default)?;
        match parse(&text) {
            Ok(v) => return Ok(v),
            Err(e) => {
                eprintln!("  {e}");
                last = Some(e);
            }
        }
    }
    Err(last.unwrap_or_else(|| user("no valid answer")))
}

fn parse_kind(text: &str) -> Result<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "single" | "s" | "1" => Ok(false),
        "workspace" | "w" | "microservices" | "m" | "2" => Ok(true),
        other => Err(user(format!(
            "answer `single` or `workspace`, not `{other}`"
        ))),
    }
}

fn parse_out(text: &str) -> Result<String> {
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

/// The interactive questions, in order: name, shape, servers, client, output folders.
fn ask_answers(p: &mut dyn Prompter, dir_name: &str) -> Result<Answers> {
    let name = sanitize(&p.ask("Project name", dir_name)?);
    let name = if name.is_empty() {
        dir_name.to_string()
    } else {
        name
    };
    let kind_q = "Single service or microservices workspace? (single/workspace)";
    let workspace = ask_parsed(p, kind_q, "single", parse_kind)?;
    let services = if workspace {
        ask_parsed(
            p,
            "Service names, comma-separated",
            "users,orders",
            parse_services,
        )?
    } else {
        Vec::new()
    };
    let names = if workspace {
        services.clone()
    } else {
        vec![name.clone()]
    };
    let mut servers = Vec::new();
    for svc in &names {
        let q = format!(
            "Server for `{svc}`: node[:express|fastify|hono|nest|node-http], java[:jdk|spring] or none"
        );
        servers.push((
            svc.clone(),
            ask_parsed(p, &q, "node:express", Server::parse)?,
        ));
    }
    let client_q = "Client: typescript[:react|vue], java or none";
    let client = ask_parsed(p, client_q, "typescript", Client::parse)?;
    let mut answers = Answers {
        name,
        services,
        servers,
        client,
        outs: Vec::new(),
    };
    for (target, default) in default_outs(&answers) {
        let q = format!("Output folder of `{target}`");
        let out = ask_parsed(p, &q, &default, parse_out)?;
        if out != default {
            answers.outs.push((target, out));
        }
    }
    Ok(answers)
}

/// `(target name, default out)` of the config the answers render.
fn default_outs(a: &Answers) -> Vec<(String, String)> {
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

fn next_steps(ui: &Ui, a: &Answers) {
    ui.info("next:");
    ui.info("  veld generate     # write the generated code");
    ui.info("  veld setup        # make it importable: @veld/<target> (npm) or a source root (Maven/Gradle)");
    let npm =
        a.client != Client::None || a.servers.iter().any(|(_, s)| matches!(s, Server::Node(_)));
    if npm {
        ui.info("  npm install       # in each app, after `veld setup`");
    }
}

/// The package cache (`.veld/`) must not be committed. Creates `.gitignore` when there is
/// none; an existing one is never edited, only mentioned.
fn gitignore(ui: &Ui, dir: &Path) -> Result<()> {
    let path = dir.join(".gitignore");
    match fs::read_to_string(&path) {
        Ok(text) if text.lines().any(|l| matches!(l.trim(), ".veld" | ".veld/")) => {}
        Ok(_) => ui.info("add `.veld/` to your .gitignore (the registry package cache)"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::write(&path, "# Veld package cache\n.veld/\nnode_modules/\n")
                .with_context(|| format!("cannot write `{}`", path.display()))?;
            ui.info("created .gitignore (ignores .veld/ and node_modules/)");
        }
        Err(_) => {}
    }
    Ok(())
}

/// Writes every file, or none: refuses if any required file already exists. An optional
/// file that exists is left alone.
fn scaffold(dir: &Path, files: &[File]) -> Result<Vec<String>> {
    if dir.join(CONFIG_FILE).exists() {
        return Err(user(format!(
            "{CONFIG_FILE} already exists here; nothing was changed"
        )));
    }
    let taken: Vec<&str> = files
        .iter()
        .filter(|f| !f.optional && dir.join(&f.path).exists())
        .map(|f| f.path.as_str())
        .collect();
    if !taken.is_empty() {
        return Err(user(format!(
            "refusing to overwrite existing files: {}",
            taken.join(", ")
        )));
    }
    let mut written = Vec::new();
    for f in files
        .iter()
        .filter(|f| !(f.optional && dir.join(&f.path).exists()))
    {
        let target = dir.join(&f.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create `{}`", parent.display()))?;
        }
        fs::write(&target, &f.text)
            .with_context(|| format!("cannot write `{}`", target.display()))?;
        written.push(f.path.clone());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scripted answers; an exhausted script answers with the default.
    struct Script(std::vec::IntoIter<&'static str>, Vec<String>);

    impl Script {
        fn new(answers: Vec<&'static str>) -> Self {
            Self(answers.into_iter(), Vec::new())
        }
    }

    impl Prompter for Script {
        fn ask(&mut self, question: &str, default: &str) -> Result<String> {
            self.1.push(question.to_string());
            Ok(match self.0.next() {
                Some(a) if !a.is_empty() => a.to_string(),
                _ => default.to_string(),
            })
        }
    }

    fn bare() -> InitArgs {
        InitArgs {
            name: None,
            workspace: None,
            server: Vec::new(),
            client: None,
            yes: false,
        }
    }

    #[test]
    fn all_defaults_make_the_classic_single_service() {
        let mut p = Script::new(vec![]);
        let a = ask_answers(&mut p, "demo").unwrap();
        assert_eq!(a, flag_answers(&bare(), "demo").unwrap());
        assert_eq!(p.1.len(), 6, "name, kind, server, client, two outs");
    }

    #[test]
    fn the_workspace_flow_asks_per_service_and_for_outputs() {
        let mut p = Script::new(vec![
            "shop",
            "workspace",
            "users, billing",
            "node:fastify",
            "java:spring",
            "typescript:react",
            "svc/users/gen",
            "",
            "web/src/generated",
        ]);
        let a = ask_answers(&mut p, "x").unwrap();
        assert_eq!(a.name, "shop");
        assert_eq!(a.services, ["users", "billing"]);
        assert_eq!(a.servers[0].1, Server::Node("fastify".into()));
        assert_eq!(a.servers[1].1, Server::Java("spring".into()));
        assert_eq!(a.client, Client::Ts(Some("react".into())));
        assert_eq!(
            a.outs,
            [
                ("users-api".to_string(), "svc/users/gen".to_string()),
                ("web".to_string(), "web/src/generated".to_string()),
            ]
        );
        assert!(p.1.iter().any(|q| q.contains("`billing`")));
    }

    #[test]
    fn a_bad_answer_is_asked_again_and_gives_up_after_three() {
        let mut p = Script::new(vec!["", "single", "cobol", "node:hono"]);
        let a = ask_answers(&mut p, "demo").unwrap();
        assert_eq!(a.servers[0].1, Server::Node("hono".into()));
        let mut p = Script::new(vec!["", "maybe", "maybe", "maybe"]);
        assert!(ask_answers(&mut p, "demo").is_err());
    }

    #[test]
    fn output_folders_must_stay_inside_the_project() {
        for bad in ["", "/abs", "../up", "a/../b", "C:/x"] {
            assert!(parse_out(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_out("a/b/").unwrap(), "a/b");
    }

    #[test]
    fn flags_describe_everything() {
        let args = InitArgs {
            name: Some("my-shop".into()),
            workspace: Some("users,orders".into()),
            server: vec!["java:spring".into(), "users=node:hono".into()],
            client: Some("typescript:vue".into()),
            yes: true,
        };
        let a = flag_answers(&args, "dir").unwrap();
        assert_eq!(a.name, "my_shop");
        assert_eq!(a.servers[0].1, Server::Node("hono".into()));
        assert_eq!(a.servers[1].1, Server::Java("spring".into()));
        assert_eq!(a.client, Client::Ts(Some("vue".into())));
    }

    #[test]
    fn init_writes_and_refuses_as_before() {
        let tmp = tempfile::tempdir().unwrap();
        let ui = Ui::plain();
        init_in(&ui, tmp.path(), &bare(), None).unwrap();
        assert!(tmp.path().join("veld.toml").is_file());
        assert!(tmp.path().join("package.json").is_file());
        let err = init_in(&ui, tmp.path(), &bare(), None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists"), "{err}");
    }

    #[test]
    fn an_existing_package_json_is_kept_and_an_existing_contract_blocks() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("package.json"), "mine").unwrap();
        init_in(&Ui::plain(), tmp.path(), &bare(), None).unwrap();
        assert_eq!(
            fs::read_to_string(tmp.path().join("package.json")).unwrap(),
            "mine"
        );

        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("contracts")).unwrap();
        fs::write(tmp.path().join("contracts/hello.veld"), "mine").unwrap();
        let err = init_in(&Ui::plain(), tmp.path(), &bare(), None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("contracts/hello.veld"), "{err}");
        assert!(!tmp.path().join("veld.toml").exists());
    }

    #[test]
    fn the_interactive_flow_refuses_before_asking() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("veld.toml"), "keep").unwrap();
        let mut p = Script::new(vec![]);
        assert!(init_in(&Ui::plain(), tmp.path(), &bare(), Some(&mut p)).is_err());
        assert!(p.1.is_empty());
    }
}

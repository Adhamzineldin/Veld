//! `veld init`: scaffold `veld.toml`, starter contracts and `package.json` files.
//!
//! With no flags on a terminal it runs the guided flow (see [`flow`]); every answer also has
//! a flag, so CI never needs a TTY. It never overwrites anything.

mod answers;
mod detect;
mod flow;
mod render;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use veld_project::config::CONFIG_FILE;

use crate::cli::{InitArgs, Template};
use crate::error::{Exit, user};
use crate::pipeline;
use crate::ui::Ui;
use crate::ui::prompt::{Clack, Lines, Prompter, is_cancelled};
use crate::ui::render as ui_render;
use answers::{
    Answers, Client, Server, assign_base_urls, assign_consumes, assign_servers, parse_services,
    sanitize,
};
use render::File;

pub fn run(ui: &Ui, args: &InitArgs) -> Result<Exit> {
    let dir = std::env::current_dir().context("cannot read the current directory")?;
    if !(args.is_bare() && Ui::interactive()) {
        return init_with(ui, &dir, args, None, false);
    }
    let theme = ui.theme_err();
    if theme.unicode {
        finish_interactive(&mut Clack::default(), ui, &dir, args, true)
    } else {
        let mut lines = Lines::new(std::io::stdin().lock(), std::io::stderr(), theme, false);
        finish_interactive(&mut lines, ui, &dir, args, true)
    }
}

/// Runs the guided flow; Ctrl-C ends it with "Cancelled", exit 130 and nothing written.
fn finish_interactive(
    p: &mut dyn Prompter,
    ui: &Ui,
    dir: &Path,
    args: &InitArgs,
    offer: bool,
) -> Result<Exit> {
    match init_with(ui, dir, args, Some(&mut *p), offer) {
        Err(e) if is_cancelled(&e) => {
            p.cancel("Cancelled")?;
            Ok(Exit::Cancelled)
        }
        other => other,
    }
}

fn dir_name_of(dir: &Path) -> String {
    dir.file_name()
        .and_then(|n| n.to_str())
        .map(sanitize)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "app".to_string())
}

/// `init` in `dir`. `prompter` is `Some` for the guided flow.
#[cfg(test)]
pub fn init_in(
    ui: &Ui,
    dir: &Path,
    args: &InitArgs,
    prompter: Option<&mut dyn Prompter>,
) -> Result<Exit> {
    init_with(ui, dir, args, prompter, false)
}

/// [`init_in`]; with `offer` the guided flow ends by offering to run generate and setup.
fn init_with(
    ui: &Ui,
    dir: &Path,
    args: &InitArgs,
    prompter: Option<&mut dyn Prompter>,
    offer: bool,
) -> Result<Exit> {
    if dir.join(CONFIG_FILE).exists() {
        return Err(user(format!(
            "{CONFIG_FILE} already exists here; nothing was changed"
        )));
    }
    check_template(args)?;
    let dir_name = dir_name_of(dir);
    let detected = detect::detect(dir);
    let Some(p) = prompter else {
        let mut answers = flag_answers(args, &dir_name)?;
        apply_detected(&mut answers, args, &detected);
        return plain_init(ui, dir, args, &answers);
    };
    p.intro("veld 2.0  contract-first API code generator")?;
    let mut answers = flow::ask_answers_in(p, &dir_name, &detected)?;
    answers.template = args.template.unwrap_or(Template::Hello);
    p.working("Writing files");
    let written = scaffold(dir, &render::files(&answers)).and_then(|w| Ok((w, gitignore(dir)?)));
    let (written, ignored) = match written {
        Ok(w) => w,
        Err(e) => {
            p.done("Could not write the project");
            return Err(e);
        }
    };
    p.done(&format!("Created {}", count_files(written.len())));
    let mut lines: Vec<String> = written.iter().map(|f| format!("+ {f}")).collect();
    lines.extend(ignored.iter().map(|m| format!("+ {m}")));
    p.note("Files", &lines)?;
    let done = if offer {
        offer_generate(p, ui, dir)?
    } else {
        Done::default()
    };
    let steps = steps(&answers, done);
    let steps: Vec<(&str, &str)> = steps
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let body = ui_render::next_steps(&crate::ui::Theme::PLAIN, &steps);
    p.note("Next steps", &body)?;
    p.outro("You are all set. Happy contracting!")?;
    Ok(Exit::Ok)
}

/// What the end of the guided flow already did.
#[derive(Debug, Clone, Copy, Default)]
struct Done {
    generated: bool,
    setup: bool,
}

/// `--template crud` is a single-service starter.
fn check_template(args: &InitArgs) -> Result<()> {
    if args.template == Some(Template::Crud) && args.workspace.is_some() {
        return Err(user(
            "`--template crud` is for a single-service project; drop `--workspace`",
        ));
    }
    Ok(())
}

/// Starts from what the folder already holds (a `package.json`, a `pom.xml`) for everything the
/// flags did not set.
fn apply_detected(answers: &mut Answers, args: &InitArgs, detected: &detect::Detected) {
    if args.server.is_empty()
        && let Some(server) = &detected.server
    {
        for (_, s) in &mut answers.servers {
            *s = server.clone();
        }
    }
    if args.client.is_empty()
        && let Some(client) = &detected.client
    {
        answers.clients = vec![client.clone()];
    }
}

/// The end of the guided flow: offer to run `generate`, then `setup`. A failure is reported but
/// never undoes the project that was just created.
fn offer_generate(p: &mut dyn Prompter, ui: &Ui, dir: &Path) -> Result<Done> {
    let mut done = Done::default();
    if !p.confirm("Run `veld generate` now?", true)? {
        return Ok(done);
    }
    let config = dir.join(CONFIG_FILE);
    let args = crate::cli::GenerateArgs {
        accept_breaking: true,
        ..crate::cli::GenerateArgs::default()
    };
    let result = pipeline::load_project(Some(&config))
        .and_then(|project| super::generate::generate(ui, project, &args));
    match result {
        Ok(Exit::Ok) => done.generated = true,
        Ok(_) => return Ok(done),
        Err(e) => {
            ui.error(&format!("{e:#}"));
            return Ok(done);
        }
    }
    if p.confirm("Run `veld setup` now?", true)? {
        let applied = pipeline::load_project(Some(&config)).and_then(|project| {
            let targets: Vec<_> = project.targets.iter().collect();
            super::setup::apply_to(ui, &project, &targets, false)
        });
        match applied {
            Ok(()) => done.setup = true,
            Err(e) => ui.error(&format!("{e:#}")),
        }
    }
    Ok(done)
}

fn count_files(n: usize) -> String {
    if n == 1 {
        "1 file".to_string()
    } else {
        format!("{n} files")
    }
}

/// The flag and `--yes` path: no prompts, plain lines, no spinner.
fn plain_init(ui: &Ui, dir: &Path, args: &InitArgs, answers: &Answers) -> Result<Exit> {
    let files = render::files(answers);
    if args.dry_run {
        ui.header("init", "dry run");
        for f in &files {
            ui.info(&format!("would write: {}", f.path));
        }
        ui.info("dry run: nothing was written");
        return Ok(Exit::Ok);
    }
    let written = scaffold(dir, &files)?;
    let ignored = gitignore(dir)?;
    ui.header("init", &answers.name);
    ui.ok(&format!("created {}", written.join(", ")));
    for m in &ignored {
        ui.ok(m);
    }
    ui.info("");
    ui.info(&ui.out_styled(crate::ui::Style::Bold, "Next steps"));
    let steps = steps(answers, Done::default());
    let steps: Vec<(&str, &str)> = steps
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    for l in ui_render::next_steps(&ui.theme_out(), &steps) {
        ui.info(&format!("  {l}"));
    }
    Ok(Exit::Ok)
}

/// The commands to run next, with what each does.
fn steps(a: &Answers, done: Done) -> Vec<(String, String)> {
    let contracts = if a.is_workspace() {
        "contracts/<service>/"
    } else if a.template == Template::Crud {
        "contracts/"
    } else {
        "contracts/hello.veld"
    };
    let mut v = vec![(format!("edit {contracts}"), "describe your API".to_string())];
    if !done.generated {
        v.push((
            "veld generate".to_string(),
            "write the generated code".to_string(),
        ));
    }
    if !done.setup {
        v.push((
            "veld setup".to_string(),
            "make it importable: @veld/<target> (npm) or a source root (Maven/Gradle)".to_string(),
        ));
    }
    let npm = !a.clients.is_empty() || a.servers.iter().any(|(_, s)| matches!(s, Server::Node(_)));
    if npm {
        v.push((
            "npm install".to_string(),
            "in each app, after `veld setup`".to_string(),
        ));
    }
    v
}

/// The answers the flags describe, with the defaults of `--yes` for anything not given.
fn flag_answers(args: &InitArgs, dir_name: &str) -> Result<Answers> {
    let name = match &args.name {
        Some(n) => match sanitize(n) {
            n if n.is_empty() => dir_name.to_string(),
            n => n,
        },
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
    let clients = if args.client.is_empty() {
        vec![Client::Ts(None)]
    } else {
        Client::parse_list(&args.client)?
    };
    if services.is_empty() && !(args.base_url.is_empty() && args.consumes.is_empty()) {
        return Err(user(
            "`--base-url` and `--consumes` are for a microservices workspace; add `--workspace`",
        ));
    }
    let base_urls = if services.is_empty() {
        Vec::new()
    } else {
        assign_base_urls(&services, &args.base_url)?
    };
    let consumes = assign_consumes(&services, &args.consumes)?;
    let mut answers = Answers {
        template: args.template.unwrap_or(Template::Hello),
        servers: assign_servers(&names, &args.server)?,
        base_urls,
        consumes,
        name,
        services,
        clients,
        outs: Vec::new(),
        description: args
            .description
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string(),
    };
    apply_outs(&mut answers, &args.out)?;
    Ok(answers)
}

/// `--out TARGET=DIR`: the target must exist in the generated config.
fn apply_outs(answers: &mut Answers, specs: &[String]) -> Result<()> {
    if specs.is_empty() {
        return Ok(());
    }
    let known = flow::default_outs(answers);
    for spec in specs {
        let (target, dir) = spec
            .split_once('=')
            .ok_or_else(|| user(format!("`--out {spec}`: expected TARGET=DIR")))?;
        let target = target.trim();
        if !known.iter().any(|(t, _)| t == target) {
            let names: Vec<&str> = known.iter().map(|(t, _)| t.as_str()).collect();
            return Err(user(format!(
                "`--out {spec}`: no target `{target}` (targets: {})",
                names.join(", ")
            )));
        }
        let dir = flow::parse_out(dir)?;
        answers.outs.retain(|(t, _)| t != target);
        answers.outs.push((target.to_string(), dir));
    }
    Ok(())
}

/// The package cache (`.veld/`) must not be committed. Creates `.gitignore` when there is
/// none; an existing one is never edited, only mentioned. Returns what happened.
fn gitignore(dir: &Path) -> Result<Vec<String>> {
    let path = dir.join(".gitignore");
    match fs::read_to_string(&path) {
        Ok(text) if text.lines().any(|l| matches!(l.trim(), ".veld" | ".veld/")) => Ok(Vec::new()),
        Ok(_) => Ok(vec![
            "add `.veld/` to your .gitignore (the registry package cache)".to_string(),
        ]),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::write(&path, "# Veld package cache\n.veld/\nnode_modules/\n")
                .with_context(|| format!("cannot write `{}`", path.display()))?;
            Ok(vec![
                "created .gitignore (ignores .veld/ and node_modules/)".to_string(),
            ])
        }
        Err(_) => Ok(Vec::new()),
    }
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

/// Runs the guided flow on scripted `input` lines and returns the transcript. For tests.
#[cfg(test)]
fn scripted(dir: &Path, args: &InitArgs, input: &str) -> (Result<Exit>, String) {
    let mut lines = Lines::new(
        input.as_bytes(),
        Vec::<u8>::new(),
        crate::ui::Theme::PLAIN,
        true,
    );
    let r = finish_interactive(&mut lines, &Ui::plain(), dir, args, false);
    (
        r,
        String::from_utf8(lines.into_output()).unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare() -> InitArgs {
        InitArgs {
            name: None,
            workspace: None,
            server: Vec::new(),
            client: Vec::new(),
            base_url: Vec::new(),
            consumes: Vec::new(),
            out: Vec::new(),
            description: None,
            template: None,
            yes: false,
            dry_run: false,
        }
    }

    fn ask(script: &str, dir_name: &str) -> Result<Answers> {
        let mut l = Lines::new(
            script.as_bytes(),
            Vec::<u8>::new(),
            crate::ui::Theme::PLAIN,
            true,
        );
        flow::ask_answers(&mut l, dir_name)
    }

    /// Name, kind, server language, framework, clients, TS framework, change outputs,
    /// description, confirm: all defaults.
    const DEFAULTS: &str = "\n\n\n\n\n\n\n\n\n";

    fn offered(input: &str, dir: &Path) -> Exit {
        let mut lines = Lines::new(
            input.as_bytes(),
            Vec::<u8>::new(),
            crate::ui::Theme::PLAIN,
            true,
        );
        finish_interactive(&mut lines, &Ui::plain(), dir, &bare(), true).unwrap()
    }

    #[test]
    fn the_guided_flow_offers_to_run_generate_and_setup() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(offered(&format!("{DEFAULTS}y\ny\n"), tmp.path()), Exit::Ok);
        assert!(tmp.path().join("generated/server/index.ts").is_file());
        let pkg = fs::read_to_string(tmp.path().join("package.json")).unwrap();
        assert!(pkg.contains("@veld/"), "setup wired the package: {pkg}");
    }

    #[test]
    fn declining_the_offer_leaves_only_the_scaffold() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(offered(&format!("{DEFAULTS}n\n"), tmp.path()), Exit::Ok);
        assert!(tmp.path().join("veld.toml").is_file());
        assert!(!tmp.path().join("generated").exists());
    }

    #[test]
    fn declining_setup_still_generates() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(offered(&format!("{DEFAULTS}y\nn\n"), tmp.path()), Exit::Ok);
        assert!(tmp.path().join("generated/server/index.ts").is_file());
        let pkg = fs::read_to_string(tmp.path().join("package.json")).unwrap();
        assert!(!pkg.contains("@veld/"), "{pkg}");
    }

    #[test]
    fn the_scripted_flow_without_the_offer_asks_nothing_more() {
        let tmp = tempfile::tempdir().unwrap();
        let (r, transcript) = scripted(tmp.path(), &bare(), DEFAULTS);
        assert_eq!(r.unwrap(), Exit::Ok);
        assert!(!transcript.contains("Run `veld generate`"), "{transcript}");
    }
    #[test]
    fn all_defaults_make_the_classic_single_service() {
        let a = ask(DEFAULTS, "demo").unwrap();
        assert_eq!(a, flag_answers(&bare(), "demo").unwrap());
    }

    #[test]
    fn every_node_framework_is_selectable() {
        for (i, fw) in answers::NODE_FRAMEWORKS.iter().enumerate() {
            let script = format!("\n\n1\n{}\n\n\n\n\n\n", i + 1);
            let a = ask(&script, "demo").unwrap();
            assert_eq!(a.servers[0].1, Server::Node((*fw).into()));
        }
    }

    #[test]
    fn every_java_framework_rust_and_none_are_selectable() {
        for (i, fw) in answers::JAVA_FRAMEWORKS.iter().enumerate() {
            let script = format!("\n\n2\n{}\n\n\n\n\n\n", i + 1);
            let a = ask(&script, "demo").unwrap();
            assert_eq!(a.servers[0].1, Server::Java((*fw).into()));
        }
        let rust = ask("\n\n3\n\n\n\n\n\n", "demo").unwrap();
        assert_eq!(rust.servers[0].1, Server::Rust("axum".into()));
        let a = ask("\n\n4\n\n\n\n\n\n", "demo").unwrap();
        assert_eq!(a.servers[0].1, Server::None);
        assert_eq!(a.clients, [Client::Ts(None)]);
    }

    #[test]
    fn every_client_choice_is_selectable() {
        let pick = |clients: &str, ts_fw: &str| {
            ask(&format!("\n\n\n\n{clients}\n{ts_fw}\n\n\n\n"), "demo").unwrap()
        };
        assert_eq!(pick("1", "1").clients, [Client::Ts(None)]);
        assert_eq!(pick("1", "2").clients, [Client::Ts(Some("react".into()))]);
        assert_eq!(pick("1", "3").clients, [Client::Ts(Some("vue".into()))]);
        let java = ask("\n\n\n\n2\n\n\n\n", "demo").unwrap();
        assert_eq!(java.clients, [Client::Java]);
        assert_eq!(pick("3", "1").clients, [Client::Rust("reqwest".into())]);
        assert_eq!(pick("3", "2").clients, [Client::Rust("ureq".into())]);
        let both = pick("1,2", "3");
        assert_eq!(both.clients, [Client::Ts(Some("vue".into())), Client::Java]);
        let none = ask("\n\n\n\n-\n\n\n\n", "demo").unwrap();
        assert!(none.clients.is_empty());
    }

    #[test]
    fn the_workspace_flow_asks_per_service_and_for_outputs() {
        let script = [
            "shop",
            "2",
            "users, billing",
            "n",
            "1",
            "2",
            "2",
            "2",
            "",
            "",
            "1",
            "-",
            "1",
            "2",
            "y",
            "svc/users/gen",
            "",
            "web/src/generated",
            "A shop",
            "",
        ]
        .join("\n")
            + "\n";
        let a = ask(&script, "x").unwrap();
        assert_eq!(a.name, "shop");
        assert_eq!(a.services, ["users", "billing"]);
        assert_eq!(a.servers[0].1, Server::Node("fastify".into()));
        assert_eq!(a.servers[1].1, Server::Java("spring".into()));
        assert_eq!(a.clients, [Client::Ts(Some("react".into()))]);
        assert_eq!(a.description, "A shop");
        assert_eq!(
            a.base_urls,
            [
                ("users".to_string(), "http://localhost:3001".to_string()),
                ("billing".to_string(), "http://localhost:3002".to_string()),
            ]
        );
        assert_eq!(
            a.consumes,
            [("users".to_string(), vec!["billing".to_string()])]
        );
        assert_eq!(
            a.outs,
            [
                ("users-api".to_string(), "svc/users/gen".to_string()),
                ("web".to_string(), "web/src/generated".to_string()),
            ]
        );
    }

    #[test]
    fn one_stack_for_all_services_is_asked_once() {
        let script = "\n2\n\n\n1\n2\n".to_string() + &"\n".repeat(12);
        let a = ask(&script, "x").unwrap();
        assert_eq!(a.services.len(), 2);
        assert_eq!(a.servers[0].1, a.servers[1].1);
    }

    #[test]
    fn bad_answers_are_asked_again() {
        let a = ask("!!!\nshop\n\n\n\n\n\n\n\n\n", "demo").unwrap();
        assert_eq!(a.name, "shop");
        let a = ask("\n2\n1a\nusers\n\n\n\n\n\n\n\n\n\n\n", "demo").unwrap();
        assert_eq!(a.services, ["users"]);
    }

    #[test]
    fn output_folders_must_stay_inside_the_project() {
        for bad in ["", "/abs", "../up", "a/../b", "C:/x"] {
            assert!(flow::parse_out(bad).is_err(), "{bad}");
        }
        assert_eq!(flow::parse_out("a/b/").unwrap(), "a/b");
    }

    #[test]
    fn cancel_in_the_middle_writes_nothing_and_exits_130() {
        let tmp = tempfile::tempdir().unwrap();
        let (r, out) = scripted(tmp.path(), &bare(), "shop\n");
        assert_eq!(r.unwrap(), Exit::Cancelled);
        assert!(out.contains("Cancelled"), "{out}");
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn declining_the_summary_cancels() {
        let tmp = tempfile::tempdir().unwrap();
        let (r, _) = scripted(tmp.path(), &bare(), "\n\n\n\n\n\n\n\nn\n");
        assert_eq!(r.unwrap(), Exit::Cancelled);
        assert!(!tmp.path().join("veld.toml").exists());
    }

    #[test]
    fn the_guided_flow_writes_the_project_and_prints_next_steps() {
        let tmp = tempfile::tempdir().unwrap();
        let (r, out) = scripted(tmp.path(), &bare(), DEFAULTS);
        assert_eq!(r.unwrap(), Exit::Ok);
        assert!(tmp.path().join("veld.toml").is_file());
        for want in [
            "Created",
            "Next steps",
            "veld generate",
            "veld setup",
            "npm install",
        ] {
            assert!(out.contains(want), "{want} in\n{out}");
        }
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn the_guided_flow_refuses_before_asking() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("veld.toml"), "keep").unwrap();
        let (r, out) = scripted(tmp.path(), &bare(), DEFAULTS);
        assert!(r.is_err());
        assert!(out.is_empty(), "{out}");
    }

    #[test]
    fn summary_snapshot() {
        let a = ask(
            &([
                "shop",
                "2",
                "users,billing",
                "y",
                "2",
                "2",
                "",
                "https://billing.example.com",
                "1",
                "-",
                "1,2",
                "3",
                "n",
                "Shop APIs",
                "",
            ]
            .join("\n")
                + "\n"),
            "x",
        );
        let a = a.unwrap_or_else(|e| panic!("{e}"));
        insta::assert_snapshot!(flow::summary(&a).join("\n"));
    }

    #[test]
    fn full_flow_transcript_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("my-shop");
        fs::create_dir(&project).unwrap();
        let script = [
            "",
            "2",
            "users,billing",
            "n",
            "1",
            "2",
            "2",
            "2",
            "",
            "",
            "1",
            "-",
            "1,2",
            "3",
            "y",
            "svc/users/gen",
            "",
            "web/src/generated",
            "",
            "Shop APIs",
            "",
        ]
        .join("\n")
            + "\n";
        let (r, out) = scripted(&project, &bare(), &script);
        assert_eq!(r.unwrap(), Exit::Ok);
        insta::assert_snapshot!(out);
    }

    #[test]
    fn flags_describe_everything() {
        let args = InitArgs {
            base_url: Vec::new(),
            consumes: Vec::new(),
            name: Some("my-shop".into()),
            workspace: Some("users,orders".into()),
            server: vec!["java:spring".into(), "users=node:hono".into()],
            client: vec!["typescript:vue".into(), "java".into()],
            out: vec!["web=ui/gen".into()],
            description: Some(" Shop ".into()),
            template: None,
            yes: true,
            dry_run: false,
        };
        let a = flag_answers(&args, "dir").unwrap();
        assert_eq!(a.name, "my_shop");
        assert_eq!(a.servers[0].1, Server::Node("hono".into()));
        assert_eq!(a.servers[1].1, Server::Java("spring".into()));
        assert_eq!(a.clients, [Client::Ts(Some("vue".into())), Client::Java]);
        assert_eq!(a.outs, [("web".to_string(), "ui/gen".to_string())]);
        assert_eq!(a.description, "Shop");
        let bad = InitArgs {
            out: vec!["nope=x".into()],
            ..bare()
        };
        assert!(flag_answers(&bad, "dir").is_err());
    }

    #[test]
    fn flags_set_base_urls_and_consumes_and_nothing_is_assigned_otherwise() {
        let mk = |base: &[&str], consumes: &[&str]| InitArgs {
            workspace: Some("users,orders".into()),
            base_url: base.iter().map(|s| (*s).to_string()).collect(),
            consumes: consumes.iter().map(|s| (*s).to_string()).collect(),
            yes: true,
            ..bare()
        };
        let a = flag_answers(&mk(&[], &[]), "dir").unwrap();
        assert!(a.consumes.is_empty(), "no auto-assigned consumes");
        assert_eq!(a.base_url("orders"), "http://localhost:3002");
        let a = flag_answers(
            &mk(&["orders=https://orders.example.com"], &["orders=users"]),
            "dir",
        )
        .unwrap();
        assert_eq!(a.base_url("orders"), "https://orders.example.com");
        assert_eq!(a.consumes_of("orders"), ["users"]);
        assert!(a.consumes_of("users").is_empty());
        assert!(flag_answers(&mk(&[], &["orders=ghost"]), "dir").is_err());
        let single = InitArgs {
            base_url: vec!["a=http://x".into()],
            yes: true,
            ..bare()
        };
        assert!(flag_answers(&single, "dir").is_err());
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
    fn dry_run_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let args = InitArgs {
            dry_run: true,
            ..bare()
        };
        init_in(&Ui::plain(), tmp.path(), &args, None).unwrap();
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
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
}

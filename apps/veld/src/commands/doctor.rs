//! `veld doctor`: is this project, and the machine it runs on, ready to generate?
//!
//! Each check prints one line (`✓` ok, `!` warning, `✗` failure) with a fix under every
//! problem. Any failure exits 1; warnings do not.

use std::path::{Path, PathBuf};

use anyhow::Result;
use veld_plugin_host::{HostConfig, PluginGenerator};
use veld_project::config::{EmitterRef, Project, Role, TargetSpec};
use veld_project::packages::{Lockfile, install_state};

use crate::error::Exit;
use crate::pipeline;
use crate::registry;
use crate::ui::{Style, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug)]
struct Line {
    status: Status,
    title: String,
    detail: String,
    fix: Option<String>,
}

impl Line {
    fn ok(title: &str, detail: impl Into<String>) -> Self {
        Self::new(Status::Ok, title, detail, None)
    }
    fn warn(title: &str, detail: impl Into<String>, fix: &str) -> Self {
        Self::new(Status::Warn, title, detail, Some(fix.to_string()))
    }
    fn fail(title: &str, detail: impl Into<String>, fix: &str) -> Self {
        Self::new(Status::Fail, title, detail, Some(fix.to_string()))
    }
    fn new(status: Status, title: &str, detail: impl Into<String>, fix: Option<String>) -> Self {
        Self {
            status,
            title: title.to_string(),
            detail: detail.into(),
            fix,
        }
    }
}

pub fn run(ui: &Ui, config: Option<&Path>) -> Result<Exit> {
    ui.header("doctor", "");
    let mut lines = Vec::new();
    match pipeline::load_project(config) {
        Err(e) => lines.push(Line::fail(
            "config",
            e.to_string(),
            "fix veld.toml (or run `veld init` to create one)",
        )),
        Ok(project) => {
            lines.push(Line::ok(
                "config",
                format!("veld.toml loads ({})", project.name),
            ));
            project_checks(&project, &mut lines);
        }
    }
    lines.push(tool_line(
        "git",
        "git",
        "needed by `veld diff --against`",
        true,
    ));
    Ok(print(ui, &lines))
}

fn project_checks(project: &Project, lines: &mut Vec<Line>) {
    lines.push(packages_line(project));
    lines.push(registry_line(project));
    lines.push(compile_line(project));
    lines.push(lint_line(project));
    lines.extend(
        project
            .targets
            .iter()
            .flat_map(|t| target_lines(project, t)),
    );
    lines.push(baseline_line(project));
    lines.extend(tool_lines(project));
}

fn packages_line(project: &Project) -> Line {
    let missing: Vec<String> = project
        .packages
        .iter()
        .filter(|p| !p.root.is_dir())
        .map(|p| format!("{} ({})", p.name, pipeline::display_path(&p.root)))
        .collect();
    if missing.is_empty() {
        Line::ok(
            "packages",
            format!(
                "{} found",
                pipeline::plural(project.packages.len(), "package")
            ),
        )
    } else {
        Line::fail(
            "packages",
            format!("contracts directory missing: {}", missing.join(", ")),
            "create the directory or fix `contracts = ...` in veld.toml",
        )
    }
}

fn registry_line(project: &Project) -> Line {
    if project.dependencies.is_empty() {
        return Line::ok("registry", "no registry dependencies");
    }
    let lock = match Lockfile::read(&project.root) {
        Ok(Some(l)) => l,
        Ok(None) => {
            return Line::fail(
                "registry",
                "veld.lock is missing",
                "run `veld add @org/name` for each dependency, or `veld install`",
            );
        }
        Err(e) => {
            return Line::fail(
                "registry",
                e.to_string(),
                "delete veld.lock and re-run `veld add`",
            );
        }
    };
    let unpinned: Vec<&str> = project
        .dependencies
        .keys()
        .filter(|d| lock.get(d).is_none())
        .map(String::as_str)
        .collect();
    if !unpinned.is_empty() {
        return Line::fail(
            "registry",
            format!("not pinned in veld.lock: {}", unpinned.join(", ")),
            "run `veld add <package>` for each",
        );
    }
    match install_state(&project.root) {
        Ok(s) if s.missing.is_empty() => Line::ok(
            "registry",
            format!("veld.lock is consistent ({} pinned)", lock.packages.len()),
        ),
        Ok(s) => Line::fail(
            "registry",
            format!("{} pinned package(s) are not installed", s.missing.len()),
            "run `veld install`",
        ),
        Err(e) => Line::fail("registry", e.to_string(), "run `veld install`"),
    }
}

fn compile_line(project: &Project) -> Line {
    match pipeline::compile_project(project) {
        Err(e) => Line::fail("contracts", e.to_string(), "fix the problem and re-run"),
        Ok(out) => {
            let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
            if errors == 0 {
                Line::ok("contracts", "compile without errors")
            } else {
                Line::fail(
                    "contracts",
                    pipeline::plural(errors, "error"),
                    "run `veld check` to see them",
                )
            }
        }
    }
}

fn lint_line(project: &Project) -> Line {
    match veld_tools::lint::LintConfig::from_levels(&project.lint) {
        Ok(_) => Line::ok("lint", "[lint] rules are valid"),
        Err(e) => Line::fail("lint", e.to_string(), "fix the rule name in `[lint]`"),
    }
}

fn target_lines(project: &Project, t: &TargetSpec) -> Vec<Line> {
    let title = format!("target {}", t.name);
    let emitter = match emitter_line(project, t, &title) {
        Ok(line) => line,
        Err(line) => return vec![line],
    };
    vec![emitter, out_line(t, &title)]
}

fn emitter_line(project: &Project, t: &TargetSpec, title: &str) -> Result<Line, Line> {
    match &t.emitter {
        EmitterRef::Builtin(id) => {
            let Some(g) = registry::find(id) else {
                return Err(Line::fail(
                    title,
                    format!("unknown emitter `{id}`"),
                    &format!("use one of: {}", registry::ids().join(", ")),
                ));
            };
            let role = match t.role {
                Role::Server => veld_codegen::Role::Server,
                Role::Client => veld_codegen::Role::Client,
            };
            if g.roles().contains(&role) {
                Ok(Line::ok(title, format!("emitter `{id}` ({})", role.name())))
            } else {
                Err(Line::fail(
                    title,
                    format!("emitter `{id}` cannot produce the `{}` role", role.name()),
                    "change `role` or `emitter` in veld.toml",
                ))
            }
        }
        EmitterRef::Plugin(p) => {
            let Some(spec) = project.plugins.iter().find(|s| &s.name == p) else {
                return Err(Line::fail(
                    title,
                    format!("plugin `{p}` is not declared"),
                    &format!("add `[plugins.{p}]` with a `command` to veld.toml"),
                ));
            };
            match PluginGenerator::connect(spec, HostConfig::new(project.root.clone())) {
                Ok(_) => Ok(Line::ok(
                    title,
                    format!("plugin `{p}` starts and its hash matches"),
                )),
                Err(e) => Err(Line::fail(
                    title,
                    format!("plugin `{p}`: error[{}]: {e}", e.code()),
                    "install the plugin, or update `sha256`/`version` in `[plugins]`",
                )),
            }
        }
    }
}

/// The output directory must be creatable (or already writable).
fn out_line(t: &TargetSpec, title: &str) -> Line {
    let shown = pipeline::display_path(&t.out);
    if t.out.exists() && !t.out.is_dir() {
        return Line::fail(
            title,
            format!("`{shown}` exists but is not a directory"),
            "pick another `out`",
        );
    }
    match writable(&t.out) {
        Ok(()) => Line::ok(title, format!("`{shown}` is writable")),
        Err(e) => Line::fail(
            title,
            format!("cannot write to `{shown}`: {e}"),
            "check the folder permissions",
        ),
    }
}

/// Probes the nearest existing ancestor of `dir` by creating and removing a file there.
fn writable(dir: &Path) -> std::io::Result<()> {
    let base = dir
        .ancestors()
        .find(|d| d.is_dir())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let probe = base.join(format!(".veld-doctor-{}", std::process::id()));
    std::fs::write(&probe, b"")?;
    std::fs::remove_file(&probe)
}

fn baseline_line(project: &Project) -> Line {
    let path = pipeline::lock_path(project);
    if !path.is_file() {
        return Line::ok(
            "baseline",
            "no veld.lock.ir.json yet (the first `veld generate` creates it)",
        );
    }
    match pipeline::read_lock(project) {
        Some(ir) => Line::ok(
            "baseline",
            format!(
                "veld.lock.ir.json is valid ({})",
                pipeline::plural(ir.services.len(), "service")
            ),
        ),
        None => Line::fail(
            "baseline",
            "veld.lock.ir.json cannot be read",
            "run `veld clean --lock`, then `veld generate`",
        ),
    }
}

/// Tools that the wired targets need.
fn tool_lines(project: &Project) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut node = false;
    let mut java = false;
    let mut rust = false;
    for t in &project.targets {
        if let EmitterRef::Builtin(id) = &t.emitter {
            node |= id == "node" || id == "typescript";
            java |= id == "java";
            rust |= id == "rust";
        }
    }
    if node {
        lines.push(tool_line(
            "node",
            "node",
            "to run and build the TypeScript targets",
            false,
        ));
        lines.push(tool_line(
            "npm",
            "npm",
            "to install the generated packages",
            false,
        ));
    }
    if java {
        lines.push(tool_line(
            "javac",
            "javac",
            "to compile the Java targets (install a JDK)",
            false,
        ));
        match build_tool(project) {
            Some(BuildTool::Gradle) => lines.push(tool_line(
                "gradle",
                "gradle",
                "to build the Java server targets",
                false,
            )),
            Some(BuildTool::Maven) => lines.push(tool_line(
                "mvn",
                "mvn",
                "to build the Java server targets",
                false,
            )),
            None => {}
        }
    }
    if rust {
        lines.push(tool_line(
            "cargo",
            "cargo",
            "to build the Rust targets (install Rust from https://rustup.rs)",
            false,
        ));
    }
    lines
}

/// The build tool a Java target needs `veld doctor` to find.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildTool {
    Maven,
    Gradle,
}

/// Maven or Gradle, only for Java SERVER targets whose app directory builds with it (`pom.xml` or
/// `build.gradle(.kts)`). A client-only project, or a server without a build file, needs neither:
/// `javac` is enough to compile the generated sources.
fn build_tool(project: &Project) -> Option<BuildTool> {
    let markers = ["pom.xml", "build.gradle.kts", "build.gradle"];
    let mut found = None;
    for t in &project.targets {
        let is_java_server =
            t.role == Role::Server && matches!(&t.emitter, EmitterRef::Builtin(id) if id == "java");
        if !is_java_server {
            continue;
        }
        let Some(app) = crate::setup::app_dir(project, t, &markers) else {
            continue;
        };
        if app.join("pom.xml").is_file() {
            return Some(BuildTool::Maven);
        }
        if app.join("build.gradle").is_file() || app.join("build.gradle.kts").is_file() {
            found = Some(BuildTool::Gradle);
        }
    }
    found
}

fn tool_line(title: &str, program: &str, why: &str, optional: bool) -> Line {
    match find_on_path(program) {
        Some(p) => Line::ok(title, format!("found {}", p.display())),
        None if optional => Line::warn(
            title,
            "not found on PATH",
            &format!("install {program} ({why})"),
        ),
        None => Line::fail(
            title,
            "not found on PATH",
            &format!("install {program} ({why})"),
        ),
    }
}

/// `program` on `PATH` (with the `PATHEXT` extensions on Windows), without running it.
fn find_on_path(program: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".to_string())
            .split(';')
            .map(str::to_ascii_lowercase)
            .collect()
    } else {
        Vec::new()
    };
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|dir| {
        let plain = dir.join(program);
        if plain.is_file() {
            return Some(plain);
        }
        exts.iter()
            .map(|e| dir.join(format!("{program}{e}")))
            .find(|p| p.is_file())
    })
}

fn print(ui: &Ui, lines: &[Line]) -> Exit {
    let t = ui.theme_out();
    let width = lines.iter().map(|l| l.title.len()).max().unwrap_or(0);
    for l in lines {
        let (mark, style) = match l.status {
            Status::Ok => (t.sym().ok, Style::Green),
            Status::Warn => ("!", Style::Yellow),
            Status::Fail => (t.sym().fail, Style::Red),
        };
        ui.data(&format!(
            "{} {:<width$}  {}",
            t.paint(style, mark),
            l.title,
            l.detail
        ));
        if let (Some(fix), true) = (&l.fix, l.status != Status::Ok) {
            ui.data(&format!(
                "    {}",
                ui.out_styled(Style::Dim, &format!("-> fix: {fix}"))
            ));
        }
    }
    let failed = lines.iter().filter(|l| l.status == Status::Fail).count();
    let warned = lines.iter().filter(|l| l.status == Status::Warn).count();
    if failed > 0 {
        eprintln!(
            "{}",
            crate::ui::render::fail(
                &ui.theme_err(),
                &format!("{} found", pipeline::plural(failed, "problem"))
            )
        );
        return Exit::User;
    }
    let note = match warned {
        0 => String::new(),
        n => format!(" ({})", pipeline::plural(n, "warning")),
    };
    ui.ok(&format!("everything looks good{note}"));
    Exit::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_is_not_found() {
        assert!(find_on_path("veld-definitely-not-installed-xyz").is_none());
    }

    fn project_in(dir: &Path, role: &str, build_file: Option<&str>) -> Project {
        if let Some(name) = build_file {
            std::fs::create_dir_all(dir.join("app")).unwrap();
            std::fs::write(dir.join("app").join(name), "").unwrap();
        }
        let text = format!(
            "[service]
name = \"s\"
[{role}]
name = \"j\"
emitter = \"java\"
out = \"app/src/gen\"
app = \"app\"
"
        );
        veld_project::config::load_str(dir, &text).unwrap()
    }

    #[test]
    fn maven_is_needed_only_by_a_java_server_that_builds_with_it() {
        let tmp = tempfile::tempdir().unwrap();
        let p = project_in(tmp.path(), "server", Some("pom.xml"));
        assert_eq!(build_tool(&p), Some(BuildTool::Maven));
        let names: Vec<String> = tool_lines(&p).into_iter().map(|l| l.title).collect();
        assert!(names.iter().any(|n| n == "mvn"), "{names:?}");
    }

    #[test]
    fn a_gradle_server_needs_gradle_and_never_mvn() {
        let tmp = tempfile::tempdir().unwrap();
        let p = project_in(tmp.path(), "server", Some("build.gradle.kts"));
        assert_eq!(build_tool(&p), Some(BuildTool::Gradle));
        let names: Vec<String> = tool_lines(&p).into_iter().map(|l| l.title).collect();
        assert!(
            names.iter().any(|n| n == "gradle") && !names.iter().any(|n| n == "mvn"),
            "{names:?}"
        );
    }

    #[test]
    fn a_java_client_needs_no_build_tool_even_next_to_a_pom() {
        let tmp = tempfile::tempdir().unwrap();
        let p = project_in(tmp.path(), "client", Some("pom.xml"));
        assert_eq!(build_tool(&p), None);
        let names: Vec<String> = tool_lines(&p).into_iter().map(|l| l.title).collect();
        assert!(
            names.iter().any(|n| n == "javac") && !names.iter().any(|n| n == "mvn"),
            "{names:?}"
        );
    }

    #[test]
    fn a_java_server_without_a_build_file_needs_no_build_tool() {
        let tmp = tempfile::tempdir().unwrap();
        let p = project_in(tmp.path(), "server", None);
        assert_eq!(build_tool(&p), None);
    }

    #[test]
    fn writable_probe_cleans_up_after_itself() {
        let tmp = tempfile::tempdir().unwrap();
        writable(&tmp.path().join("deep/new/out")).unwrap();
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }
}

//! `veld watch` (and `veld generate --watch`): regenerate when a contract, `veld.toml` or
//! `veld.lock` changes. Diagnostics are shown inline and never end the watch.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use veld_project::config::{CONFIG_FILE, Project};

use crate::cli::{GenerateArgs, WatchArgs};
use crate::commands::generate;
use crate::error::{self, Exit};
use crate::pipeline;
use crate::setup::relative;
use crate::ui::render;
use crate::ui::{Style, Ui};

/// The lock file Veld writes next to `veld.toml`.
const LOCK_FILE: &str = "veld.lock";

/// What a watch run needs from the command line.
#[derive(Debug, Clone)]
pub struct Options {
    pub target: Option<String>,
    pub force: bool,
    pub dry_run: bool,
    /// Quiet period after the last change before regenerating.
    pub debounce: Duration,
}

impl From<&WatchArgs> for Options {
    fn from(a: &WatchArgs) -> Self {
        Options {
            target: a.target.clone(),
            force: a.force,
            dry_run: a.dry_run,
            debounce: Duration::from_millis(a.debounce),
        }
    }
}

impl From<&GenerateArgs> for Options {
    fn from(a: &GenerateArgs) -> Self {
        Options {
            target: a.target.clone(),
            force: a.force,
            dry_run: a.dry_run,
            debounce: Duration::from_millis(300),
        }
    }
}

pub fn run(ui: &Ui, config: Option<&Path>, opts: &Options) -> Result<Exit> {
    let file = pipeline::config_file(config)?;
    // A second handler (in-process tests) is harmless to refuse.
    let _ = ctrlc::set_handler(|| {
        println!("\nwatch stopped");
        std::process::exit(0);
    });
    let mut first = true;
    loop {
        let project = pipeline::load_project(Some(&file));
        let watched = Watched::of(&file, project.as_ref().ok());
        if first {
            print_header(ui, &file, project.as_ref().ok(), &watched);
        }
        run_once(ui, project, opts, !first);
        first = false;
        let (_watcher, rx) = watch(&watched)?;
        ui.info(&status_line(ui, "watching for changes", Style::Cyan));
        let changed = wait_for_change(&rx, &watched.outs, opts.debounce)?;
        let shown: Vec<String> = changed.iter().map(|p| relative(&watched.base, p)).collect();
        ui.info(&render::stamped(
            &ui.theme_out(),
            &now(),
            "changed",
            &shown.join(", "),
        ));
    }
}

/// Paths one watch cycle depends on.
struct Watched {
    base: PathBuf,
    config: PathBuf,
    roots: Vec<PathBuf>,
    outs: Vec<PathBuf>,
}

impl Watched {
    fn of(config: &Path, project: Option<&Project>) -> Self {
        let base = config.parent().unwrap_or(Path::new(".")).to_path_buf();
        Watched {
            base,
            config: config.to_path_buf(),
            roots: project
                .map(|p| p.packages.iter().map(|pkg| pkg.root.clone()).collect())
                .unwrap_or_default(),
            outs: project
                .map(|p| p.targets.iter().map(|t| t.out.clone()).collect())
                .unwrap_or_default(),
        }
    }
}

fn print_header(ui: &Ui, file: &Path, project: Option<&Project>, w: &Watched) {
    ui.header("watch", project.map_or("", |p| p.name.as_str()));
    let mut rows: Vec<(&str, String)> = Vec::new();
    let contracts: Vec<String> = w.roots.iter().map(|r| relative(&w.base, r)).collect();
    if !contracts.is_empty() {
        rows.push(("contracts", contracts.join(", ")));
    }
    let name = |p: &Path| {
        p.file_name().map_or_else(
            || CONFIG_FILE.to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    };
    rows.push(("config", name(file)));
    rows.push(("lock", LOCK_FILE.to_string()));
    ui.info("Watching:");
    ui.lines(&render::rows(&ui.theme_out(), 2, &rows));
    ui.info(&render::hint(&ui.theme_out(), "Ctrl-C to stop"));
    ui.info("");
}

/// One generate run with a timestamped result line. Errors and diagnostics are printed, not
/// returned: a broken contract must not end the watch.
fn run_once(ui: &Ui, project: Result<Project>, opts: &Options, rerun: bool) {
    let args = GenerateArgs {
        target: opts.target.clone(),
        dry_run: opts.dry_run,
        force: opts.force,
        watch: false,
    };
    let t = ui.theme_out();
    let started = Instant::now();
    let result = project.and_then(|p| generate::generate(ui, p, &args));
    let took = render::elapsed(started.elapsed());
    let ok = match result {
        Ok(Exit::Ok) => true,
        Ok(_) => false,
        Err(err) => {
            error::report(ui, &err);
            false
        }
    };
    let (kind, msg) = match (ok, rerun) {
        (true, true) => ("ok", format!("regenerated in {took}")),
        (true, false) => ("ready", format!("ready in {took}")),
        (false, _) => (
            "fail",
            format!("failed after {took}; fix the problem, watching continues"),
        ),
    };
    ui.info(&render::stamped(&t, &now(), kind, &msg));
}

fn status_line(ui: &Ui, text: &str, style: Style) -> String {
    let t = ui.theme_out();
    format!(
        "{} {}",
        t.paint(style, t.sym().active),
        t.paint(Style::Dim, text)
    )
}

/// `HH:MM:SS`, local time when the platform lets us know it, else UTC.
fn now() -> String {
    let t = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second())
}

fn watch(w: &Watched) -> Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .context("cannot start the file watcher")?;
    let parent = w.config.parent().unwrap_or(Path::new("."));
    watcher
        .watch(parent, RecursiveMode::NonRecursive)
        .with_context(|| format!("cannot watch `{}`", parent.display()))?;
    for root in &w.roots {
        if root.is_dir() {
            watcher
                .watch(root, RecursiveMode::Recursive)
                .with_context(|| format!("cannot watch `{}`", root.display()))?;
        }
    }
    Ok((watcher, rx))
}

/// Blocks until a relevant change arrives, then collects changes until `debounce` of quiet.
/// Returns the changed files, sorted and distinct.
fn wait_for_change(
    rx: &Receiver<notify::Result<Event>>,
    outs: &[PathBuf],
    debounce: Duration,
) -> Result<Vec<PathBuf>> {
    let mut changed = BTreeSet::new();
    loop {
        let event = rx.recv().context("the file watcher stopped")?;
        if collect(&event, outs, &mut changed) {
            break;
        }
    }
    loop {
        match rx.recv_timeout(debounce) {
            Ok(event) => {
                collect(&event, outs, &mut changed);
            }
            Err(RecvTimeoutError::Timeout) => return Ok(changed.into_iter().collect()),
            Err(RecvTimeoutError::Disconnected) => anyhow::bail!("the file watcher stopped"),
        }
    }
}

/// Adds the relevant paths of `event`; true if there were any.
fn collect(event: &notify::Result<Event>, outs: &[PathBuf], into: &mut BTreeSet<PathBuf>) -> bool {
    let Ok(e) = event else { return false };
    let before = into.len();
    let mut any = false;
    for p in e.paths.iter().filter(|p| path_matters(p, outs)) {
        any = true;
        into.insert(p.clone());
    }
    any || into.len() > before
}

/// A `.veld` file, `veld.toml` or `veld.lock`, and not something inside a target's output
/// directory (so the watch never reacts to its own output).
fn path_matters(path: &Path, outs: &[PathBuf]) -> bool {
    let name_ok = path.extension().is_some_and(|e| e == "veld")
        || path
            .file_name()
            .is_some_and(|n| n == CONFIG_FILE || n == LOCK_FILE);
    name_ok && !outs.iter().any(|o| path.starts_with(o))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_contract_files_config_and_lock_matter() {
        let outs = [PathBuf::from("/p/gen")];
        assert!(path_matters(Path::new("/p/contracts/a.veld"), &outs));
        assert!(path_matters(Path::new("/p/veld.toml"), &outs));
        assert!(path_matters(Path::new("/p/veld.lock"), &outs));
        assert!(!path_matters(Path::new("/p/veld.lock.ir.json"), &outs));
        assert!(!path_matters(Path::new("/p/contracts/readme.md"), &outs));
        assert!(!path_matters(Path::new("/p/gen/x.veld"), &outs));
        assert!(!path_matters(Path::new("/p/gen/veld.lock"), &outs));
    }

    #[test]
    fn debounce_waits_for_quiet_and_reports_distinct_files() {
        let (tx, rx) = mpsc::channel();
        let ev = |p: &str| Event::new(notify::EventKind::Any).add_path(PathBuf::from(p));
        tx.send(Ok(ev("/p/b.veld"))).unwrap();
        tx.send(Ok(ev("/p/a.veld"))).unwrap();
        tx.send(Ok(ev("/p/b.veld"))).unwrap();
        let quiet = Duration::from_millis(100);
        let start = Instant::now();
        let files = wait_for_change(&rx, &[], quiet).unwrap();
        assert!(start.elapsed() >= quiet);
        assert_eq!(
            files,
            [PathBuf::from("/p/a.veld"), PathBuf::from("/p/b.veld")]
        );
    }

    #[test]
    fn options_come_from_either_command() {
        let w = WatchArgs {
            target: Some("api".into()),
            force: true,
            dry_run: false,
            debounce: 50,
        };
        let o = Options::from(&w);
        assert_eq!(o.debounce, Duration::from_millis(50));
        assert_eq!(o.target.as_deref(), Some("api"));
    }
}

//! `veld generate --watch`: regenerate when a `.veld` file or `veld.toml` changes.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use veld_project::config::{CONFIG_FILE, Project};

use crate::cli::GenerateArgs;
use crate::commands::generate;
use crate::error::{self, Exit};
use crate::pipeline;
use crate::ui::Ui;

/// Quiet period after the last change before regenerating.
const DEBOUNCE: Duration = Duration::from_millis(300);

pub fn run(ui: &Ui, config: Option<&Path>, args: &GenerateArgs) -> Result<Exit> {
    let file = pipeline::config_file(config)?;
    loop {
        let project = pipeline::load_project(Some(&file));
        let (roots, outs) = match &project {
            Ok(p) => (
                p.packages.iter().map(|pkg| pkg.root.clone()).collect(),
                p.targets.iter().map(|t| t.out.clone()).collect(),
            ),
            Err(_) => (Vec::new(), Vec::new()),
        };
        run_once(ui, project, args);
        let (_watcher, rx) = watch(&file, &roots)?;
        ui.info("watching for changes (press Ctrl+C to stop)");
        wait_for_change(&rx, &outs)?;
    }
}

fn run_once(ui: &Ui, project: Result<Project>, args: &GenerateArgs) {
    let result = project.and_then(|p| generate::generate(ui, p, args));
    if let Err(err) = result {
        error::report(ui, &err);
    }
}

fn watch(
    config: &Path,
    roots: &[PathBuf],
) -> Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .context("cannot start the file watcher")?;
    let parent = config.parent().unwrap_or(Path::new("."));
    watcher
        .watch(parent, RecursiveMode::NonRecursive)
        .with_context(|| format!("cannot watch `{}`", parent.display()))?;
    for root in roots {
        if root.is_dir() {
            watcher
                .watch(root, RecursiveMode::Recursive)
                .with_context(|| format!("cannot watch `{}`", root.display()))?;
        }
    }
    Ok((watcher, rx))
}

/// Blocks until a relevant change arrives, then waits for [`DEBOUNCE`] of quiet.
fn wait_for_change(rx: &Receiver<notify::Result<Event>>, outs: &[PathBuf]) -> Result<()> {
    loop {
        let event = rx.recv().context("the file watcher stopped")?;
        if is_relevant(&event, outs) {
            break;
        }
    }
    loop {
        match rx.recv_timeout(DEBOUNCE) {
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => return Ok(()),
            Err(RecvTimeoutError::Disconnected) => anyhow::bail!("the file watcher stopped"),
        }
    }
}

fn is_relevant(event: &notify::Result<Event>, outs: &[PathBuf]) -> bool {
    event
        .as_ref()
        .is_ok_and(|e| e.paths.iter().any(|p| path_matters(p, outs)))
}

/// A `.veld` file or `veld.toml`, and not something inside a target's output directory.
fn path_matters(path: &Path, outs: &[PathBuf]) -> bool {
    let name_ok = path.extension().is_some_and(|e| e == "veld")
        || path.file_name().is_some_and(|n| n == CONFIG_FILE);
    name_ok && !outs.iter().any(|o| path.starts_with(o))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_contract_files_and_config_matter() {
        let outs = [PathBuf::from("/p/gen")];
        assert!(path_matters(Path::new("/p/contracts/a.veld"), &outs));
        assert!(path_matters(Path::new("/p/veld.toml"), &outs));
        assert!(!path_matters(Path::new("/p/contracts/readme.md"), &outs));
        assert!(!path_matters(Path::new("/p/gen/x.veld"), &outs));
    }

    #[test]
    fn debounce_waits_for_quiet_then_returns() {
        let (tx, rx) = mpsc::channel();
        let ev = Event::new(notify::EventKind::Any).add_path(PathBuf::from("/p/a.veld"));
        tx.send(Ok(ev.clone())).unwrap();
        tx.send(Ok(ev)).unwrap();
        let start = std::time::Instant::now();
        wait_for_change(&rx, &[]).unwrap();
        assert!(start.elapsed() >= DEBOUNCE);
    }
}

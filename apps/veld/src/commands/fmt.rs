//! `veld fmt`: canonical formatting of contracts. Files with syntax errors are never touched.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use veld_compiler::SourceMap;

use crate::cli::FmtArgs;
use crate::error::{Exit, user};
use crate::fsutil;
use crate::pipeline;
use crate::ui::{Style, Ui};

pub fn run(ui: &Ui, config: Option<&Path>, args: &FmtArgs) -> Result<Exit> {
    let files = if args.paths.is_empty() {
        project_files(config)?
    } else {
        explicit_files(&args.paths)?
    };
    if files.is_empty() {
        ui.info("no .veld files to format");
        return Ok(Exit::Ok);
    }
    let (mut changed, mut failed) = (Vec::new(), 0usize);
    for file in &files {
        match format_file(ui, file, args.check)? {
            Outcome::Unchanged => {}
            Outcome::Changed => changed.push(pipeline::display_path(file)),
            Outcome::Failed => failed += 1,
        }
    }
    Ok(summarize(ui, args.check, files.len(), &changed, failed))
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Unchanged,
    Changed,
    Failed,
}

fn format_file(ui: &Ui, file: &Path, check: bool) -> Result<Outcome> {
    let text =
        fs::read_to_string(file).with_context(|| format!("cannot read `{}`", file.display()))?;
    match veld_tools::format(&text) {
        Ok(out) if out == text => Ok(Outcome::Unchanged),
        Ok(out) => {
            if !check {
                fsutil::atomic_write(file, out.as_bytes())?;
            }
            Ok(Outcome::Changed)
        }
        Err(diags) => {
            let mut sources = SourceMap::default();
            sources.add(pipeline::display_path(file), text);
            for d in &diags {
                ui.diagnostic(&sources.render(d));
            }
            ui.error(&format!(
                "{}: not formatted because of syntax errors",
                pipeline::display_path(file)
            ));
            Ok(Outcome::Failed)
        }
    }
}

fn summarize(ui: &Ui, check: bool, total: usize, changed: &[String], failed: usize) -> Exit {
    if check {
        for path in changed {
            ui.data(&format!("would reformat {path}"));
        }
        if !changed.is_empty() {
            ui.error(&format!(
                "{} would change; run `veld fmt`",
                pipeline::plural(changed.len(), "file")
            ));
        }
    } else {
        for path in changed {
            ui.info(&format!(
                "{} {path}",
                ui.out_styled(Style::Green, "formatted")
            ));
        }
        ui.info(&format!(
            "{} checked, {} reformatted",
            pipeline::plural(total, "file"),
            changed.len()
        ));
    }
    if failed > 0 || (check && !changed.is_empty()) {
        Exit::User
    } else {
        Exit::Ok
    }
}

fn project_files(config: Option<&Path>) -> Result<Vec<PathBuf>> {
    let project = pipeline::load_project(config)?;
    let mut files = Vec::new();
    for pkg in &project.packages {
        for (rel, _) in veld_project::config::collect_sources(pkg)? {
            files.push(pkg.root.join(rel));
        }
    }
    Ok(files)
}

fn explicit_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            walk(p, &mut files)?;
        } else if p.is_file() {
            files.push(p.clone());
        } else {
            return Err(user(format!("`{}` does not exist", p.display())));
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read `{}`", dir.display()))? {
        let path = entry?.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if path.is_dir() {
            if name != "node_modules" && !name.starts_with('.') {
                walk(&path, out)?;
            }
        } else if path.extension().is_some_and(|e| e == "veld") {
            out.push(path);
        }
    }
    Ok(())
}

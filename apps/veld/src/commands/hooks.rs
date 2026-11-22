//! `[hooks] post-generate`: shell commands run after a successful, non-dry-run generate.

use std::process::Command;

use anyhow::Result;
use veld_project::config::Project;

use crate::error::user;
use crate::ui::Ui;

/// Runs every `post-generate` hook in order, from the project root, with its output streamed.
/// A failing hook is a warning, unless `[hooks] fail = true`, which makes it an error.
pub fn post_generate(ui: &Ui, project: &Project) -> Result<()> {
    for cmd in &project.hooks.post_generate {
        ui.info(&format!(
            "{} {cmd}",
            ui.out_styled(crate::ui::Style::Dim, "hook:")
        ));
        let started = std::time::Instant::now();
        let status = shell(cmd).current_dir(&project.root).status();
        ui.debug(
            1,
            &format!(
                "hook `{cmd}` took {}",
                crate::ui::render::elapsed(started.elapsed())
            ),
        );
        let problem = match status {
            Ok(s) if s.success() => continue,
            Ok(s) => format!("hook `{cmd}` failed ({s})"),
            Err(e) => format!("cannot run hook `{cmd}`: {e}"),
        };
        if project.hooks.fail {
            return Err(user(problem));
        }
        ui.warn(&problem);
    }
    Ok(())
}

#[cfg(windows)]
fn shell(cmd: &str) -> Command {
    let mut c = Command::new("cmd");
    c.args(["/C", cmd]);
    c
}

#[cfg(not(windows))]
fn shell(cmd: &str) -> Command {
    let mut c = Command::new("sh");
    c.args(["-c", cmd]);
    c
}

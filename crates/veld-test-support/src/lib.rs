//! Spawn test servers so that they can never leak.
//!
//! A server started by a test is often a launcher (`java`, `npx`, `npm.cmd`) that starts the real
//! process; killing only the launcher orphans the rest, and an orphan keeps the test's stdout and
//! stderr pipes open, which makes a test run look hung. [`ProcessTree`] owns the child and, when
//! dropped, kills the whole tree: `taskkill /T /F` on Windows, the whole process group on Unix.
//! Spawned servers get a null stdin and either a log file or a null device as output, never the
//! test's own pipes (see [`ProcessTree::spawn_logged`]); [`isolate`] is for callers that need
//! their own pipes.
#![allow(clippy::missing_panics_doc)]

use std::fs::File;
use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Puts the command in its own process group (Unix) or console process group (Windows), so the
/// whole tree can be killed later. [`ProcessTree::spawn_logged`] calls this for you.
pub fn isolate(cmd: &mut Command) -> &mut Command {
    #[cfg(unix)]
    cmd.process_group(0);
    #[cfg(windows)]
    cmd.creation_flags(0x0000_0200); // CREATE_NEW_PROCESS_GROUP
    cmd
}

/// A child process whose whole process tree is killed when this is dropped.
#[derive(Debug)]
pub struct ProcessTree {
    child: Child,
}

impl ProcessTree {
    /// Spawns `cmd` with a null stdin and stdout and stderr appended to `log`.
    pub fn spawn_logged(cmd: &mut Command, log: &Path) -> io::Result<Self> {
        let file = File::create(log)?;
        cmd.stdin(Stdio::null())
            .stdout(Stdio::from(file.try_clone()?))
            .stderr(Stdio::from(file));
        Self::spawn(cmd)
    }

    /// Spawns `cmd` with null stdin, stdout and stderr.
    pub fn spawn_silent(cmd: &mut Command) -> io::Result<Self> {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        Self::spawn(cmd)
    }

    /// Spawns `cmd` exactly as configured by the caller (which then owns the stdio choice; use
    /// pipes you drain or files, never the inherited ones) and isolates it for tree kill.
    pub fn spawn(cmd: &mut Command) -> io::Result<Self> {
        isolate(cmd);
        Ok(Self::adopt(cmd.spawn()?))
    }

    /// Takes over a child that was spawned with [`isolate`] (otherwise only the child itself, and
    /// on Windows its descendants, are killed).
    #[must_use]
    pub fn adopt(child: Child) -> Self {
        Self { child }
    }

    /// The process id of the child (the tree's root).
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// The underlying child, for `try_wait` and taking pipes.
    pub fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    /// Kills the whole tree and reaps the child. Safe to call twice.
    pub fn kill_tree(&mut self) {
        kill_tree_of(self.child.id());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        self.kill_tree();
    }
}

#[cfg(windows)]
fn kill_tree_of(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .output();
}

#[cfg(unix)]
fn kill_tree_of(pid: u32) {
    // The child leads its own process group (see `isolate`): signal the whole group. The shell
    // builtin is used because a slim image may have no `kill` executable (procps).
    let _ = Command::new("sh")
        .args(["-c", &format!("kill -s KILL -- -{pid}")])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
}

/// Whether a process with this id is alive. A zombie (dead, not yet reaped) is not alive: after
/// its parent is killed a grandchild can stay a zombie for as long as its new parent, such as
/// PID 1 of a container, does not reap it, and `kill -0` still succeeds for it.
#[must_use]
pub fn pid_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .stdin(Stdio::null())
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
    }
    #[cfg(unix)]
    {
        unix_state(pid).is_some_and(|state| !state.starts_with('Z'))
    }
}

/// The scheduler state of a process (`R`, `S`, `Z`, ...), or `None` when there is no such process.
#[cfg(target_os = "linux")]
fn unix_state(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // `pid (comm) S ...`: the name may contain spaces and parentheses, so cut at the last `)`.
    let rest = stat.get(stat.rfind(')')? + 1..)?;
    rest.split_whitespace().next().map(str::to_string)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn unix_state(pid: u32) -> Option<String> {
    let out = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let state = text.trim();
    (out.status.success() && !state.is_empty()).then(|| state.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    const PARENT: &str = r"
const { spawn } = require('child_process');
const fs = require('fs');
const kid = spawn(process.execPath, ['-e', 'setInterval(()=>{},1000)'], { stdio: 'ignore' });
fs.writeFileSync(process.argv[1], String(kid.pid));
setInterval(() => {}, 1000);
";

    fn node_present() -> bool {
        Command::new("node")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    fn wait_for(what: &str, ok: impl Fn() -> bool) {
        let end = Instant::now() + Duration::from_secs(30);
        while Instant::now() < end {
            if ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    fn dropping_kills_the_child_and_its_descendants() {
        if !node_present() {
            assert!(
                std::env::var_os("VELD_REQUIRE_TOOLCHAINS").is_none(),
                "node is required"
            );
            eprintln!("skipped: node not found");
            return;
        }
        let dir = std::env::temp_dir().join(format!("veld-ts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pidfile = dir.join("grandchild.pid");
        let mut cmd = Command::new("node");
        cmd.args(["-e", PARENT]).arg(&pidfile);
        let tree = ProcessTree::spawn_logged(&mut cmd, &dir.join("out.log")).unwrap();
        let parent = tree.id();
        wait_for("the grandchild", || pidfile.is_file());
        wait_for("the grandchild pid", || {
            std::fs::read_to_string(&pidfile).is_ok_and(|t| t.trim().parse::<u32>().is_ok())
        });
        let grandchild: u32 = std::fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // The pid file proves the grandchild was spawned, not that the OS lists it yet: wait.
        wait_for("the parent and the grandchild to be alive", || {
            pid_alive(parent) && pid_alive(grandchild)
        });

        drop(tree);

        wait_for("the whole tree to be gone", || {
            !pid_alive(parent) && !pid_alive(grandchild)
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn this_process_is_alive() {
        assert!(pid_alive(std::process::id()));
    }

    /// A child that exited but was not reaped is a zombie: it is not alive.
    #[cfg(unix)]
    #[test]
    fn an_unreaped_exited_child_is_not_alive() {
        let mut child = Command::new("sh")
            .args(["-c", "exit 0"])
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        wait_for("the child to be reported dead", || !pid_alive(child.id()));
        let _ = child.wait();
    }
}

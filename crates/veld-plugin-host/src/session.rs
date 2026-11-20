//! One plugin process: spawn, JSON lines in and out, timeouts, kill.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;

use crate::command::CommandLine;
use crate::error::PluginError;

/// Upper bound on everything a plugin may write to stdout (a 200 MiB payload with JSON escaping).
const MAX_STDOUT_BYTES: u64 = 640 * 1024 * 1024;

#[derive(Debug)]
enum Event {
    Line(String),
    Eof,
    Bad(String),
}

/// A running plugin. Dropping it kills the process.
#[derive(Debug)]
pub struct Session {
    name: String,
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Event>,
    /// The plugin's working directory; removed when the session ends.
    _cwd: TempDir,
}

impl Session {
    /// Starts `exe` with a fresh empty working directory and an environment holding only `PATH`.
    pub fn spawn(name: &str, exe: &Path, cmd: &CommandLine) -> Result<Self, PluginError> {
        let cwd = tempfile::tempdir().map_err(|e| PluginError::process(name, format!("cannot create a work directory: {e}")))?;
        let mut command = Command::new(exe);
        command.args(&cmd.args).current_dir(cwd.path()).env_clear();
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|e| PluginError::process(name, format!("cannot start `{}`: {e}", exe.display())))?;
        let stdin = child.stdin.take();
        let lines = spawn_readers(name, &mut child);
        Ok(Self { name: name.to_string(), child, stdin, lines, _cwd: cwd })
    }

    /// Sends one JSON message as a single line.
    pub fn send(&mut self, message: &Value) -> Result<(), PluginError> {
        let mut text = serde_json::to_string(message)
            .map_err(|e| PluginError::protocol(&self.name, format!("cannot encode a message: {e}")))?;
        text.push('\n');
        let name = self.name.clone();
        let stdin = self.stdin.as_mut().ok_or_else(|| PluginError::process(&name, "its input is already closed"))?;
        stdin
            .write_all(text.as_bytes())
            .and_then(|()| stdin.flush())
            .map_err(|e| PluginError::process(&name, format!("cannot write to the plugin (did it exit?): {e}")))
    }

    /// Waits up to `timeout` for the next message. On timeout the plugin is killed.
    pub fn recv(&mut self, timeout: Duration, what: &str) -> Result<Value, PluginError> {
        match self.lines.recv_timeout(timeout) {
            Ok(Event::Line(line)) => serde_json::from_str(&line)
                .map_err(|e| PluginError::protocol(&self.name, format!("{what} is not valid JSON: {e}"))),
            Ok(Event::Bad(why)) => Err(self.fail(PluginError::protocol(&self.name, format!("{what}: {why}")))),
            Ok(Event::Eof) | Err(RecvTimeoutError::Disconnected) => Err(self.closed(what)),
            Err(RecvTimeoutError::Timeout) => {
                let msg = format!("did not send {what} within {}; killed", fmt_duration(timeout));
                Err(self.fail(PluginError::process(&self.name, msg)))
            }
        }
    }

    /// Stdout closed before a message arrived: report the exit code if the process is gone.
    fn closed(&mut self, what: &str) -> PluginError {
        let status = self.wait_for_exit(Duration::from_secs(2));
        let how = match status {
            Some(s) => format!("exited ({}) before sending {what}", describe(&s)),
            None => format!("closed its output before sending {what}"),
        };
        self.fail(PluginError::process(&self.name, how))
    }

    fn fail(&mut self, e: PluginError) -> PluginError {
        self.kill();
        e
    }

    pub fn kill(&mut self) {
        self.stdin = None;
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Closes the plugin's input and waits up to `grace` for it to exit; kills it after that.
    /// Returns the exit status when the plugin exited on its own.
    pub fn finish(&mut self, grace: Duration) -> Option<ExitStatus> {
        self.stdin = None;
        let status = self.wait_for_exit(grace);
        if status.is_none() {
            self.kill();
        }
        status
    }

    fn wait_for_exit(&mut self, grace: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + grace;
        loop {
            match self.child.try_wait() {
                Ok(Some(s)) => return Some(s),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                _ => return None,
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.kill();
    }
}

fn fmt_duration(d: Duration) -> String {
    if d.as_secs() >= 1 { format!("{} s", d.as_secs()) } else { format!("{} ms", d.as_millis()) }
}

/// `"exit code 3"`, or `"killed by a signal"`.
pub fn describe(status: &ExitStatus) -> String {
    match status.code() {
        Some(c) => format!("exit code {c}"),
        None => "killed by a signal".to_string(),
    }
}

/// Reader threads: stdout lines go to the channel, stderr lines are forwarded with a prefix.
/// They end by themselves when the process closes its pipes, so they are never joined.
fn spawn_readers(name: &str, child: &mut Child) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    if let Some(out) = child.stdout.take() {
        thread::spawn(move || read_lines(out, &tx));
    }
    if let Some(err) = child.stderr.take() {
        let prefix = format!("[plugin:{name}]");
        thread::spawn(move || {
            for line in BufReader::new(err.take(MAX_STDOUT_BYTES)).lines().map_while(Result::ok) {
                eprintln!("{prefix} {line}");
            }
        });
    }
    rx
}

fn read_lines(out: impl Read, tx: &mpsc::Sender<Event>) {
    let mut reader = BufReader::new(out.take(MAX_STDOUT_BYTES));
    loop {
        let mut buf = Vec::new();
        let event = match reader.read_until(b'\n', &mut buf) {
            Ok(0) => Event::Eof,
            Ok(_) => match String::from_utf8(buf) {
                Ok(line) if line.trim().is_empty() => continue,
                Ok(line) => Event::Line(line),
                Err(_) => Event::Bad("output is not valid UTF-8".into()),
            },
            Err(e) => Event::Bad(format!("cannot read output: {e}")),
        };
        let stop = !matches!(event, Event::Line(_));
        if tx.send(event).is_err() || stop {
            return;
        }
    }
}

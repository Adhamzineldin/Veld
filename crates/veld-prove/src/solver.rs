//! Finding a solver, running it on a batch of questions and reading its answers.
//!
//! One solver process answers a whole batch: the script has one `(push 1)` ... `(pop 1)` block
//! per question, each opened by `(echo "Q<i>")`. The exit code is not looked at: Z3 exits with 1
//! when a command it was given has no answer (for example `(get-value ...)` after `unsat`).

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::sexp::{Sexp, parse_all};

/// The random seed every solver is started with.
pub const SEED: u32 = 0;

/// Time added to the whole batch on top of the limit of each question, for starting the solver.
const GRACE: Duration = Duration::from_secs(20);

/// Which command-line and options the solver understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    Z3,
    Cvc5,
}

/// A solver program that was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solver {
    pub program: PathBuf,
    pub flavour: Flavour,
}

/// Why no solver could be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotFound {
    #[error("the solver `{0}` does not exist")]
    Missing(String),
    #[error(
        "no SMT solver found: pass --solver <path>, set VELD_SOLVER, or put z3 or cvc5 on PATH"
    )]
    None,
}

fn flavour_of(program: &Path) -> Flavour {
    let stem = program
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if stem.contains("cvc5") {
        Flavour::Cvc5
    } else {
        Flavour::Z3
    }
}

fn on_path(dirs: &OsString, name: &str) -> Option<PathBuf> {
    for dir in std::env::split_paths(dirs) {
        for file in [name.to_owned(), format!("{name}.exe")] {
            let candidate = dir.join(&file);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Finds the solver: `explicit` (`--solver`), then `env` (`VELD_SOLVER`), then `z3` and `cvc5`
/// on `path`. The three are passed in so that a test can say what it wants.
///
/// # Errors
/// [`NotFound`] when a named solver does not exist or none is on the path.
pub fn find(
    explicit: Option<&Path>,
    env: Option<&OsString>,
    path: Option<&OsString>,
) -> Result<Solver, NotFound> {
    let named = explicit
        .map(Path::to_path_buf)
        .or_else(|| env.filter(|v| !v.is_empty()).map(PathBuf::from));
    if let Some(program) = named {
        if !program.is_file() {
            return Err(NotFound::Missing(program.display().to_string()));
        }
        return Ok(Solver {
            flavour: flavour_of(&program),
            program,
        });
    }
    let dirs = path.ok_or(NotFound::None)?;
    for name in ["z3", "cvc5"] {
        if let Some(program) = on_path(dirs, name) {
            return Ok(Solver {
                flavour: flavour_of(&program),
                program,
            });
        }
    }
    Err(NotFound::None)
}

/// [`find`] with the process environment.
///
/// # Errors
/// As [`find`].
pub fn find_in_environment(explicit: Option<&Path>) -> Result<Solver, NotFound> {
    find(
        explicit,
        std::env::var_os("VELD_SOLVER").as_ref(),
        std::env::var_os("PATH").as_ref(),
    )
}

/// What a run of the solver produced.
#[derive(Debug, Clone, Default)]
pub struct RunOutput {
    pub stdout: String,
    /// The process was killed because the whole batch took too long.
    pub killed: bool,
    /// The solver could not be started.
    pub failed: Option<String>,
}

/// Something that runs a script. [`Solver`] starts a process; a test can answer from a text.
pub trait Backend {
    fn flavour(&self) -> Flavour;
    fn run(&self, script: &str, questions: usize, per_question: Duration) -> RunOutput;
}

impl Backend for Solver {
    fn flavour(&self) -> Flavour {
        self.flavour
    }

    fn run(&self, script: &str, questions: usize, per_question: Duration) -> RunOutput {
        let budget = per_question
            .saturating_mul(u32::try_from(questions.max(1)).unwrap_or(u32::MAX))
            .saturating_add(GRACE);
        run_process(self, script, budget)
    }
}

fn run_process(solver: &Solver, script: &str, budget: Duration) -> RunOutput {
    let mut cmd = Command::new(&solver.program);
    match solver.flavour {
        Flavour::Z3 => cmd.args(["-in", "-smt2"]),
        Flavour::Cvc5 => cmd.args(["--lang", "smt2", "--incremental"]),
    };
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return RunOutput {
                failed: Some(format!("cannot start {}: {e}", solver.program.display())),
                ..RunOutput::default()
            };
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let text = script.to_owned();
        thread::spawn(move || {
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    let sink = Arc::new(Mutex::new(Vec::<u8>::new()));
    if let Some(mut out) = child.stdout.take() {
        let sink = Arc::clone(&sink);
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = out.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if let Ok(mut s) = sink.lock() {
                    s.extend_from_slice(&buf[..n]);
                }
            }
        });
    }
    let started = Instant::now();
    let mut killed = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) if started.elapsed() > budget => {
                let _ = child.kill();
                killed = true;
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
        }
    }
    if !killed {
        // The reader ends with the pipe; give it a moment to deliver the last bytes.
        thread::sleep(Duration::from_millis(50));
    }
    let bytes = sink.lock().map(|s| s.clone()).unwrap_or_default();
    RunOutput {
        stdout: String::from_utf8_lossy(&bytes).into_owned(),
        killed,
        failed: None,
    }
}

/// The options and declarations that open every script.
#[must_use]
pub fn preamble(flavour: Flavour, per_question: Duration) -> String {
    let ms = per_question.as_millis();
    let mut out =
        String::from("(set-option :produce-models true)\n(set-option :produce-unsat-cores true)\n");
    match flavour {
        Flavour::Z3 => {
            out.push_str(&format!("(set-option :timeout {ms})\n"));
            out.push_str(&format!("(set-option :random-seed {SEED})\n"));
        }
        Flavour::Cvc5 => {
            out.push_str(&format!("(set-option :tlimit-per {ms})\n"));
            out.push_str(&format!("(set-option :seed {SEED})\n"));
        }
    }
    out.push_str("(set-logic ALL)\n(get-info :name)\n(get-info :version)\n");
    out.push_str("(declare-sort Str 0)\n(declare-sort Uuid 0)\n(declare-fun slen (Str) Int)\n");
    out
}

/// What the solver said about one question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// `sat`, with the values asked for, in the order of the terms.
    Sat(Vec<Sexp>),
    /// `unsat`, with the names of the assertions of the unsatisfiable core.
    Unsat(Vec<String>),
    /// `unknown`, a timeout, or no answer.
    Unknown(String),
}

/// The name and version the solver reported about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverInfo {
    pub name: String,
    pub version: String,
}

/// Everything read from the output of a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answers {
    pub info: Option<SolverInfo>,
    pub verdicts: Vec<Verdict>,
}

fn info_of(head: &str) -> Option<SolverInfo> {
    let (mut name, mut version) = (None, None);
    for e in parse_all(head) {
        if let Some([Sexp::Atom(key), Sexp::Str(v)]) = e.list() {
            match key.as_str() {
                ":name" => name = Some(v.clone()),
                ":version" => version = Some(v.clone()),
                _ => {}
            }
        }
    }
    Some(SolverInfo {
        name: name?,
        version: version.unwrap_or_default(),
    })
}

/// The text of each block `Q<i>` of the output, and what came before the first.
fn blocks(stdout: &str, count: usize) -> (String, Vec<String>) {
    let mut head = String::new();
    let mut out: Vec<String> = vec![String::new(); count];
    let mut current: Option<usize> = None;
    for line in stdout.lines() {
        let marker = line
            .trim()
            .strip_prefix('Q')
            .and_then(|n| n.parse::<usize>().ok())
            .filter(|n| *n < count);
        match (marker, current) {
            (Some(n), _) => current = Some(n),
            (None, Some(i)) => {
                if let Some(b) = out.get_mut(i) {
                    b.push_str(line);
                    b.push('\n');
                }
            }
            (None, None) => {
                head.push_str(line);
                head.push('\n');
            }
        }
    }
    (head, out)
}

fn verdict_of(block: &str) -> Verdict {
    let exprs: Vec<Sexp> = parse_all(block)
        .into_iter()
        .filter(|e| !e.is_error())
        .collect();
    let Some(first) = exprs.first().and_then(Sexp::atom) else {
        return Verdict::Unknown("the solver gave no answer".to_owned());
    };
    let list = exprs.get(1).and_then(Sexp::list);
    match first {
        "sat" => {
            let values = list
                .unwrap_or_default()
                .iter()
                .filter_map(|pair| pair.list().and_then(|p| p.get(1)).cloned())
                .collect();
            Verdict::Sat(values)
        }
        "unsat" => Verdict::Unsat(
            list.unwrap_or_default()
                .iter()
                .filter_map(|a| a.atom().map(str::to_owned))
                .collect(),
        ),
        "unknown" | "timeout" => {
            Verdict::Unknown("the solver gave up (unknown or time limit)".to_owned())
        }
        other => Verdict::Unknown(format!("the solver answered `{other}`")),
    }
}

/// Reads the output of a batch of `count` questions.
#[must_use]
pub fn parse_answers(stdout: &str, count: usize) -> Answers {
    let (head, blocks) = blocks(stdout, count);
    Answers {
        info: info_of(&head),
        verdicts: blocks.iter().map(|b| verdict_of(b)).collect(),
    }
}

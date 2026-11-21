//! Interactive prompts behind one trait, so the flows are testable.
//!
//! [`Clack`] is the real terminal experience (arrow keys, `cliclack`). [`Lines`] is a plain
//! line-based fallback for dumb terminals and the scripted driver of the tests.

use std::io::{BufRead, Write};

use anyhow::Result;

use super::render;
use super::theme::{Style, Theme};

/// The user pressed Ctrl-C (or Esc), or the input ended.
#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for Cancelled {}

#[must_use]
pub fn is_cancelled(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Cancelled>().is_some()
}

#[must_use]
pub fn cancelled() -> anyhow::Error {
    anyhow::Error::new(Cancelled)
}

/// One option of a select or multiselect.
#[derive(Debug, Clone)]
pub struct Choice {
    pub label: String,
    pub hint: String,
}

impl Choice {
    #[must_use]
    pub fn new(label: &str, hint: &str) -> Self {
        Self {
            label: label.to_string(),
            hint: hint.to_string(),
        }
    }
}

/// A text question. An empty answer means `default` (when it is not empty).
pub struct Text<'a> {
    pub label: &'a str,
    pub default: &'a str,
    pub placeholder: &'a str,
    pub required: bool,
    /// `Err(message)` rejects the answer and asks again.
    pub validate: fn(&str) -> std::result::Result<(), String>,
}

pub trait Prompter {
    fn intro(&mut self, title: &str) -> Result<()>;
    fn text(&mut self, q: &Text<'_>) -> Result<String>;
    fn select(&mut self, label: &str, items: &[Choice], initial: usize) -> Result<usize>;
    fn multiselect(
        &mut self,
        label: &str,
        items: &[Choice],
        initial: &[usize],
    ) -> Result<Vec<usize>>;
    fn confirm(&mut self, label: &str, default: bool) -> Result<bool>;
    fn note(&mut self, title: &str, body: &[String]) -> Result<()>;
    /// A long step starts (a spinner on a terminal).
    fn working(&mut self, message: &str);
    /// The step started by [`Prompter::working`] ended.
    fn done(&mut self, message: &str);
    fn outro(&mut self, message: &str) -> Result<()>;
    fn cancel(&mut self, message: &str) -> Result<()>;
}

fn io_err(e: std::io::Error) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::Interrupted {
        cancelled()
    } else {
        anyhow::Error::new(e).context("cannot use the terminal")
    }
}

/// The arrow-key terminal UI.
#[derive(Default)]
pub struct Clack {
    spinner: Option<cliclack::ProgressBar>,
}

impl Prompter for Clack {
    fn intro(&mut self, title: &str) -> Result<()> {
        cliclack::intro(title).map_err(io_err)
    }

    fn text(&mut self, q: &Text<'_>) -> Result<String> {
        let mut input = cliclack::input(q.label).required(q.required && q.default.is_empty());
        if !q.placeholder.is_empty() {
            input = input.placeholder(q.placeholder);
        }
        if !q.default.is_empty() {
            input = input.default_input(q.default);
        }
        let validate = q.validate;
        input
            .validate(move |s: &String| validate(s))
            .interact::<String>()
            .map_err(io_err)
    }

    fn select(&mut self, label: &str, items: &[Choice], initial: usize) -> Result<usize> {
        let mut s = cliclack::select::<usize>(label);
        for (i, c) in items.iter().enumerate() {
            s = s.item(i, &c.label, &c.hint);
        }
        s.initial_value(initial).interact().map_err(io_err)
    }

    fn multiselect(
        &mut self,
        label: &str,
        items: &[Choice],
        initial: &[usize],
    ) -> Result<Vec<usize>> {
        let mut s = cliclack::multiselect::<usize>(label).required(false);
        for (i, c) in items.iter().enumerate() {
            s = s.item(i, &c.label, &c.hint);
        }
        s.initial_values(initial.to_vec())
            .interact()
            .map_err(io_err)
    }

    fn confirm(&mut self, label: &str, default: bool) -> Result<bool> {
        cliclack::confirm(label)
            .initial_value(default)
            .interact()
            .map_err(io_err)
    }

    fn note(&mut self, title: &str, body: &[String]) -> Result<()> {
        cliclack::note(title, body.join("\n")).map_err(io_err)
    }

    fn working(&mut self, message: &str) {
        let s = cliclack::spinner();
        s.start(message);
        self.spinner = Some(s);
    }

    fn done(&mut self, message: &str) {
        if let Some(s) = self.spinner.take() {
            s.stop(message);
        }
    }

    fn outro(&mut self, message: &str) -> Result<()> {
        cliclack::outro(message).map_err(io_err)
    }

    fn cancel(&mut self, message: &str) -> Result<()> {
        cliclack::outro_cancel(message).map_err(io_err)
    }
}

/// Line-based prompts: numbers pick from a list, Enter takes the default, end of input
/// cancels. With `echo` the answers are repeated in the output (for scripted input).
pub struct Lines<R, W> {
    input: R,
    out: W,
    theme: Theme,
    echo: bool,
}

impl<R: BufRead, W: Write> Lines<R, W> {
    pub fn new(input: R, out: W, theme: Theme, echo: bool) -> Self {
        Self {
            input,
            out,
            theme,
            echo,
        }
    }

    #[cfg(test)]
    pub fn into_output(self) -> W {
        self.out
    }

    fn say(&mut self, line: &str) -> Result<()> {
        writeln!(self.out, "{line}").map_err(io_err)
    }

    fn rail(&self) -> String {
        self.theme.paint(Style::Dim, self.theme.sym().rail)
    }

    fn head(&mut self, label: &str) -> Result<()> {
        let t = self.theme;
        self.say(&format!(
            "{}  {}",
            t.paint(Style::Cyan, t.sym().active),
            t.paint(Style::Bold, label)
        ))
    }

    /// Prints the prompt line and reads the answer (trimmed). Ends the run on EOF.
    fn read(&mut self, prompt: &str) -> Result<String> {
        write!(self.out, "{}  {prompt}", self.rail()).map_err(io_err)?;
        self.out.flush().map_err(io_err)?;
        let mut line = String::new();
        let n = self.input.read_line(&mut line).map_err(io_err)?;
        if n == 0 {
            return Err(cancelled());
        }
        let line = line.trim().to_string();
        if self.echo {
            writeln!(self.out, "{line}").map_err(io_err)?;
        }
        Ok(line)
    }

    fn note_line(&mut self, text: &str) -> Result<()> {
        let rail = self.rail();
        self.say(&format!("{rail}  {text}"))
    }

    fn answered(&mut self, label: &str, value: &str) -> Result<()> {
        let t = self.theme;
        self.say(&format!(
            "{}  {}  {}",
            t.paint(Style::Green, t.sym().done),
            t.paint(Style::Dim, label),
            value
        ))?;
        let rail = self.rail();
        self.say(&rail)
    }

    fn list(&mut self, items: &[Choice], marks: &dyn Fn(usize) -> bool) -> Result<()> {
        let t = self.theme;
        for (i, c) in items.iter().enumerate() {
            let mark = if marks(i) { t.sym().ok } else { " " };
            let hint = if c.hint.is_empty() {
                String::new()
            } else {
                format!("  {}", t.paint(Style::Dim, &c.hint))
            };
            self.note_line(&format!("{mark} {}) {}{hint}", i + 1, c.label))?;
        }
        Ok(())
    }
}

fn index(text: &str, n: usize) -> Option<usize> {
    text.parse::<usize>()
        .ok()
        .filter(|i| (1..=n).contains(i))
        .map(|i| i - 1)
}

impl<R: BufRead, W: Write> Prompter for Lines<R, W> {
    fn intro(&mut self, title: &str) -> Result<()> {
        let t = self.theme;
        self.say(&format!(
            "{}  {}",
            t.paint(Style::Dim, t.sym().top),
            t.paint(Style::Brand, title)
        ))?;
        let rail = self.rail();
        self.say(&rail)
    }

    fn text(&mut self, q: &Text<'_>) -> Result<String> {
        self.head(q.label)?;
        loop {
            let hint = if !q.default.is_empty() {
                format!("[{}] ", q.default)
            } else if !q.placeholder.is_empty() {
                format!("({}) ", q.placeholder)
            } else {
                String::new()
            };
            let mut ans = self.read(&format!("> {hint}"))?;
            if ans.is_empty() {
                ans = q.default.to_string();
            }
            if ans.is_empty() && q.required {
                self.note_line("required")?;
                continue;
            }
            match (q.validate)(&ans) {
                Ok(()) => {
                    self.answered(q.label, &ans)?;
                    return Ok(ans);
                }
                Err(msg) => {
                    let red = self.theme.paint(Style::Red, &msg);
                    self.note_line(&red)?;
                }
            }
        }
    }

    fn select(&mut self, label: &str, items: &[Choice], initial: usize) -> Result<usize> {
        self.head(label)?;
        self.list(items, &|i| i == initial)?;
        loop {
            let ans = self.read(&format!("choose [{}] ", initial + 1))?;
            let pick = if ans.is_empty() {
                Some(initial)
            } else {
                index(&ans, items.len())
            };
            if let Some(i) = pick {
                self.answered(label, &items[i].label)?;
                return Ok(i);
            }
            self.note_line(&format!("enter a number from 1 to {}", items.len()))?;
        }
    }

    fn multiselect(
        &mut self,
        label: &str,
        items: &[Choice],
        initial: &[usize],
    ) -> Result<Vec<usize>> {
        self.head(label)?;
        self.list(items, &|i| initial.contains(&i))?;
        let default: Vec<String> = initial.iter().map(|i| (i + 1).to_string()).collect();
        loop {
            let ans = self.read(&format!(
                "numbers, comma-separated, `-` for none [{}] ",
                default.join(",")
            ))?;
            let picked: Option<Vec<usize>> = if ans.is_empty() {
                Some(initial.to_vec())
            } else if ans == "-" {
                Some(Vec::new())
            } else {
                ans.split(',')
                    .map(|p| index(p.trim(), items.len()))
                    .collect()
            };
            if let Some(mut p) = picked {
                p.sort_unstable();
                p.dedup();
                let names: Vec<&str> = p.iter().map(|i| items[*i].label.as_str()).collect();
                let shown = if names.is_empty() {
                    "none".to_string()
                } else {
                    names.join(", ")
                };
                self.answered(label, &shown)?;
                return Ok(p);
            }
            self.note_line(&format!("enter numbers from 1 to {}", items.len()))?;
        }
    }

    fn confirm(&mut self, label: &str, default: bool) -> Result<bool> {
        self.head(label)?;
        let hint = if default { "Y/n" } else { "y/N" };
        loop {
            let ans = self.read(&format!("[{hint}] "))?.to_ascii_lowercase();
            let v = match ans.as_str() {
                "" => Some(default),
                "y" | "yes" => Some(true),
                "n" | "no" => Some(false),
                _ => None,
            };
            if let Some(v) = v {
                self.answered(label, if v { "yes" } else { "no" })?;
                return Ok(v);
            }
            self.note_line("answer y or n")?;
        }
    }

    fn note(&mut self, title: &str, body: &[String]) -> Result<()> {
        for l in render::note_box(&self.theme, title, body) {
            self.say(&l)?;
        }
        Ok(())
    }

    fn working(&mut self, message: &str) {
        let t = self.theme;
        let _ = writeln!(
            self.out,
            "{}  {message}...",
            t.paint(Style::Cyan, t.sym().active)
        );
    }

    fn done(&mut self, message: &str) {
        let t = self.theme;
        let _ = writeln!(self.out, "{}  {message}", t.paint(Style::Green, t.sym().ok));
        let _ = writeln!(self.out, "{}", self.rail());
    }

    fn outro(&mut self, message: &str) -> Result<()> {
        let t = self.theme;
        self.say(&format!(
            "{}  {message}",
            t.paint(Style::Dim, t.sym().bottom)
        ))
    }

    fn cancel(&mut self, message: &str) -> Result<()> {
        let t = self.theme;
        self.say(&format!(
            "{}  {}",
            t.paint(Style::Dim, t.sym().bottom),
            t.paint(Style::Red, message)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(script: &str) -> Lines<&[u8], Vec<u8>> {
        Lines::new(script.as_bytes(), Vec::new(), Theme::PLAIN, true)
    }

    fn out(l: Lines<&[u8], Vec<u8>>) -> String {
        String::from_utf8(l.into_output()).unwrap_or_default()
    }

    #[test]
    fn select_takes_default_and_numbers_and_rejects_junk() {
        let items = [Choice::new("a", ""), Choice::new("b", "hint")];
        assert_eq!(lines("\n").select("Pick", &items, 1).unwrap(), 1);
        assert_eq!(lines("1\n").select("Pick", &items, 1).unwrap(), 0);
        let mut l = lines("9\nx\n2\n");
        assert_eq!(l.select("Pick", &items, 0).unwrap(), 1);
        assert!(out(l).contains("enter a number from 1 to 2"));
    }

    #[test]
    fn text_validates_and_asks_again() {
        fn no_x(s: &str) -> std::result::Result<(), String> {
            if s.contains('x') {
                Err("no x".into())
            } else {
                Ok(())
            }
        }
        let q = Text {
            label: "Name",
            default: "dflt",
            placeholder: "",
            required: true,
            validate: no_x,
        };
        let mut l = lines("xx\nok\n");
        assert_eq!(l.text(&q).unwrap(), "ok");
        assert!(out(l).contains("no x"));
        assert_eq!(lines("\n").text(&q).unwrap(), "dflt");
    }

    #[test]
    fn multiselect_confirm_and_eof() {
        let items = [Choice::new("a", ""), Choice::new("b", "")];
        assert_eq!(
            lines("2,1\n").multiselect("M", &items, &[0]).unwrap(),
            [0, 1]
        );
        assert_eq!(
            lines("-\n").multiselect("M", &items, &[0]).unwrap(),
            Vec::<usize>::new()
        );
        assert!(lines("maybe\nno\n").confirm("Ok?", true).is_ok_and(|v| !v));
        assert!(is_cancelled(&lines("").confirm("Ok?", true).unwrap_err()));
    }
}

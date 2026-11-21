//! All terminal output of the CLI: palette and symbols ([`theme`]), pure renderers
//! ([`render`]), the spinner ([`spinner`]) and the interactive prompts ([`prompt`]).
//! Commands never write escape codes themselves.

pub mod prompt;
pub mod render;
pub mod spinner;
pub mod theme;

use std::io::IsTerminal;

pub use theme::{Style, Theme};

/// Output settings resolved once from the global flags and the environment.
#[derive(Debug, Clone, Copy)]
pub struct Ui {
    pub quiet: bool,
    /// `-v` count: 1 shows phases and timings, 2 also write-plan entries.
    pub verbose: u8,
    color_out: bool,
    color_err: bool,
    unicode: bool,
    animate: bool,
}

impl Ui {
    /// Colour is on only for a terminal, never with `--no-color` or a non-empty `NO_COLOR`.
    pub fn new(no_color: bool, quiet: bool, verbose: u8) -> Self {
        let allowed = theme::color_allowed(no_color);
        if !allowed {
            console::set_colors_enabled(false);
            console::set_colors_enabled_stderr(false);
        }
        let err_tty = std::io::stderr().is_terminal();
        Ui {
            verbose,
            quiet,
            color_out: allowed && std::io::stdout().is_terminal(),
            color_err: allowed && err_tty,
            unicode: theme::detect_unicode(),
            animate: err_tty && !quiet,
        }
    }

    #[cfg(test)]
    pub fn plain() -> Self {
        Ui {
            verbose: 0,
            quiet: false,
            color_out: false,
            color_err: false,
            unicode: true,
            animate: false,
        }
    }

    #[cfg(test)]
    pub fn colored() -> Self {
        Ui {
            verbose: 0,
            quiet: false,
            color_out: true,
            color_err: true,
            unicode: true,
            animate: false,
        }
    }

    #[must_use]
    pub fn theme_out(&self) -> Theme {
        Theme {
            color: self.color_out,
            unicode: self.unicode,
        }
    }

    #[must_use]
    pub fn theme_err(&self) -> Theme {
        Theme {
            color: self.color_err,
            unicode: self.unicode,
        }
    }

    /// Whether prompts may be shown: a real terminal on stdin, stdout and stderr.
    #[must_use]
    pub fn interactive() -> bool {
        std::io::stdin().is_terminal()
            && std::io::stdout().is_terminal()
            && std::io::stderr().is_terminal()
    }

    /// Styled text for stdout.
    pub fn out_styled(&self, style: Style, text: &str) -> String {
        self.theme_out().paint(style, text)
    }

    /// Styled text for stderr.
    pub fn err_styled(&self, style: Style, text: &str) -> String {
        self.theme_err().paint(style, text)
    }

    /// A normal line on stdout, suppressed by `--quiet`.
    pub fn info(&self, line: &str) {
        if !self.quiet {
            println!("{line}");
        }
    }

    /// Each line through [`Ui::info`].
    pub fn lines(&self, lines: &[String]) {
        lines.iter().for_each(|l| self.info(l));
    }

    /// A line on stdout that `--quiet` does not hide (command results such as IR dumps).
    pub fn data(&self, line: &str) {
        println!("{line}");
    }

    /// The short coloured header of a command: `veld 2.0 · generate  detail`.
    pub fn header(&self, command: &str, detail: &str) {
        self.info(&render::header(&self.theme_out(), command, detail));
    }

    /// `✓ message`
    pub fn ok(&self, msg: &str) {
        self.info(&render::ok(&self.theme_out(), msg));
    }

    /// `→ message`: what to do next.
    pub fn hint(&self, msg: &str) {
        self.info(&render::hint(&self.theme_out(), msg));
    }

    /// A line on stderr, never suppressed.
    pub fn error(&self, msg: &str) {
        eprintln!("{} {msg}", self.err_styled(Style::Red, "error:"));
    }

    pub fn warn(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{} {msg}", self.err_styled(Style::Yellow, "warning:"));
        }
    }

    /// A `-v` line on stderr (level 1: phases and timings, level 2: plan entries).
    pub fn debug(&self, level: u8, msg: &str) {
        if self.verbose >= level {
            eprintln!("{}", self.err_styled(Style::Dim, &format!("  · {msg}")));
        }
    }

    /// A spinner on stderr while something runs; silent unless stderr is a terminal.
    #[must_use]
    pub fn spinner(&self, message: &str) -> spinner::Spinner {
        if self.animate {
            spinner::Spinner::start(self.theme_err(), message)
        } else {
            spinner::Spinner::off()
        }
    }

    /// Prints a rendered compiler diagnostic block to stderr with the severity word coloured.
    pub fn diagnostic(&self, rendered: &str) {
        eprint!("{}", self.colorize_diagnostic(rendered));
        eprintln!();
    }

    pub fn colorize_diagnostic(&self, rendered: &str) -> String {
        let mut out = String::new();
        for line in rendered.lines() {
            out.push_str(&self.colorize_line(line));
            out.push('\n');
        }
        out
    }

    fn colorize_line(&self, line: &str) -> String {
        let head = |word: &str, style: Style| -> Option<String> {
            let rest = line.strip_prefix(word)?;
            let end = rest.find(':')?;
            let (code, tail) = rest.split_at(end);
            Some(format!(
                "{}{tail}",
                self.err_styled(style, &format!("{word}{code}"))
            ))
        };
        if let Some(s) = head("error", Style::Red).or_else(|| head("warning", Style::Yellow)) {
            return s;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("= help:") || trimmed.starts_with("= note:") {
            return self.err_styled(Style::Cyan, line);
        }
        if trimmed.starts_with("-->") {
            return self.err_styled(Style::Dim, line);
        }
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ui_adds_no_escape_codes() {
        let ui = Ui::plain();
        let text = "error[E0205]: unknown name `Foo`\n --> a.veld:1:1\n  = help: import it\n";
        assert_eq!(ui.colorize_diagnostic(text), text);
    }

    #[test]
    fn colored_ui_marks_only_the_severity_prefix() {
        let ui = Ui::colored();
        let out = ui.colorize_diagnostic("error[E0205]: unknown name\n");
        assert!(
            out.starts_with("\x1b[1;31merror[E0205]\x1b[0m: unknown name"),
            "{out:?}"
        );
        let warn = ui.colorize_diagnostic("warning[W0202]: shadows\n");
        assert!(warn.contains("\x1b[1;33mwarning[W0202]\x1b[0m"));
    }

    #[test]
    fn headers_and_marks_are_plain_without_colour() {
        let ui = Ui::plain();
        let t = ui.theme_out();
        assert!(!render::header(&t, "generate", "x").contains('\x1b'));
        assert!(render::ok(&t, "done").starts_with("✓ "));
    }
}

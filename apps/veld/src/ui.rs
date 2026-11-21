//! Terminal output: colour handling and the few helpers every command shares.

use std::io::IsTerminal;

#[derive(Debug, Clone, Copy)]
pub enum Style {
    Red,
    Green,
    Yellow,
    Cyan,
    Dim,
}

impl Style {
    fn code(self) -> &'static str {
        match self {
            Style::Red => "1;31",
            Style::Green => "32",
            Style::Yellow => "1;33",
            Style::Cyan => "36",
            Style::Dim => "2",
        }
    }
}

/// Output settings resolved once from the global flags and the environment.
#[derive(Debug, Clone, Copy)]
pub struct Ui {
    pub quiet: bool,
    color_out: bool,
    color_err: bool,
}

impl Ui {
    /// Colour is on only for a terminal, never with `--no-color` or a non-empty `NO_COLOR`.
    pub fn new(no_color: bool, quiet: bool) -> Self {
        let env_off = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        let allowed = !no_color && !env_off;
        Ui {
            quiet,
            color_out: allowed && std::io::stdout().is_terminal(),
            color_err: allowed && std::io::stderr().is_terminal(),
        }
    }

    #[cfg(test)]
    pub fn plain() -> Self {
        Ui {
            quiet: false,
            color_out: false,
            color_err: false,
        }
    }

    #[cfg(test)]
    pub fn colored() -> Self {
        Ui {
            quiet: false,
            color_out: true,
            color_err: true,
        }
    }

    fn paint(on: bool, style: Style, text: &str) -> String {
        if on {
            format!("\x1b[{}m{text}\x1b[0m", style.code())
        } else {
            text.to_string()
        }
    }

    /// Styled text for stdout.
    pub fn out_styled(&self, style: Style, text: &str) -> String {
        Self::paint(self.color_out, style, text)
    }

    /// Styled text for stderr.
    pub fn err_styled(&self, style: Style, text: &str) -> String {
        Self::paint(self.color_err, style, text)
    }

    /// A normal line on stdout, suppressed by `--quiet`.
    pub fn info(&self, line: &str) {
        if !self.quiet {
            println!("{line}");
        }
    }

    /// A line on stdout that `--quiet` does not hide (command results such as IR dumps).
    pub fn data(&self, line: &str) {
        println!("{line}");
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
}

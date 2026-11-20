//! Indentation-aware source writer with deterministic `\n` output.

/// Builds source text line by line.
///
/// Blank lines never carry trailing whitespace, consecutive blank lines are
/// collapsed and a leading blank line is dropped, so emitters can call
/// [`CodeWriter::blank`] freely.
#[derive(Debug, Clone)]
pub struct CodeWriter {
    buf: String,
    level: usize,
    unit: &'static str,
}

impl Default for CodeWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeWriter {
    /// A writer that indents with two spaces.
    #[must_use]
    pub fn new() -> Self {
        Self::with_unit("  ")
    }

    /// A writer with a custom indentation unit (for example four spaces).
    #[must_use]
    pub fn with_unit(unit: &'static str) -> Self {
        Self { buf: String::new(), level: 0, unit }
    }

    /// Writes `text` at the current indentation. Embedded newlines produce
    /// several lines, each indented. An empty string is a blank line.
    pub fn line(&mut self, text: impl AsRef<str>) -> &mut Self {
        let text = text.as_ref();
        if text.is_empty() {
            return self.blank();
        }
        for part in text.split('\n') {
            if part.trim().is_empty() {
                self.blank();
            } else {
                for _ in 0..self.level {
                    self.buf.push_str(self.unit);
                }
                self.buf.push_str(part.trim_end());
                self.buf.push('\n');
            }
        }
        self
    }

    /// Writes an empty line, unless one was just written or nothing precedes it.
    pub fn blank(&mut self) -> &mut Self {
        if !self.buf.is_empty() && !self.buf.ends_with("\n\n") {
            self.buf.push('\n');
        }
        self
    }

    /// Writes `head {` and indents.
    pub fn open(&mut self, head: impl AsRef<str>) -> &mut Self {
        self.open_with(head, " {")
    }

    /// Writes `head` followed by `opener` verbatim (for example `(` or ` = [`) and indents.
    pub fn open_with(&mut self, head: impl AsRef<str>, opener: &str) -> &mut Self {
        let line = format!("{}{opener}", head.as_ref());
        self.line(line);
        self.level += 1;
        self
    }

    /// Dedents and writes `}`.
    pub fn close(&mut self) -> &mut Self {
        self.close_with("}")
    }

    /// Dedents and writes `text` (for example `});` or `]`).
    pub fn close_with(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.level = self.level.saturating_sub(1);
        self.line(text)
    }

    pub fn indent(&mut self) -> &mut Self {
        self.level += 1;
        self
    }

    pub fn dedent(&mut self) -> &mut Self {
        self.level = self.level.saturating_sub(1);
        self
    }

    /// The finished text, ending in exactly one `\n` (or empty).
    #[must_use]
    pub fn finish(mut self) -> String {
        while self.buf.ends_with("\n\n") {
            self.buf.pop();
        }
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indents_blocks_and_collapses_blanks() {
        let mut w = CodeWriter::new();
        w.blank();
        w.open("fn a()");
        w.line("x;");
        w.blank();
        w.blank();
        w.open_with("call", "(");
        w.line("1,");
        w.close_with(");");
        w.close();
        assert_eq!(w.finish(), "fn a() {\n  x;\n\n  call(\n    1,\n  );\n}\n");
    }

    #[test]
    fn multiline_text_keeps_relative_indent_without_trailing_space() {
        let mut w = CodeWriter::with_unit("    ");
        w.open("a");
        w.line("one  \n\n  two");
        w.close();
        assert_eq!(w.finish(), "a {
    one

      two
}
");
    }

    #[test]
    fn close_without_open_does_not_panic() {
        let mut w = CodeWriter::new();
        w.close();
        assert_eq!(w.finish(), "}\n");
    }
}

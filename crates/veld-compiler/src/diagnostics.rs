//! Diagnostics, source map and the plain-text renderer.

use std::fmt::Write as _;

/// Index of a file inside a [`SourceMap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub u32);

impl FileId {
    /// Marker for diagnostics that belong to no source file (workspace config).
    pub const NONE: FileId = FileId(u32::MAX);
}

/// A byte range in one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    #[must_use]
    pub fn new(file: FileId, start: usize, end: usize) -> Span {
        Span {
            file,
            start: clamp_u32(start),
            end: clamp_u32(end.max(start)),
        }
    }
    /// A span that points at no file.
    #[must_use]
    pub fn none() -> Span {
        Span {
            file: FileId::NONE,
            start: 0,
            end: 0,
        }
    }
    /// Smallest span that covers both.
    #[must_use]
    pub fn to(self, other: Span) -> Span {
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

fn clamp_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub primary: Span,
    pub labels: Vec<(Span, String)>,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: &'static str, message: impl Into<String>, primary: Span) -> Diagnostic {
        Diagnostic::new(code, Severity::Error, message.into(), primary)
    }
    #[must_use]
    pub fn warning(code: &'static str, message: impl Into<String>, primary: Span) -> Diagnostic {
        Diagnostic::new(code, Severity::Warning, message.into(), primary)
    }
    fn new(code: &'static str, severity: Severity, message: String, primary: Span) -> Diagnostic {
        debug_assert!(
            crate::codes::lookup(code).is_some(),
            "diagnostic code `{code}` is not in the code table (crates/veld-compiler/src/codes.rs)"
        );
        Diagnostic {
            code,
            severity,
            message,
            primary,
            labels: Vec::new(),
            notes: Vec::new(),
            help: None,
        }
    }
    #[must_use]
    pub fn with_label(mut self, span: Span, text: impl Into<String>) -> Diagnostic {
        self.labels.push((span, text.into()));
        self
    }
    #[must_use]
    pub fn with_note(mut self, text: impl Into<String>) -> Diagnostic {
        self.notes.push(text.into());
        self
    }
    #[must_use]
    pub fn with_help(mut self, text: impl Into<String>) -> Diagnostic {
        self.help = Some(text.into());
        self
    }
    /// Help text `did you mean `x`?` when `word` is close to one of `candidates`;
    /// otherwise `fallback` (when given) is the help text.
    #[must_use]
    pub fn did_you_mean<'a>(
        self,
        word: &str,
        candidates: impl IntoIterator<Item = &'a str>,
        fallback: Option<String>,
    ) -> Diagnostic {
        match veld_ir::suggest::closest(word, candidates) {
            Some(s) => self.with_help(format!("did you mean `{s}`?")),
            None => match fallback {
                Some(f) => self.with_help(f),
                None => self,
            },
        }
    }
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// True when any diagnostic is an error.
#[must_use]
pub fn has_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(Diagnostic::is_error)
}

#[derive(Debug, Clone)]
struct SourceEntry {
    path: String,
    text: String,
    line_starts: Vec<usize>,
}

/// All source files of one compilation.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: Vec<SourceEntry>,
}

impl SourceMap {
    pub fn add(&mut self, path: impl Into<String>, text: impl Into<String>) -> FileId {
        let text = text.into();
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        let id = FileId(clamp_u32(self.files.len()));
        self.files.push(SourceEntry {
            path: path.into(),
            text,
            line_starts,
        });
        id
    }

    #[must_use]
    pub fn path(&self, file: FileId) -> Option<&str> {
        self.entry(file).map(|e| e.path.as_str())
    }

    #[must_use]
    pub fn text(&self, file: FileId) -> Option<&str> {
        self.entry(file).map(|e| e.text.as_str())
    }

    fn entry(&self, file: FileId) -> Option<&SourceEntry> {
        self.files.get(file.0 as usize)
    }

    /// 1-based (line, column) of a byte offset. Columns count characters.
    #[must_use]
    pub fn line_col(&self, file: FileId, offset: u32) -> Option<(u32, u32)> {
        let e = self.entry(file)?;
        let off = (offset as usize).min(e.text.len());
        let line = e
            .line_starts
            .partition_point(|&s| s <= off)
            .saturating_sub(1);
        let start = *e.line_starts.get(line)?;
        let slice = e.text.get(start..off).unwrap_or("");
        Some((clamp_u32(line + 1), clamp_u32(slice.chars().count() + 1)))
    }

    /// The text of a 1-based line without its line terminator.
    #[must_use]
    pub fn line_text(&self, file: FileId, line: u32) -> Option<&str> {
        let e = self.entry(file)?;
        let idx = (line as usize).checked_sub(1)?;
        let start = *e.line_starts.get(idx)?;
        let end = e.line_starts.get(idx + 1).copied().unwrap_or(e.text.len());
        Some(e.text.get(start..end)?.trim_end_matches(['\n', '\r']))
    }

    /// Renders one diagnostic as plain text.
    #[must_use]
    pub fn render(&self, d: &Diagnostic) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "{}[{}]: {}", d.severity.as_str(), d.code, d.message);
        self.render_snippet(&mut out, d.primary, None, '^');
        for (span, text) in &d.labels {
            self.render_snippet(&mut out, *span, Some(text), '-');
        }
        for n in &d.notes {
            let _ = writeln!(out, "  = note: {n}");
        }
        if let Some(h) = &d.help {
            let _ = writeln!(out, "  = help: {h}");
        }
        out
    }

    /// Renders every diagnostic, separated by blank lines.
    #[must_use]
    pub fn render_all(&self, diags: &[Diagnostic]) -> String {
        diags
            .iter()
            .map(|d| self.render(d))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_snippet(&self, out: &mut String, span: Span, text: Option<&String>, mark: char) {
        let Some(path) = self.path(span.file) else {
            let _ = writeln!(out, " --> <workspace>");
            if let Some(t) = text {
                let _ = writeln!(out, "  = {t}");
            }
            return;
        };
        let (line, col) = self.line_col(span.file, span.start).unwrap_or((1, 1));
        let _ = writeln!(out, " --> {path}:{line}:{col}");
        let Some(src) = self.line_text(span.file, line) else {
            return;
        };
        let num = line.to_string();
        let pad = " ".repeat(num.len());
        let width = self.underline_width(span, line, col);
        let lead = " ".repeat((col as usize).saturating_sub(1));
        let label = text.map(|t| format!(" {t}")).unwrap_or_default();
        let _ = writeln!(out, "{pad} |");
        let _ = writeln!(out, "{num} | {}", src.replace('\t', " "));
        let _ = writeln!(
            out,
            "{pad} | {lead}{}{label}",
            mark.to_string().repeat(width)
        );
    }

    fn underline_width(&self, span: Span, line: u32, col: u32) -> usize {
        let Some(src) = self.line_text(span.file, line) else {
            return 1;
        };
        let start_byte = src
            .char_indices()
            .nth((col as usize).saturating_sub(1))
            .map_or(src.len(), |(i, _)| i);
        let len = (span.end.saturating_sub(span.start)) as usize;
        let tail = src.get(start_byte..).unwrap_or("");
        let shown: usize = tail.char_indices().take_while(|(i, _)| *i < len).count();
        shown.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_is_one_based_and_counts_chars() {
        let mut m = SourceMap::default();
        let f = m.add("a.veld", "ab\ncé x\n");
        assert_eq!(m.line_col(f, 0), Some((1, 1)));
        assert_eq!(m.line_col(f, 3), Some((2, 1)));
        assert_eq!(m.line_col(f, 7), Some((2, 4)));
    }

    #[test]
    fn render_prints_header_location_and_caret() {
        let mut m = SourceMap::default();
        let f = m.add("app/a.veld", "model A {\n  x: Foo\n}\n");
        let d = Diagnostic::error("E0205", "unknown name `Foo`", Span::new(f, 15, 18))
            .with_help("import it");
        let text = m.render(&d);
        assert_eq!(
            text,
            "error[E0205]: unknown name `Foo`\n --> app/a.veld:2:6\n  |\n2 |   x: Foo\n  |      ^^^\n  = help: import it\n"
        );
    }

    #[test]
    fn render_without_file_points_at_workspace() {
        let m = SourceMap::default();
        let d = Diagnostic::warning("W0701", "self", Span::none());
        assert_eq!(m.render(&d), "warning[W0701]: self\n --> <workspace>\n");
    }

    #[test]
    fn labels_render_with_dashes() {
        let mut m = SourceMap::default();
        let f = m.add("a.veld", "abc def\n");
        let d = Diagnostic::error("E0201", "dup", Span::new(f, 4, 7))
            .with_label(Span::new(f, 0, 3), "first here");
        assert!(m.render(&d).contains("| --- first here"));
    }
}

package codegen

import (
	"bytes"
	"strings"
)

// Writer provides buffered, indented code output for any language.
// Handles indentation, line breaks, and import collection automatically.
type Writer struct {
	buf       *bytes.Buffer
	indent    int
	indentStr string
	imports   map[string]bool // deduplicated imports
}

// NewWriter creates a new code writer with the given indentation string (e.g., "  " or "\t").
func NewWriter(indentStr string) *Writer {
	return &Writer{
		buf:       &bytes.Buffer{},
		indent:    0,
		indentStr: indentStr,
		imports:   make(map[string]bool),
	}
}

// Write writes a string to the buffer with current indentation.
func (w *Writer) Write(s string) *Writer {
	if w.buf.Len() == 0 || w.buf.String()[w.buf.Len()-1] == '\n' {
		for i := 0; i < w.indent; i++ {
			w.buf.WriteString(w.indentStr)
		}
	}
	w.buf.WriteString(s)
	return w
}

// Writeln writes a string with current indentation and appends a newline.
func (w *Writer) Writeln(s string) *Writer {
	w.Write(s)
	w.buf.WriteString("\n")
	return w
}

// WriteBlock writes a line with opening brace, increments indent, and returns the writer.
// Usage: w.WriteBlock("function foo() {").Writeln("body").Dedent().Writeln("}")
func (w *Writer) WriteBlock(s string) *Writer {
	w.Writeln(s)
	w.Indent()
	return w
}

// Indent increases indentation level.
func (w *Writer) Indent() *Writer {
	w.indent++
	return w
}

// Dedent decreases indentation level.
func (w *Writer) Dedent() *Writer {
	if w.indent > 0 {
		w.indent--
	}
	return w
}

// BlankLine writes a blank line.
func (w *Writer) BlankLine() *Writer {
	w.buf.WriteString("\n")
	return w
}

// AddImport records an import statement (deduplicates automatically).
// Import format depends on language; just store the import spec here.
// Example: "chi/v5", "serde", "java.util.*"
func (w *Writer) AddImport(importSpec string) *Writer {
	w.imports[importSpec] = true
	return w
}

// Imports returns all recorded imports as a slice.
func (w *Writer) Imports() []string {
	imports := make([]string, 0, len(w.imports))
	for imp := range w.imports {
		imports = append(imports, imp)
	}
	// Sort for deterministic output
	sortStrings(imports)
	return imports
}

// ClearImports clears the import set.
func (w *Writer) ClearImports() *Writer {
	w.imports = make(map[string]bool)
	return w
}

// String returns the complete generated code as a string.
func (w *Writer) String() string {
	return w.buf.String()
}

// Bytes returns the generated code as bytes.
func (w *Writer) Bytes() []byte {
	return w.buf.Bytes()
}

// Len returns the number of bytes written so far.
func (w *Writer) Len() int {
	return w.buf.Len()
}

// Reset clears the buffer and resets state.
func (w *Writer) Reset() *Writer {
	w.buf = &bytes.Buffer{}
	w.indent = 0
	w.imports = make(map[string]bool)
	return w
}

// WriteComment writes a single-line comment in the specified language style.
func (w *Writer) WriteComment(text string, syntax CommentStyle) *Writer {
	w.Writeln(syntax.Single + " " + text)
	return w
}

// WriteMultiLineComment writes a multi-line comment.
func (w *Writer) WriteMultiLineComment(lines []string, syntax CommentStyle) *Writer {
	if len(lines) == 0 {
		return w
	}
	if syntax.Multi == "" {
		// Fall back to single-line comments
		for _, line := range lines {
			w.WriteComment(line, syntax)
		}
		return w
	}

	w.Writeln(syntax.Multi)
	for _, line := range lines {
		w.Writeln(strings.TrimLeft(syntax.Multi, " ") + " " + line)
	}
	w.Writeln(syntax.MultiEnd)
	return w
}

// Separator writes a visual separator (e.g., "// ---" or "# ---").
func (w *Writer) Separator(syntax CommentStyle) *Writer {
	w.Writeln(syntax.Single + " " + strings.Repeat("-", 60))
	return w
}

// sortStrings sorts a slice of strings in-place (simple bubble sort for small slices).
func sortStrings(s []string) {
	for i := 0; i < len(s); i++ {
		for j := i + 1; j < len(s); j++ {
			if s[j] < s[i] {
				s[i], s[j] = s[j], s[i]
			}
		}
	}
}

// CommentStyle describes language comment syntax.
type CommentStyle struct {
	Single   string // "//"
	Multi    string // "/*"
	MultiEnd string // "*/"
}

// FormatCode applies basic formatting to generated code.
// This removes trailing whitespace and ensures proper line endings.
func FormatCode(code string) string {
	lines := strings.Split(code, "\n")
	var result []string
	for _, line := range lines {
		result = append(result, strings.TrimRight(line, " \t"))
	}
	return strings.Join(result, "\n")
}

// IndentCode indents all lines of code by the given number of levels.
// indentStr is typically "  " or "\t".
func IndentCode(code string, levels int, indentStr string) string {
	if levels <= 0 {
		return code
	}
	indent := strings.Repeat(indentStr, levels)
	lines := strings.Split(code, "\n")
	var result []string
	for _, line := range lines {
		if line != "" {
			result = append(result, indent+line)
		} else {
			result = append(result, "")
		}
	}
	return strings.Join(result, "\n")
}

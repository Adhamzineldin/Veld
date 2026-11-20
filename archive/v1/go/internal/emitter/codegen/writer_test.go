package codegen

import (
	"strings"
	"testing"
)

func TestWriterBasic(t *testing.T) {
	w := NewWriter("  ")
	w.Writeln("package main")
	w.BlankLine()
	w.Writeln("func main() {")
	w.Indent()
	w.Writeln("fmt.Println(\"Hello\")")
	w.Dedent()
	w.Writeln("}")

	code := w.String()
	expected := "package main\n\nfunc main() {\n  fmt.Println(\"Hello\")\n}\n"
	if code != expected {
		t.Errorf("got %q, want %q", code, expected)
	}
}

func TestWriterIndentation(t *testing.T) {
	w := NewWriter("  ")
	w.Writeln("if true {")
	w.Indent()
	w.Writeln("for i := 0; i < 10; i++ {")
	w.Indent()
	w.Writeln("x := i")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")

	code := w.String()
	lines := strings.Split(strings.TrimSpace(code), "\n")

	if len(lines) != 5 {
		t.Errorf("expected 5 lines, got %d", len(lines))
	}

	// Check indentation levels
	if !strings.HasPrefix(lines[1], "  for") {
		t.Errorf("line 2 should have 2-space indent")
	}
	if !strings.HasPrefix(lines[2], "    x :=") {
		t.Errorf("line 3 should have 4-space indent")
	}
}

func TestWriterImports(t *testing.T) {
	w := NewWriter("  ")
	w.AddImport("fmt")
	w.AddImport("errors")
	w.AddImport("fmt") // duplicate

	imports := w.Imports()
	if len(imports) != 2 {
		t.Errorf("expected 2 imports, got %d", len(imports))
	}

	// Check deduplication
	count := 0
	for _, imp := range imports {
		if imp == "fmt" {
			count++
		}
	}
	if count != 1 {
		t.Errorf("duplicate import not removed")
	}
}

func TestWriterComments(t *testing.T) {
	w := NewWriter("  ")
	style := CommentStyle{Single: "//", Multi: "/*", MultiEnd: "*/"}

	w.WriteComment("This is a comment", style)
	w.WriteMultiLineComment([]string{"Line 1", "Line 2"}, style)

	code := w.String()
	if !strings.Contains(code, "// This is a comment") {
		t.Errorf("single comment not found in output")
	}
	if !strings.Contains(code, "/*") {
		t.Errorf("multi comment start not found")
	}
}

func TestWriterReset(t *testing.T) {
	w := NewWriter("  ")
	w.Writeln("initial")
	w.AddImport("fmt")

	w.Reset()

	if w.Len() != 0 {
		t.Errorf("writer not reset")
	}
	if len(w.Imports()) != 0 {
		t.Errorf("imports not cleared")
	}
}

func TestFormatCode(t *testing.T) {
	code := "line1  \nline2\t\nline3\n"
	formatted := FormatCode(code)
	expected := "line1\nline2\nline3\n"
	if formatted != expected {
		t.Errorf("got %q, want %q", formatted, expected)
	}
}

func TestIndentCode(t *testing.T) {
	code := "line1\nline2\nline3"
	indented := IndentCode(code, 2, "  ")
	lines := strings.Split(indented, "\n")

	for i, line := range lines {
		if !strings.HasPrefix(line, "    ") {
			t.Errorf("line %d not indented: %q", i, line)
		}
	}
}

func TestImportManagerDeduplication(t *testing.T) {
	im := NewImportManager()
	im.Add("fmt", GroupStdlib)
	im.Add("errors", GroupStdlib)
	im.Add("fmt", GroupStdlib) // duplicate

	if im.Len() != 2 {
		t.Errorf("expected 2 imports, got %d", im.Len())
	}
}

func TestImportManagerFormatGo(t *testing.T) {
	im := NewImportManager()
	im.Add("fmt", GroupStdlib)
	im.Add("errors", GroupStdlib)
	im.Add("github.com/go-chi/chi/v5", GroupThirdParty)

	formatted := im.Format("go")

	if !strings.Contains(formatted, "import (") {
		t.Errorf("import block not found")
	}
	if !strings.Contains(formatted, "fmt") {
		t.Errorf("fmt import not found")
	}
	if !strings.Contains(formatted, "chi/v5") {
		t.Errorf("chi import not found")
	}
}

func TestImportManagerFormatRust(t *testing.T) {
	im := NewImportManager()
	im.Add("serde_json", GroupThirdParty)
	im.Add("tokio::runtime", GroupThirdParty)

	formatted := im.Format("rust")

	if !strings.Contains(formatted, "use serde_json;") && !strings.Contains(formatted, "use serde_json") {
		t.Errorf("serde_json import not found")
	}
}

func TestImportManagerFormatJava(t *testing.T) {
	im := NewImportManager()
	im.Add("java.util.List", GroupStdlib)
	im.Add("com.example.User", GroupLocal)

	formatted := im.Format("java")

	if !strings.Contains(formatted, "import java.util.List;") && !strings.Contains(formatted, "import java.util.List") {
		t.Errorf("java.util.List import not found")
	}
}

func TestImportManagerFormatPython(t *testing.T) {
	im := NewImportManager()
	im.Add("json", GroupStdlib)

	formatted := im.Format("python")

	if !strings.Contains(formatted, "import json") {
		t.Errorf("json import not found in Python format")
	}
}

func TestImportManagerClear(t *testing.T) {
	im := NewImportManager()
	im.Add("fmt", GroupStdlib)
	im.Clear()

	if im.Len() != 0 {
		t.Errorf("imports not cleared")
	}
}

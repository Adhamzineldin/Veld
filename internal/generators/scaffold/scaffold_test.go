package scaffold_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/generators/scaffold"
)

func minimalAST() ast.AST {
	return ast.AST{
		Modules: []ast.Module{
			{Name: "Auth", Actions: []ast.Action{
				{Name: "Login", Method: "POST", Path: "/auth/login"},
				{Name: "Me", Method: "GET", Path: "/auth/me"},
			}},
		},
	}
}

func TestScaffoldEmitCreatesTestFiles(t *testing.T) {
	e := scaffold.New()
	outDir := t.TempDir()

	// Create package.json to simulate node project
	os.WriteFile(filepath.Join(outDir, "package.json"), []byte("{}"), 0644)

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	testsDir := filepath.Join(outDir, "tests")
	if _, err := os.Stat(testsDir); os.IsNotExist(err) {
		t.Fatal("expected tests/ directory to exist")
	}

	entries, _ := os.ReadDir(testsDir)
	if len(entries) == 0 {
		t.Fatal("expected at least one test file")
	}
}

func TestScaffoldNodeTestContent(t *testing.T) {
	e := scaffold.New()
	outDir := t.TempDir()
	os.WriteFile(filepath.Join(outDir, "package.json"), []byte("{}"), 0644)

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "tests", "auth.test.ts"))
	content := string(data)

	for _, needle := range []string{"describe", "it(", "expect", "Login", "Me"} {
		if !strings.Contains(content, needle) {
			t.Errorf("auth.test.ts missing %q", needle)
		}
	}
}

func TestScaffoldGoTestContent(t *testing.T) {
	e := scaffold.New()
	outDir := t.TempDir()
	os.WriteFile(filepath.Join(outDir, "go.mod"), []byte("module x"), 0644)

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "tests", "auth_test.go"))
	content := string(data)

	for _, needle := range []string{"package tests", "testing", "TestAuth"} {
		if !strings.Contains(content, needle) {
			t.Errorf("auth_test.go missing %q", needle)
		}
	}
}

func TestScaffoldDryRun(t *testing.T) {
	e := scaffold.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

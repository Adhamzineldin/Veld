package cicd_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/generators/cicd"
)

func minimalAST() ast.AST {
	return ast.AST{Modules: []ast.Module{{Name: "Auth"}}}
}

func TestCICDEmitCreatesWorkflow(t *testing.T) {
	e := cicd.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, ".github", "workflows", "ci.yml")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("expected ci.yml to exist")
	}
}

func TestCICDWorkflowContent(t *testing.T) {
	e := cicd.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, ".github", "workflows", "ci.yml"))
	content := string(data)

	for _, needle := range []string{"name: CI", "on:", "push:", "jobs:", "build:", "docker:"} {
		if !strings.Contains(content, needle) {
			t.Errorf("ci.yml missing %q", needle)
		}
	}
}

func TestCICDGoDetection(t *testing.T) {
	e := cicd.New()
	outDir := t.TempDir()
	os.WriteFile(filepath.Join(outDir, "go.mod"), []byte("module x"), 0644)

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, ".github", "workflows", "ci.yml"))
	if !strings.Contains(string(data), "setup-go") {
		t.Error("ci.yml should use setup-go action for Go projects")
	}
}

func TestCICDDryRun(t *testing.T) {
	e := cicd.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

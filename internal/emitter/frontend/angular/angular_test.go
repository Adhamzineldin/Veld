package angular_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/frontend/angular"
)

func minimalAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{Name: "LoginInput", Fields: []ast.Field{
				{Name: "email", Type: "string"},
			}},
			{Name: "User", Fields: []ast.Field{
				{Name: "id", Type: "string"},
				{Name: "name", Type: "string"},
			}},
		},
		Modules: []ast.Module{
			{Name: "Auth", Prefix: "/api", Actions: []ast.Action{
				{Name: "Login", Method: "POST", Path: "/auth/login", Input: "LoginInput", Output: "User"},
				{Name: "Me", Method: "GET", Path: "/auth/me", Output: "User"},
			}},
		},
	}
}

func TestAngularEmitCreatesFiles(t *testing.T) {
	e := angular.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	expected := []string{
		filepath.Join(outDir, "models", "index.ts"),
		filepath.Join(outDir, "services", "auth.service.ts"),
		filepath.Join(outDir, "services", "index.ts"),
	}
	for _, f := range expected {
		if _, err := os.Stat(f); os.IsNotExist(err) {
			t.Errorf("expected %s to exist", f)
		}
	}
}

func TestAngularServiceContent(t *testing.T) {
	e := angular.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "services", "auth.service.ts"))
	content := string(data)

	for _, needle := range []string{
		"@Injectable",
		"HttpClient",
		"Observable",
		"AuthService",
		"login(",
		"me(",
	} {
		if !strings.Contains(content, needle) {
			t.Errorf("auth.service.ts missing %q", needle)
		}
	}
}

func TestAngularModelsContent(t *testing.T) {
	e := angular.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "models", "index.ts"))
	content := string(data)

	for _, needle := range []string{"export interface User", "export interface LoginInput"} {
		if !strings.Contains(content, needle) {
			t.Errorf("models/index.ts missing %q", needle)
		}
	}
}

func TestAngularDryRun(t *testing.T) {
	e := angular.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

package vue_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/typescript"
	"github.com/Adhamzineldin/Veld/internal/emitter/frontend/vue"
)

func minimalAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{Name: "LoginInput", Fields: []ast.Field{
				{Name: "email", Type: "string"},
			}},
			{Name: "User", Fields: []ast.Field{
				{Name: "id", Type: "string"},
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

func TestVueEmitCreatesFiles(t *testing.T) {
	e := vue.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	expected := []string{
		filepath.Join(outDir, "client", "api.ts"),
		filepath.Join(outDir, "composables", "useAuth.ts"),
		filepath.Join(outDir, "composables", "index.ts"),
	}
	for _, f := range expected {
		if _, err := os.Stat(f); os.IsNotExist(err) {
			t.Errorf("expected %s to exist", f)
		}
	}
}

func TestVueComposableContent(t *testing.T) {
	e := vue.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "composables", "useAuth.ts"))
	content := string(data)

	for _, needle := range []string{"from 'vue'", "ref(false)", "useAuth", "loading", "error", "api.Auth"} {
		if !strings.Contains(content, needle) {
			t.Errorf("useAuth.ts missing %q", needle)
		}
	}
}

func TestVueDryRun(t *testing.T) {
	e := vue.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

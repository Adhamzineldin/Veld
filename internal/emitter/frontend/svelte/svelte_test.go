package svelte_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/frontend/svelte"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/typescript"
)

func minimalAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{Name: "LoginInput", Fields: []ast.Field{{Name: "email", Type: "string"}}},
			{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}},
		},
		Modules: []ast.Module{
			{Name: "Auth", Prefix: "/api", Actions: []ast.Action{
				{Name: "Login", Method: "POST", Path: "/auth/login", Input: "LoginInput", Output: "User"},
				{Name: "Me", Method: "GET", Path: "/auth/me", Output: "User"},
			}},
		},
	}
}

func TestSvelteEmitCreatesFiles(t *testing.T) {
	e := svelte.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	expected := []string{
		filepath.Join(outDir, "client", "api.ts"),
		filepath.Join(outDir, "stores", "auth.store.ts"),
		filepath.Join(outDir, "stores", "index.ts"),
	}
	for _, f := range expected {
		if _, err := os.Stat(f); os.IsNotExist(err) {
			t.Errorf("expected %s to exist", f)
		}
	}
}

func TestSvelteStoreContent(t *testing.T) {
	e := svelte.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "stores", "auth.store.ts"))
	content := string(data)

	for _, needle := range []string{"writable", "svelte/store", "createAuthStore", "loading", "authApi"} {
		if !strings.Contains(content, needle) {
			t.Errorf("auth.store.ts missing %q", needle)
		}
	}
}

func TestSvelteDryRun(t *testing.T) {
	e := svelte.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

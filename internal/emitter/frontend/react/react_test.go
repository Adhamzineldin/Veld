package react_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/frontend/react"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/typescript"
)

func minimalAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{Name: "LoginInput", Fields: []ast.Field{
				{Name: "email", Type: "string"},
				{Name: "password", Type: "string"},
			}},
			{Name: "User", Fields: []ast.Field{
				{Name: "id", Type: "string"},
				{Name: "email", Type: "string"},
			}},
		},
		Enums: []ast.Enum{
			{Name: "Role", Values: []string{"admin", "user"}},
		},
		Modules: []ast.Module{
			{Name: "Auth", Prefix: "/api", Actions: []ast.Action{
				{Name: "Login", Method: "POST", Path: "/auth/login", Input: "LoginInput", Output: "User", Description: "Log in"},
				{Name: "Me", Method: "GET", Path: "/auth/me", Output: "User", Description: "Get current user"},
			}},
		},
	}
}

func TestReactEmitCreatesFiles(t *testing.T) {
	e := react.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	expected := []string{
		filepath.Join(outDir, "client", "api.ts"),
		filepath.Join(outDir, "hooks", "authHooks.ts"),
		filepath.Join(outDir, "hooks", "index.ts"),
	}
	for _, f := range expected {
		if _, err := os.Stat(f); os.IsNotExist(err) {
			t.Errorf("expected %s to exist", f)
		}
	}
}

func TestReactDryRun(t *testing.T) {
	e := react.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

func TestReactHooksContent(t *testing.T) {
	e := react.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "hooks", "authHooks.ts"))
	content := string(data)

	checks := []string{
		"useState",
		"useEffect",
		"useCallback",
		"useLogin",
		"useMe",
		"setData",
		"setLoading",
		"setError",
		"refetch",
		"LoginInput",
		"User",
		"authHooks",
	}
	for _, needle := range checks {
		if !strings.Contains(content, needle) {
			t.Errorf("authHooks.ts missing %q", needle)
		}
	}
}

func TestReactBarrelExport(t *testing.T) {
	e := react.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "hooks", "index.ts"))
	if !strings.Contains(string(data), "authHooks") {
		t.Error("index.ts should barrel-export authHooks")
	}
}

func TestReactSummary(t *testing.T) {
	e := react.New()
	lines := e.Summary([]string{"Auth"})
	if len(lines) == 0 {
		t.Fatal("Summary should return lines")
	}
	found := false
	for _, l := range lines {
		if strings.Contains(l.Dir, "hooks") {
			found = true
		}
	}
	if !found {
		t.Error("Summary should mention hooks/")
	}
}

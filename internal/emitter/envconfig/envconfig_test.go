package envconfig_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/envconfig"
)

func minimalAST() ast.AST {
	return ast.AST{
		Modules: []ast.Module{
			{Name: "Auth", Actions: []ast.Action{
				{Name: "Login", Method: "POST", Path: "/auth/login"},
			}},
		},
	}
}

func TestEnvEmitCreatesFile(t *testing.T) {
	e := envconfig.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, ".env.example")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("expected .env.example to exist")
	}
}

func TestEnvContent(t *testing.T) {
	e := envconfig.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, ".env.example"))
	content := string(data)

	for _, needle := range []string{"PORT=8080", "DATABASE_URL", "CORS_ORIGIN", "LOG_LEVEL"} {
		if !strings.Contains(content, needle) {
			t.Errorf(".env.example missing %q", needle)
		}
	}
}

func TestEnvAuthDetection(t *testing.T) {
	e := envconfig.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, ".env.example"))
	content := string(data)

	// Auth module detected → JWT vars should be present
	if !strings.Contains(content, "JWT_SECRET") {
		t.Error(".env.example should contain JWT_SECRET when Auth module exists")
	}
}

func TestEnvNoAuth(t *testing.T) {
	e := envconfig.New()
	outDir := t.TempDir()

	noAuth := ast.AST{
		Modules: []ast.Module{{Name: "Products"}},
	}

	if err := e.Emit(noAuth, outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, ".env.example"))
	if strings.Contains(string(data), "JWT_SECRET") {
		t.Error(".env.example should NOT contain JWT_SECRET when no Auth module")
	}
}

func TestEnvDryRun(t *testing.T) {
	e := envconfig.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

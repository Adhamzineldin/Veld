package typesonly_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/frontend/typesonly"
)

func minimalAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{
				{Name: "id", Type: "uuid"},
				{Name: "email", Type: "string"},
				{Name: "role", Type: "Role"},
				{Name: "tags", Type: "string", IsArray: true},
				{Name: "bio", Type: "string", Optional: true},
			}},
		},
		Enums: []ast.Enum{
			{Name: "Role", Values: []string{"admin", "user", "guest"}},
		},
		Modules: []ast.Module{},
	}
}

func TestTypesOnlyEmitCreatesFiles(t *testing.T) {
	e := typesonly.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	expected := []string{
		filepath.Join(outDir, "types", "index.ts"),
	}
	for _, f := range expected {
		if _, err := os.Stat(f); os.IsNotExist(err) {
			t.Errorf("expected %s to exist", f)
		}
	}
}

func TestTypesOnlyTypesContent(t *testing.T) {
	e := typesonly.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "types", "index.ts"))
	content := string(data)

	for _, needle := range []string{
		"export interface User",
		"export enum Role",
		"tags: string[]",
		"bio?: string",
	} {
		if !strings.Contains(content, needle) {
			t.Errorf("types/index.ts missing %q", needle)
		}
	}
}

func TestTypesOnlyDryRun(t *testing.T) {
	e := typesonly.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

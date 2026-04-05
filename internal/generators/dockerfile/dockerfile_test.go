package dockerfile_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/generators/dockerfile"
)

func minimalAST() ast.AST {
	return ast.AST{Modules: []ast.Module{{Name: "Auth"}}}
}

func TestDockerfileEmitCreatesFiles(t *testing.T) {
	e := dockerfile.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	for _, f := range []string{"Dockerfile", ".dockerignore"} {
		path := filepath.Join(outDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected %s to exist", f)
		}
	}
}

func TestDockerfileContent(t *testing.T) {
	e := dockerfile.New()
	outDir := t.TempDir()

	// Write a package.json to simulate node project
	os.WriteFile(filepath.Join(outDir, "package.json"), []byte("{}"), 0644)

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "Dockerfile"))
	content := string(data)

	for _, needle := range []string{"FROM", "WORKDIR", "EXPOSE", "CMD"} {
		if !strings.Contains(content, needle) {
			t.Errorf("Dockerfile missing %q", needle)
		}
	}
}

func TestDockerfileGoDetection(t *testing.T) {
	e := dockerfile.New()
	outDir := t.TempDir()

	// Write a go.mod to simulate Go project
	os.WriteFile(filepath.Join(outDir, "go.mod"), []byte("module x"), 0644)

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "Dockerfile"))
	if !strings.Contains(string(data), "golang") {
		t.Error("Dockerfile should use Go base image when go.mod exists")
	}
}

func TestDockerfileDryRun(t *testing.T) {
	e := dockerfile.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

func TestDockerignoreContent(t *testing.T) {
	e := dockerfile.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, ".dockerignore"))
	content := string(data)
	if !strings.Contains(content, ".git") {
		t.Error(".dockerignore should include .git")
	}
}

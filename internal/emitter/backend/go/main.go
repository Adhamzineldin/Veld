// Package gobackend provides a Go backend code generator for Veld.
// It generates a complete, compilable Go HTTP service using the Chi router.
//
// Registration happens via init() — just blank-import this package in main.go:
//
//	_ "github.com/veld-dev/veld/internal/emitter/backend/go"
package gobackend

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/veld-dev/veld/internal/ast"
	"github.com/veld-dev/veld/internal/emitter"
	"github.com/veld-dev/veld/internal/emitter/lang"
)

// goModuleName is the Go module path used in go.mod and internal imports.
// Users should run `go mod edit -module <their-path>` after generating.
const goModuleName = "example.com/veld-generated"

func init() {
	emitter.RegisterBackend("go", New())
}

// GoEmitter generates a complete Go HTTP backend from a Veld AST.
// Uses the Chi router; all generated output is idiomatic Go.
type GoEmitter struct {
	adapter lang.LanguageAdapter
}

// New creates a GoEmitter with the standard Go language adapter.
func New() *GoEmitter {
	return &GoEmitter{adapter: &lang.GoAdapter{}}
}

// IsBackend satisfies the BackendEmitter marker interface.
func (e *GoEmitter) IsBackend() {}

// Emit generates all Go backend files into outDir.
// The output is deterministic: same AST → identical files.
func (e *GoEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
	if opts.DryRun {
		for _, line := range e.Summary(moduleNames(a.Modules)) {
			fmt.Printf("  [dry-run] %s%s\n", line.Dir, line.Files)
		}
		return nil
	}

	if err := e.createDirs(outDir); err != nil {
		return err
	}

	steps := []struct {
		name string
		fn   func() error
	}{
		{"types", func() error { return e.generateTypes(a, outDir) }},
		{"middleware", func() error { return e.generateMiddleware(outDir) }},
		{"routes setup", func() error { return e.generateRoutesSetup(a, outDir) }},
		{"server", func() error { return e.generateServer(a, outDir) }},
		{"main", func() error { return e.generateMain(outDir) }},
		{"go.mod", func() error { return e.generateGoMod(outDir) }},
	}

	for _, step := range steps {
		if err := step.fn(); err != nil {
			return fmt.Errorf("go emitter [%s]: %w", step.name, err)
		}
	}

	// Per-module generation.
	for _, mod := range a.Modules {
		if err := e.generateInterface(a, mod, outDir); err != nil {
			return fmt.Errorf("go emitter [interface for %s]: %w", mod.Name, err)
		}
		if err := e.generateModuleRoutes(a, mod, outDir); err != nil {
			return fmt.Errorf("go emitter [routes for %s]: %w", mod.Name, err)
		}
	}

	return nil
}

// Summary returns a human-readable description of the generated files.
func (e *GoEmitter) Summary(modules []string) []emitter.SummaryLine {
	var lines []emitter.SummaryLine

	lines = append(lines, emitter.SummaryLine{
		Dir:   "internal/models/",
		Files: "types.go",
	})

	ifaceFiles := make([]string, len(modules))
	for i, m := range modules {
		ifaceFiles[i] = strings.ToLower(m) + ".go"
	}
	lines = append(lines, emitter.SummaryLine{
		Dir:   "internal/interfaces/",
		Files: strings.Join(ifaceFiles, ", "),
	})

	routeFiles := make([]string, len(modules)+1)
	routeFiles[0] = "routes.go"
	for i, m := range modules {
		routeFiles[i+1] = strings.ToLower(m) + ".go"
	}
	lines = append(lines, emitter.SummaryLine{
		Dir:   "internal/routes/",
		Files: strings.Join(routeFiles, ", "),
	})

	lines = append(lines, emitter.SummaryLine{
		Dir:   "internal/middleware/",
		Files: "errors.go",
	})

	lines = append(lines, emitter.SummaryLine{
		Dir:   "./",
		Files: "server.go, main.go, go.mod",
	})

	return lines
}

// createDirs ensures all required output directories exist.
func (e *GoEmitter) createDirs(outDir string) error {
	dirs := []string{
		filepath.Join(outDir, "internal", "models"),
		filepath.Join(outDir, "internal", "interfaces"),
		filepath.Join(outDir, "internal", "routes"),
		filepath.Join(outDir, "internal", "middleware"),
	}
	for _, dir := range dirs {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return fmt.Errorf("mkdir %s: %w", dir, err)
		}
	}
	return nil
}

// moduleNames extracts module names from a slice of modules.
func moduleNames(modules []ast.Module) []string {
	names := make([]string, len(modules))
	for i, m := range modules {
		names[i] = m.Name
	}
	return names
}

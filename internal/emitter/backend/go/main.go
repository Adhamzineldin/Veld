// Package gobackend provides a Go backend code generator for Veld.
package gobackend

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/veld-dev/veld/internal/ast"
	"github.com/veld-dev/veld/internal/emitter"
	"github.com/veld-dev/veld/internal/emitter/lang"
)

func init() {
	emitter.RegisterBackend("go", New())
}

// GoEmitter generates Go backend code from a Veld AST.
type GoEmitter struct {
	adapter lang.LanguageAdapter
}

// New creates a new Go emitter with the Go language adapter.
func New() *GoEmitter {
	return &GoEmitter{
		adapter: &lang.GoAdapter{},
	}
}

// IsBackend marks this as a backend emitter.
func (e *GoEmitter) IsBackend() {}

// Emit generates Go backend code and writes it to outDir.
func (e *GoEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
	if opts.DryRun {
		fmt.Println("[DRY RUN] Go backend would generate:")
		for _, m := range a.Modules {
			fmt.Printf("  - module: %s\n", m.Name)
		}
		return nil
	}

	// Create output directory structure
	if err := os.MkdirAll(outDir, 0755); err != nil {
		return fmt.Errorf("failed to create output directory: %w", err)
	}

	// Create subdirectories
	dirs := []string{
		filepath.Join(outDir, "internal", "models"),
		filepath.Join(outDir, "internal", "routes"),
		filepath.Join(outDir, "internal", "middleware"),
	}
	for _, dir := range dirs {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return fmt.Errorf("failed to create directory %s: %w", dir, err)
		}
	}

	// Generate types
	if err := e.generateCommonTypes(a, outDir); err != nil {
		return fmt.Errorf("failed to generate types: %w", err)
	}

	// Generate routes
	if err := e.generateRoutesSetup(a, outDir); err != nil {
		return fmt.Errorf("failed to generate routes: %w", err)
	}

	// Generate routes for each module
	for _, m := range a.Modules {
		if err := e.generateModuleRoutes(m, outDir); err != nil {
			return fmt.Errorf("failed to generate routes for module %s: %w", m.Name, err)
		}
	}

	// Generate middleware
	if err := e.generateErrorMiddleware(outDir); err != nil {
		return fmt.Errorf("failed to generate middleware: %w", err)
	}

	// Generate server setup
	if err := e.generateServerSetup(a, outDir); err != nil {
		return fmt.Errorf("failed to generate server: %w", err)
	}

	// Generate go.mod
	if err := e.generateGoMod(outDir); err != nil {
		return fmt.Errorf("failed to generate go.mod: %w", err)
	}

	return nil
}

// Summary returns a summary of generated files.
func (e *GoEmitter) Summary(modules []string) []emitter.SummaryLine {
	var lines []emitter.SummaryLine

	lines = append(lines, emitter.SummaryLine{
		Dir:   "internal/models/",
		Files: "types.go",
	})

	lines = append(lines, emitter.SummaryLine{
		Dir:   "internal/routes/",
		Files: "routes.go",
	})

	lines = append(lines, emitter.SummaryLine{
		Dir:   "internal/middleware/",
		Files: "errors.go, logger.go",
	})

	lines = append(lines, emitter.SummaryLine{
		Dir:   "./",
		Files: "server.go, main.go, go.mod",
	})

	return lines
}

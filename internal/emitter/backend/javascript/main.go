package javascript

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

func init() {
	emitter.RegisterBackend("javascript", New())
}

// JSEmitter generates a plain JavaScript backend from a Veld AST.
type JSEmitter struct{}

func (*JSEmitter) IsBackend() {}
func New() *JSEmitter         { return &JSEmitter{} }

// Summary returns a human-readable list of files that will be generated.
func (e *JSEmitter) Summary(modules []string) []emitter.SummaryLine {
	var lines []emitter.SummaryLine

	typeFiles := make([]string, 0, len(modules)+1)
	for _, m := range modules {
		typeFiles = append(typeFiles, strings.ToLower(m)+".js")
	}
	typeFiles = append(typeFiles, "index.js")
	lines = append(lines, emitter.SummaryLine{Dir: "types/", Files: strings.Join(typeFiles, ", ")})

	ifaceFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ifaceFiles = append(ifaceFiles, "I"+m+"Service.js")
	}
	if len(ifaceFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: "interfaces/", Files: strings.Join(ifaceFiles, ", ")})
	}

	routeFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		routeFiles = append(routeFiles, strings.ToLower(m)+".routes.js")
	}
	if len(routeFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: "routes/", Files: strings.Join(routeFiles, ", ")})
	}

	return lines
}

// Emit writes all generated files to outDir.
func (e *JSEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
	if opts.DryRun {
		return nil
	}
	if err := e.emitPerModuleTypes(a, outDir); err != nil {
		return fmt.Errorf("types: %w", err)
	}
	for _, mod := range a.Modules {
		if err := e.emitInterface(a, mod, outDir); err != nil {
			return fmt.Errorf("interface for %s: %w", mod.Name, err)
		}
		if err := e.emitRoutes(a, mod, outDir, opts); err != nil {
			return fmt.Errorf("routes for %s: %w", mod.Name, err)
		}
		if err := e.emitErrors(mod, outDir); err != nil {
			return fmt.Errorf("errors for %s: %w", mod.Name, err)
		}
	}
	if opts.Validate {
		if err := e.emitValidators(a, outDir); err != nil {
			return fmt.Errorf("validators: %w", err)
		}
	}
	if err := e.emitBarrel(a, outDir); err != nil {
		return fmt.Errorf("barrel: %w", err)
	}
	return nil
}

package node

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	nodestrategy "github.com/Adhamzineldin/Veld/internal/emitter/backend/node/strategy"
)

func init() {
	emitter.RegisterBackend("node-ts", New())
}

// NodeEmitter generates a typed Node.js backend from a Veld AST.
type NodeEmitter struct{}

func (*NodeEmitter) IsBackend() {}
func New() *NodeEmitter         { return &NodeEmitter{} }

// Summary returns a human-readable list of files that will be generated.
func (e *NodeEmitter) Summary(modules []string) []emitter.SummaryLine {
	var lines []emitter.SummaryLine

	typeFiles := make([]string, 0, len(modules)+1)
	for _, m := range modules {
		typeFiles = append(typeFiles, strings.ToLower(m)+".ts")
	}
	typeFiles = append(typeFiles, "index.ts")
	lines = append(lines, emitter.SummaryLine{Dir: "types/", Files: strings.Join(typeFiles, ", ")})

	ifaceFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ifaceFiles = append(ifaceFiles, "I"+m+"Service.ts")
	}
	if len(ifaceFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: "interfaces/", Files: strings.Join(ifaceFiles, ", ")})
	}

	routeFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		routeFiles = append(routeFiles, strings.ToLower(m)+".routes.ts")
	}
	if len(routeFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: "routes/", Files: strings.Join(routeFiles, ", ")})
	}

	return lines
}

// Emit writes all generated files to outDir.
func (e *NodeEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
	if opts.DryRun {
		return nil
	}
	strat := nodestrategy.New(opts.BackendFramework)
	if err := e.emitPerModuleTypes(a, outDir); err != nil {
		return fmt.Errorf("types: %w", err)
	}
	// Emit the shared ApiError base class before per-module error files.
	if err := e.emitErrorsBase(a, outDir); err != nil {
		return fmt.Errorf("errors base: %w", err)
	}
	for _, mod := range a.Modules {
		if err := e.emitInterface(a, mod, outDir); err != nil {
			return fmt.Errorf("interface for %s: %w", mod.Name, err)
		}
		if err := e.emitRoutes(a, mod, outDir, opts, strat); err != nil {
			return fmt.Errorf("routes for %s: %w", mod.Name, err)
		}
		if err := e.emitErrors(mod, outDir); err != nil {
			return fmt.Errorf("errors for %s: %w", mod.Name, err)
		}
	}
	// Emit errors barrel (errors/index.ts) re-exporting all module error files.
	if err := e.emitErrorsBarrel(a, outDir); err != nil {
		return fmt.Errorf("errors barrel: %w", err)
	}
	// Emit a single shared middleware interface for all modules.
	if err := e.emitMiddlewareInterface(a, outDir, strat); err != nil {
		return fmt.Errorf("middleware interface: %w", err)
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

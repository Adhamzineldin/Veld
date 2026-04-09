package csharp

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	csstrategy "github.com/Adhamzineldin/Veld/internal/emitter/backend/csharp/strategy"
)

func init() {
	emitter.RegisterBackend("csharp", New())
}

// CSharpEmitter generates a typed C# ASP.NET Core backend from a Veld AST.
// Output layout:
//
//	outDir/
//	├── SETUP.md            — how to wire this project into a host app (not README.md — reserved for veld generate index)
//	├── VeldGenerated.csproj
//	├── Models/      — one .cs per model + one per enum
//	├── Services/    — I{Module}Service.cs per module
//	└── Controllers/ — {Module}Controller.cs per module
type CSharpEmitter struct{}

func (*CSharpEmitter) IsBackend() {}
func New() *CSharpEmitter         { return &CSharpEmitter{} }

// Summary returns a human-readable list of files that will be generated.
func (e *CSharpEmitter) Summary(modules []string) []emitter.SummaryLine {
	var lines []emitter.SummaryLine

	lines = append(lines, emitter.SummaryLine{Dir: "Models/", Files: "<Model>.cs, <Enum>.cs"})

	ifaceFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ifaceFiles = append(ifaceFiles, "I"+capitalize(m)+"Service.cs")
	}
	if len(ifaceFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: "Services/", Files: strings.Join(ifaceFiles, ", ")})
	}

	ctrlFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ctrlFiles = append(ctrlFiles, capitalize(m)+"Controller.cs")
	}
	if len(ctrlFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: "Controllers/", Files: strings.Join(ctrlFiles, ", ")})
	}

	lines = append(lines, emitter.SummaryLine{Dir: "./", Files: "SETUP.md, VeldGenerated.csproj"})
	return lines
}

// capitalize returns s with its first character uppercased.
func capitalize(s string) string {
	if s == "" {
		return s
	}
	return strings.ToUpper(s[:1]) + s[1:]
}

// Emit writes all generated files to outDir.
func (e *CSharpEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
	strat := csstrategy.New(opts.BackendFramework)

	if opts.DryRun {
		return nil
	}
	if err := e.createDirs(outDir); err != nil {
		return err
	}
	if err := e.emitModels(a, outDir); err != nil {
		return fmt.Errorf("models: %w", err)
	}
	if err := e.emitAllErrors(a, outDir); err != nil {
		return fmt.Errorf("errors: %w", err)
	}
	for _, mod := range a.Modules {
		if err := e.emitInterface(a, mod, outDir); err != nil {
			return fmt.Errorf("service interface %s: %w", mod.Name, err)
		}
		if err := e.emitController(a, mod, outDir, strat); err != nil {
			return fmt.Errorf("controller %s: %w", mod.Name, err)
		}
		if err := e.emitModuleMiddleware(mod, outDir); err != nil {
			return fmt.Errorf("middleware %s: %w", mod.Name, err)
		}
	}
	if err := e.emitCsproj(outDir, strat); err != nil {
		return err
	}
	if err := e.emitConstants(a, outDir); err != nil {
		return fmt.Errorf("constants: %w", err)
	}
	return e.emitReadme(outDir)
}

func (e *CSharpEmitter) createDirs(outDir string) error {
	for _, sub := range []string{"Models", "Services", "Controllers"} {
		if err := os.MkdirAll(filepath.Join(outDir, sub), 0755); err != nil {
			return err
		}
	}
	return nil
}

func (e *CSharpEmitter) emitCsproj(outDir string, strat csstrategy.CSharpFrameworkStrategy) error {
	content := strat.ProjectFileContent()
	return os.WriteFile(filepath.Join(outDir, "VeldGenerated.csproj"), []byte(content), 0644)
}

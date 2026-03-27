package java

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

func init() {
	emitter.RegisterBackend("java", New())
}

// JavaEmitter generates a typed Java backend from a Veld AST.
// The framework-specific code (annotations, imports, response types) is
// delegated to a FrameworkStrategy — defaults to SpringStrategy (Spring Boot 3).
//
// Output layout:
//
//	outDir/
//	├── pom.xml          (or framework build file)
//	├── models/          — one .java file per model + one per enum
//	├── services/        — I{Module}Service.java per module
//	└── controllers/     — {Module}Controller.java per module
type JavaEmitter struct {
	strategy FrameworkStrategy
}

func (*JavaEmitter) IsBackend() {}

// New returns a JavaEmitter targeting Spring Boot 3.
func New() *JavaEmitter { return &JavaEmitter{strategy: &SpringStrategy{}} }

// NewWithStrategy returns a JavaEmitter using a custom framework strategy.
func NewWithStrategy(s FrameworkStrategy) *JavaEmitter { return &JavaEmitter{strategy: s} }

// Summary returns a human-readable list of files that will be generated.
func (e *JavaEmitter) Summary(modules []string) []emitter.SummaryLine {
	var lines []emitter.SummaryLine

	lines = append(lines, emitter.SummaryLine{Dir: "models/", Files: "<Model>.java, <Enum>.java"})

	ifaceFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ifaceFiles = append(ifaceFiles, "I"+capitalize(m)+"Service.java")
	}
	if len(ifaceFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: "services/", Files: strings.Join(ifaceFiles, ", ")})
	}

	ctrlFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ctrlFiles = append(ctrlFiles, capitalize(m)+"Controller.java")
	}
	if len(ctrlFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: "controllers/", Files: strings.Join(ctrlFiles, ", ")})
	}

	buildFile, _ := e.strategy.BuildFile()
	lines = append(lines, emitter.SummaryLine{Dir: "./", Files: buildFile})
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
func (e *JavaEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
	if opts.DryRun {
		return nil
	}
	if err := e.createDirs(outDir); err != nil {
		return err
	}
	if err := e.emitModels(a, outDir); err != nil {
		return fmt.Errorf("models: %w", err)
	}
	if err := e.emitErrorHandler(outDir); err != nil {
		return fmt.Errorf("error handler: %w", err)
	}
	for _, mod := range a.Modules {
		if err := e.emitInterface(a, mod, outDir); err != nil {
			return fmt.Errorf("service interface %s: %w", mod.Name, err)
		}
		if err := e.emitController(a, mod, outDir); err != nil {
			return fmt.Errorf("controller %s: %w", mod.Name, err)
		}
		if err := e.emitModuleErrors(mod, outDir); err != nil {
			return fmt.Errorf("errors %s: %w", mod.Name, err)
		}
		if err := e.emitModuleMiddleware(mod, outDir); err != nil {
			return fmt.Errorf("middleware %s: %w", mod.Name, err)
		}
	}
	if err := e.emitBuildFile(outDir); err != nil {
		return fmt.Errorf("build file: %w", err)
	}
	return nil
}

func (e *JavaEmitter) createDirs(outDir string) error {
	for _, sub := range []string{"models", "services", "controllers"} {
		if err := os.MkdirAll(filepath.Join(outDir, sub), 0755); err != nil {
			return err
		}
	}
	return nil
}

func (e *JavaEmitter) emitBuildFile(outDir string) error {
	name, content := e.strategy.BuildFile()
	return os.WriteFile(filepath.Join(outDir, name), []byte(content), 0644)
}

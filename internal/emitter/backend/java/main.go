package java

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	jstrategy "github.com/Adhamzineldin/Veld/internal/emitter/backend/java/strategy"
)

func init() {
	emitter.RegisterBackend("java", New())
}

// JavaEmitter generates a typed Java backend from a Veld AST.
// The framework strategy is resolved at emit time from EmitOptions.BackendFramework:
//
//	""/"plain"   → PlainStrategy: service interfaces + types, no HTTP framework dependency
//	"spring"     → SpringStrategy: Spring Boot 3.x controllers (no pom.xml — use build-helper-maven-plugin)
//
// Output follows standard Maven/Gradle source layout so files are picked up without
// any extra configuration:
//
//	outDir/
//	├── build.gradle          (plain only)
//	└── src/main/java/
//	    └── maayn/veld/generated/
//	        ├── models/       — <Model>.java, <Enum>.java, ApiException.java
//	        ├── services/     — I<Module>Service.java
//	        └── controllers/  — <Module>Controller.java, GlobalExceptionHandler.java
type JavaEmitter struct{}

func (*JavaEmitter) IsBackend() {}
func New() *JavaEmitter         { return &JavaEmitter{} }

// Summary returns a human-readable list of files that will be generated.
func (e *JavaEmitter) Summary(modules []string) []emitter.SummaryLine {
	const srcRoot = "src/main/java/maayn/veld/generated/"
	var lines []emitter.SummaryLine

	lines = append(lines, emitter.SummaryLine{Dir: srcRoot + "models/", Files: "<Model>.java, <Enum>.java"})

	ifaceFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ifaceFiles = append(ifaceFiles, "I"+capitalize(m)+"Service.java")
	}
	if len(ifaceFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: srcRoot + "services/", Files: strings.Join(ifaceFiles, ", ")})
	}

	ctrlFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ctrlFiles = append(ctrlFiles, capitalize(m)+"Controller.java")
	}
	if len(ctrlFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{Dir: srcRoot + "controllers/", Files: strings.Join(ctrlFiles, ", ")})
	}

	lines = append(lines, emitter.SummaryLine{Dir: "./", Files: "build.gradle (plain strategy only)"})
	return lines
}

// pkgToDir returns the filesystem directory for a Java package inside outDir,
// following Maven/Gradle standard layout: src/main/java/<pkg/path>/
func pkgToDir(outDir, pkg string) string {
	return filepath.Join(outDir, "src", "main", "java",
		filepath.FromSlash(strings.ReplaceAll(pkg, ".", "/")))
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
	strat := jstrategy.New(opts.BackendFramework)

	if opts.DryRun {
		return nil
	}
	if err := e.createDirs(outDir); err != nil {
		return err
	}
	if err := e.emitModels(a, outDir); err != nil {
		return fmt.Errorf("models: %w", err)
	}
	if err := e.emitErrorHandler(strat, outDir); err != nil {
		return fmt.Errorf("error handler: %w", err)
	}
	for _, mod := range a.Modules {
		if err := e.emitInterface(strat, a, mod, outDir); err != nil {
			return fmt.Errorf("service interface %s: %w", mod.Name, err)
		}
		if err := e.emitController(strat, a, mod, outDir); err != nil {
			return fmt.Errorf("controller %s: %w", mod.Name, err)
		}
		if err := e.emitModuleErrors(mod, outDir); err != nil {
			return fmt.Errorf("errors %s: %w", mod.Name, err)
		}
		if err := e.emitModuleMiddleware(strat, mod, outDir); err != nil {
			return fmt.Errorf("middleware %s: %w", mod.Name, err)
		}
	}
	if err := e.emitBuildFile(strat, outDir); err != nil {
		return fmt.Errorf("build file: %w", err)
	}
	return nil
}

func (e *JavaEmitter) createDirs(outDir string) error {
	for _, pkg := range []string{javaPackageModels, javaPackageServices, javaPackageControllers} {
		if err := os.MkdirAll(pkgToDir(outDir, pkg), 0755); err != nil {
			return err
		}
	}
	return nil
}

func (e *JavaEmitter) emitBuildFile(strat jstrategy.FrameworkStrategy, outDir string) error {
	name, content := strat.BuildFile()
	if name == "" {
		return nil // strategy opted out (e.g. Spring — user project owns the pom.xml)
	}
	return os.WriteFile(filepath.Join(outDir, name), []byte(content), 0644)
}

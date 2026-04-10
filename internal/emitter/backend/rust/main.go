// Package rustbackend provides a Rust backend code generator for Veld.
// Generated code uses Axum (async HTTP) and Serde (JSON serialisation) by default.
// The HTTP framework is controlled by EmitOptions.BackendFramework:
//
//	""/"plain" → PlainStrategy: trait definitions only, no HTTP framework
//	"axum"     → AxumStrategy: Axum 0.7 router + handlers
//
// Registration happens via init() — blank-import this package in cmd/veld/main.go:
//
//	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/rust"
package rustbackend

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	ruststrategy "github.com/Adhamzineldin/Veld/internal/emitter/backend/rust/strategy"
	"github.com/Adhamzineldin/Veld/internal/emitter/lang"
)

func init() {
	emitter.RegisterBackend("rust", New())
}

// RustEmitter generates a complete Rust HTTP backend from a Veld AST.
type RustEmitter struct {
	adapter lang.LanguageAdapter
}

// New creates a RustEmitter with the standard Rust language adapter.
func New() *RustEmitter {
	return &RustEmitter{adapter: &lang.RustAdapter{}}
}

// strat resolves the framework strategy from opts.
func (e *RustEmitter) strat(opts emitter.EmitOptions) ruststrategy.RustFrameworkStrategy {
	return ruststrategy.New(opts.BackendFramework)
}

// IsBackend satisfies the BackendEmitter marker interface.
func (e *RustEmitter) IsBackend() {}

// Emit generates all Rust backend files into outDir.
func (e *RustEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
	strat := e.strat(opts)

	if opts.DryRun {
		for _, line := range e.Summary(moduleNames(a.Modules)) {
			fmt.Printf("  [dry-run] %s%s\n", line.Dir, line.Files)
		}
		return nil
	}

	if err := e.createDirs(a, outDir); err != nil {
		return err
	}

	// src/models/{group}.rs + src/models/mod.rs (barrel with pub use).
	if err := e.generateTypes(a, outDir); err != nil {
		return fmt.Errorf("rust emitter [types]: %w", err)
	}

	// src/services.rs — async trait definitions.
	servicesData, err := e.generateServices(a)
	if err != nil {
		return fmt.Errorf("rust emitter [services]: %w", err)
	}
	if err := os.WriteFile(filepath.Join(outDir, "src", "services.rs"), servicesData, 0644); err != nil {
		return fmt.Errorf("rust emitter [write services.rs]: %w", err)
	}

	// src/router.rs — top-level router combining all modules.
	routerData := e.generateRouter(a, strat)
	if err := os.WriteFile(filepath.Join(outDir, "src", "router.rs"), routerData, 0644); err != nil {
		return fmt.Errorf("rust emitter [write router.rs]: %w", err)
	}

	// Per-module: src/{module}/mod.rs.
	for _, mod := range a.Modules {
		modData, err := e.generateModuleRoutes(a, mod, strat)
		if err != nil {
			return fmt.Errorf("rust emitter [routes for %s]: %w", mod.Name, err)
		}
		modName := e.adapter.NamingConvention(mod.Name, lang.NamingContextPrivate)
		filePath := filepath.Join(outDir, "src", modName, "mod.rs")
		if err := os.WriteFile(filePath, modData, 0644); err != nil {
			return fmt.Errorf("rust emitter [write %s/mod.rs]: %w", modName, err)
		}
	}

	// src/main.rs — server entry point.
	mainData := e.generateMainRs(a, strat)
	if err := os.WriteFile(filepath.Join(outDir, "src", "main.rs"), mainData, 0644); err != nil {
		return fmt.Errorf("rust emitter [write main.rs]: %w", err)
	}

	// src/errors.rs — error types with IntoResponse.
	errorsData := e.generateErrors()
	if err := os.WriteFile(filepath.Join(outDir, "src", "errors.rs"), errorsData, 0644); err != nil {
		return fmt.Errorf("rust emitter [write errors.rs]: %w", err)
	}

	// Per-module typed error constructors.
	for _, mod := range a.Modules {
		modErrors := e.generateModuleErrors(mod)
		if modErrors == nil {
			continue
		}
		modName := e.adapter.NamingConvention(mod.Name, lang.NamingContextPrivate)
		fileName := modName + "_errors.rs"
		if err := os.WriteFile(filepath.Join(outDir, "src", fileName), modErrors, 0644); err != nil {
			return fmt.Errorf("rust emitter [write %s]: %w", fileName, err)
		}
	}

	// Per-module middleware traits.
	for _, mod := range a.Modules {
		mwData := e.generateModuleMiddleware(mod)
		if mwData == nil {
			continue
		}
		modName := e.adapter.NamingConvention(mod.Name, lang.NamingContextPrivate)
		fileName := modName + "_middleware.rs"
		if err := os.WriteFile(filepath.Join(outDir, "src", fileName), mwData, 0644); err != nil {
			return fmt.Errorf("rust emitter [write %s]: %w", fileName, err)
		}
	}

	// src/lib.rs — library crate root that declares modules.
	libData := e.generateLibRs(a, false)
	if err := os.WriteFile(filepath.Join(outDir, "src", "lib.rs"), libData, 0644); err != nil {
		return fmt.Errorf("rust emitter [write lib.rs]: %w", err)
	}

	// Cargo.toml.
	cargoData := e.generateCargoToml(strat)
	if err := os.WriteFile(filepath.Join(outDir, "Cargo.toml"), cargoData, 0644); err != nil {
		return fmt.Errorf("rust emitter [write Cargo.toml]: %w", err)
	}

	if err := e.emitConstants(a, outDir); err != nil {
		return fmt.Errorf("rust emitter [constants]: %w", err)
	}

	return nil
}

// generateLibRs writes the Rust crate root that declares all modules.
func (e *RustEmitter) generateLibRs(a ast.AST, withValidation bool) []byte {
	var sb strings.Builder
	sb.WriteString(header + "\n")
	sb.WriteString("pub mod models;\n")
	sb.WriteString("pub mod services;\n")
	if withValidation {
		sb.WriteString("pub mod validation;\n")
	}
	sb.WriteString("pub mod errors;\n")
	sb.WriteString("pub mod router;\n")
	for _, mod := range a.Modules {
		modName := e.adapter.NamingConvention(mod.Name, lang.NamingContextPrivate)
		sb.WriteString(fmt.Sprintf("pub mod %s;\n", modName))
		if emitter.HasErrors(mod) {
			sb.WriteString(fmt.Sprintf("pub mod %s_errors;\n", modName))
		}
		if len(emitter.CollectModuleMiddleware(mod)) > 0 {
			sb.WriteString(fmt.Sprintf("pub mod %s_middleware;\n", modName))
		}
	}
	return []byte(sb.String())
}

// generateMainRs writes the server entry point using the strategy's content.
func (e *RustEmitter) generateMainRs(a ast.AST, strat ruststrategy.RustFrameworkStrategy) []byte {
	content := strat.MainRsContent()
	if content != "" {
		// Strategy provides full main.rs content.
		return []byte(header + "\n" + content)
	}
	// Plain strategy: emit minimal mod declarations only.
	var sb strings.Builder
	sb.WriteString(header + "\n")
	for _, mod := range a.Modules {
		modName := e.adapter.NamingConvention(mod.Name, lang.NamingContextPrivate)
		sb.WriteString(fmt.Sprintf("mod %s;\n", modName))
	}
	sb.WriteString("\nfn main() {}\n")
	return []byte(sb.String())
}

// generateCargoToml writes a Cargo.toml with dependencies from the strategy.
func (e *RustEmitter) generateCargoToml(strat ruststrategy.RustFrameworkStrategy) []byte {
	deps := strat.CargoTomlDependencies()
	var depsStr strings.Builder
	for _, d := range deps {
		depsStr.WriteString(d + "\n")
	}
	// Always include async-trait for service traits.
	if !strings.Contains(depsStr.String(), "async-trait") {
		depsStr.WriteString(`async-trait = "0.1"` + "\n")
	}
	cargo := `[package]
name = "veld-generated"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "server"
path = "src/main.rs"

[dependencies]
` + depsStr.String()
	return []byte(cargo)
}

// Summary returns a human-readable listing of generated files.
func (e *RustEmitter) Summary(modules []string) []emitter.SummaryLine {
	var lines []emitter.SummaryLine

	lines = append(lines, emitter.SummaryLine{
		Dir:   "src/",
		Files: "main.rs, lib.rs, models/mod.rs, services.rs, router.rs",
	})

	for _, m := range modules {
		modName := strings.ToLower(m)
		lines = append(lines, emitter.SummaryLine{
			Dir:   fmt.Sprintf("src/%s/", modName),
			Files: "mod.rs",
		})
	}

	lines = append(lines, emitter.SummaryLine{
		Dir:   "./",
		Files: "Cargo.toml",
	})

	return lines
}

// createDirs ensures all required output directories exist.
func (e *RustEmitter) createDirs(a ast.AST, outDir string) error {
	dirs := []string{filepath.Join(outDir, "src")}
	for _, mod := range a.Modules {
		modName := e.adapter.NamingConvention(mod.Name, lang.NamingContextPrivate)
		dirs = append(dirs, filepath.Join(outDir, "src", modName))
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

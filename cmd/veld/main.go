package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"

	// Register all emitters via init(). To add a new emitter, add one line here.
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/csharp"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/go"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/java"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/javascript"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/node"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/php"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/python"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/rust"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/angular"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/dart"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/javascript"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/kotlin"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/react"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/svelte"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/swift"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/typescript"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/typesonly"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/vue"

	// Register tool emitters (auxiliary generators — NOT backends).
	_ "github.com/Adhamzineldin/Veld/internal/generators/cicd"
	_ "github.com/Adhamzineldin/Veld/internal/generators/database"
	_ "github.com/Adhamzineldin/Veld/internal/generators/dockerfile"
	_ "github.com/Adhamzineldin/Veld/internal/generators/envconfig"
	_ "github.com/Adhamzineldin/Veld/internal/generators/scaffold"
)

var Version = "0.1.0"

// Verbosity controls output level. Set by --verbose / --quiet global flags.
var verbose bool
var quiet bool

// ── ANSI color helpers ────────────────────────────────────────────────────────

const (
	colorReset  = "\033[0m"
	colorRed    = "\033[31m"
	colorGreen  = "\033[32m"
	colorYellow = "\033[33m"
	colorDim    = "\033[2m"
	colorBold   = "\033[1m"
)

func main() {
	root := &cobra.Command{
		Use:     "veld",
		Short:   "Contract-first API code generator",
		Version: Version,
		Long: `Veld — contract-first, multi-stack API code generator.

Write .veld contracts once, generate typed frontend SDKs and backend
service interfaces for any framework. Zero runtime dependencies.

  veld init                    Scaffold a new project
  veld generate                Generate from veld.config.json
  veld generate --dry-run      Preview what would be generated
  veld watch                   Auto-regenerate on file changes
  veld validate                Check contracts for errors
  veld lint                    Analyse contract quality
  veld clean                   Remove generated output
  veld openapi                 Export OpenAPI 3.0 spec
  veld diff                    Show changes since last generation
  veld docs                    Generate API documentation
  veld fmt                     Format .veld contract files
  veld lsp                     Start the LSP server
  veld setup                   Auto-configure project imports
  veld doctor                  Diagnose project health
  veld completion              Generate shell completions

Backends:  node-ts, node-js, python, go, rust, java, csharp, php
Frontends: typescript, javascript, react, vue, angular, svelte, dart, kotlin, swift, types-only, none
Aliases:   node → node-ts, js/javascript → node-js, ts → typescript, react → react-hooks`,
		SilenceUsage:  true,
		SilenceErrors: true,
	}
	root.PersistentFlags().BoolVarP(&verbose, "verbose", "v", false, "enable verbose output")
	root.PersistentFlags().BoolVarP(&quiet, "quiet", "q", false, "suppress non-essential output")
	root.AddCommand(
		// Core workflow
		newInitCmd(), newGenerateCmd(), newWatchCmd(), newCleanCmd(),
		newValidateCmd(), newSetupCmd(), newCICmd(),
		// Quality
		newLintCmd(), newDiffCmd(), newDepsCmd(),
		// Dev tools
		newASTCmd(), newFmtCmd(),
		// Export / interop (also available as top-level aliases)
		newOpenAPICmd(), newGraphQLCmd(), newSchemaCmd(), newDocsCmd(),
		// Grouped
		newExportCmd(), newRegistryCmd(),
		// Editor / shell integration (invoked directly by external tools)
		newLSPCmd(), newCompletionCmd(),
	)
	if err := root.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, red("Error: ")+err.Error())
		os.Exit(1)
	}
}

// ── validate ──────────────────────────────────────────────────────────────────

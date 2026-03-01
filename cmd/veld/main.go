package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"time"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/cache"
	"github.com/Adhamzineldin/Veld/internal/config"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/loader"
	"github.com/Adhamzineldin/Veld/internal/lsp"
	"github.com/Adhamzineldin/Veld/internal/schema"
	"github.com/Adhamzineldin/Veld/internal/validator"
	"github.com/spf13/cobra"

	// Register all emitters via init(). To add a new emitter, add one line here.
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/csharp"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/go"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/java"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/node"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/php"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/python"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/backend/rust"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/dart"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/kotlin"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/swift"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/frontend/typescript"
)

// Version is the current Veld CLI version.
// Version is set at build time via: go build -ldflags "-X main.Version=v1.2.3"
// Falls back to "dev" for local builds without ldflags.
var Version = "0.1.0"

// ── ANSI color helpers ────────────────────────────────────────────────────────

const (
	colorReset  = "\033[0m"
	colorRed    = "\033[31m"
	colorGreen  = "\033[32m"
	colorYellow = "\033[33m"
	colorDim    = "\033[2m"
	colorBold   = "\033[1m"
)

func green(s string) string  { return colorGreen + s + colorReset }
func red(s string) string    { return colorRed + s + colorReset }
func yellow(s string) string { return colorYellow + s + colorReset }
func dim(s string) string    { return colorDim + s + colorReset }
func bold(s string) string   { return colorBold + s + colorReset }

// ── shared generation logic ───────────────────────────────────────────────────

// runGenerate parses, validates, and emits output.
//
// When incremental is false every module is regenerated.
// When incremental is true only modules whose source files changed are regenerated.
//
// Returns (regeneratedModuleNames, veldFileList, error).
func runGenerate(rc config.ResolvedConfig, incremental bool, opts emitter.EmitOptions) ([]string, []string, error) {
	a, veldFiles, err := loader.Parse(rc.Input, rc.Aliases)
	if err != nil {
		return nil, nil, err
	}
	if errs := validator.Validate(a); len(errs) > 0 {
		printValidationErrors(errs, veldFiles)
		return nil, veldFiles, fmt.Errorf("contract validation failed")
	}

	// ── incremental: compute which modules need regeneration ──────────────
	var targetModules map[string]bool
	var c *cache.Cache

	if incremental {
		c = cache.Load(rc.ConfigDir)
		changedFiles := c.ChangedFiles(veldFiles)

		if len(changedFiles) == 0 {
			return nil, veldFiles, nil
		}

		changedFileSet := make(map[string]bool, len(changedFiles))
		for _, f := range changedFiles {
			changedFileSet[f] = true
		}

		anyModelFileChanged := false
		for i := range a.Models {
			if changedFileSet[a.Models[i].SourceFile] {
				anyModelFileChanged = true
				break
			}
		}

		targetModules = make(map[string]bool)
		if anyModelFileChanged {
			for i := range a.Modules {
				targetModules[a.Modules[i].Name] = true
			}
		} else {
			for i := range a.Modules {
				if changedFileSet[a.Modules[i].SourceFile] {
					targetModules[a.Modules[i].Name] = true
				}
			}
		}

		if len(targetModules) == 0 {
			for _, f := range veldFiles {
				c.Update(f)
			}
			_ = c.Save(rc.ConfigDir)
			return nil, veldFiles, nil
		}
	}

	// ── filter AST for incremental ───────────────────────────────────────
	emitAST := a
	if targetModules != nil {
		filtered := make([]ast.Module, 0, len(targetModules))
		for _, mod := range a.Modules {
			if targetModules[mod.Name] {
				filtered = append(filtered, mod)
			}
		}
		emitAST.Modules = filtered
	}

	// ── emit: backend ────────────────────────────────────────────────────
	backend, err := emitter.GetBackend(rc.Backend)
	if err != nil {
		return nil, veldFiles, err
	}
	if err := backend.Emit(emitAST, rc.Out, opts); err != nil {
		return nil, veldFiles, fmt.Errorf("%s emitter: %w", rc.Backend, err)
	}

	// ── emit: frontend ───────────────────────────────────────────────────
	frontend, err := emitter.GetFrontend(rc.Frontend)
	if err != nil {
		return nil, veldFiles, err
	}
	if frontend != nil {
		// Frontend SDK always gets the full AST (combined output).
		if err := frontend.Emit(a, rc.Out, opts); err != nil {
			return nil, veldFiles, fmt.Errorf("%s emitter: %w", rc.Frontend, err)
		}
	}

	// ── generated/README.md ──────────────────────────────────────────────
	if !opts.DryRun {
		writeGeneratedReadme(rc.Out, emitAST)
	}

	// ── update cache ─────────────────────────────────────────────────────
	if c == nil {
		c = cache.Load(rc.ConfigDir)
	}
	for _, f := range veldFiles {
		c.Update(f)
	}
	if err := c.Save(rc.ConfigDir); err != nil {
		fmt.Fprintf(os.Stderr, yellow("warning: ")+"cache save failed: %v\n", err)
	}

	names := make([]string, 0, len(emitAST.Modules))
	for _, mod := range emitAST.Modules {
		names = append(names, mod.Name)
	}
	return names, veldFiles, nil
}

// printGenerateSummary prints a detailed breakdown of generated files
// by delegating to each emitter's Summary method.
func printGenerateSummary(rc config.ResolvedConfig, modules []string) {
	relOut := rc.Out
	if cwd, err := os.Getwd(); err == nil {
		if r, err := filepath.Rel(cwd, rc.Out); err == nil {
			relOut = "./" + filepath.ToSlash(r)
		}
	}

	fmt.Println(green("✓") + " Generated → " + bold(relOut))

	// Backend summary
	if be, err := emitter.GetBackend(rc.Backend); err == nil {
		for _, line := range be.Summary(modules) {
			fmt.Printf("  %s  %s\n", dim(line.Dir), line.Files)
		}
	}

	// Frontend summary
	if fe, err := emitter.GetFrontend(rc.Frontend); err == nil && fe != nil {
		for _, line := range fe.Summary(modules) {
			fmt.Printf("  %s  %s\n", dim(line.Dir), line.Files)
		}
	}
}

// ── main ──────────────────────────────────────────────────────────────────────

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
  veld clean                   Remove generated output
  veld openapi                 Export OpenAPI 3.0 spec
  veld graphql                 Export GraphQL SDL schema
  veld schema                  Generate database schema (Prisma/SQL)
  veld diff                    Show changes since last generation
  veld docs                    Generate API documentation
  veld lsp                     Start the LSP server`,
		SilenceUsage:  true,
		SilenceErrors: true,
	}
	root.AddCommand(
		newValidateCmd(), newASTCmd(), newGenerateCmd(), newWatchCmd(),
		newInitCmd(), newCleanCmd(), newOpenAPICmd(), newGraphQLCmd(),
		newSchemaCmd(), newDiffCmd(), newDocsCmd(), newLSPCmd(),
	)
	if err := root.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, red("Error: ")+err.Error())
		os.Exit(1)
	}
}

// ── validate ──────────────────────────────────────────────────────────────────

func newValidateCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "validate [file]",
		Short:   "Parse and validate a .veld contract file",
		Example: "  veld validate\n  veld validate veld/app.veld",
		Args:    cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			path, err := config.ResolveInput(args)
			if err != nil {
				return err
			}
			a, _, err := loader.Parse(path)
			if err != nil {
				return err
			}
			errs := validator.Validate(a)
			if len(errs) > 0 {
				for _, e := range errs {
					fmt.Fprintln(os.Stderr, red("error: ")+e.Error())
				}
				os.Exit(1)
			}
			fmt.Println(green("✓") + " Contract is valid")
			return nil
		},
	}
}

// ── ast ───────────────────────────────────────────────────────────────────────

func newASTCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "ast [file]",
		Short:   "Dump the AST JSON for a .veld contract file",
		Example: "  veld ast\n  veld ast veld/app.veld",
		Args:    cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			path, err := config.ResolveInput(args)
			if err != nil {
				return err
			}
			a, _, err := loader.Parse(path)
			if err != nil {
				return err
			}
			enc := json.NewEncoder(os.Stdout)
			enc.SetIndent("", "  ")
			return enc.Encode(a)
		},
	}
}

// ── generate ──────────────────────────────────────────────────────────────────

func newGenerateCmd() *cobra.Command {
	var backendFlag, frontendFlag, inputFlag, outFlag string
	var incrementalFlag, dryRunFlag bool

	cmd := &cobra.Command{
		Use:   "generate",
		Short: "Generate code from a .veld contract",
		Long: "Generates typed backend interfaces and a frontend SDK from your .veld contract.\n\n" +
			"Every file is (re)generated by default — deterministic and safe for CI/CD.\n" +
			"Pass --incremental to skip modules whose source files have not changed\n" +
			"(intended for local development, not production pipelines).",
		Example: "  veld generate\n" +
			"  veld generate --backend=node --frontend=typescript\n" +
			"  veld generate --dry-run",
		RunE: func(cmd *cobra.Command, args []string) error {
			flags := config.FlagOverrides{
				Backend:     backendFlag,
				Frontend:    frontendFlag,
				Input:       inputFlag,
				Out:         outFlag,
				BackendSet:  cmd.Flags().Changed("backend"),
				FrontendSet: cmd.Flags().Changed("frontend"),
				InputSet:    cmd.Flags().Changed("input"),
				OutSet:      cmd.Flags().Changed("out"),
			}
			rc, err := config.BuildResolved(flags)
			if err != nil {
				return err
			}

			opts := emitter.EmitOptions{
				BaseUrl: rc.BaseUrl,
				DryRun:  dryRunFlag,
			}

			regenerated, _, err := runGenerate(rc, incrementalFlag, opts)
			if err != nil {
				return err
			}

			if dryRunFlag {
				fmt.Println(green("✓") + " Dry run — no files written")
				printGenerateSummary(rc, regenerated)
				return nil
			}

			if incrementalFlag {
				if regenerated == nil {
					fmt.Println(green("✓") + " Nothing changed")
				} else {
					fmt.Printf(green("✓")+" Regenerated %s → %s\n",
						strings.Join(regenerated, ", "), rc.Out)
				}
				return nil
			}

			printGenerateSummary(rc, regenerated)
			return nil
		},
	}
	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend target ("+strings.Join(emitter.ListBackends(), ", ")+")")
	cmd.Flags().StringVar(&frontendFlag, "frontend", "", "frontend SDK ("+strings.Join(emitter.ListFrontends(), ", ")+", none)")
	cmd.Flags().StringVar(&inputFlag, "input", "", "input .veld file")
	cmd.Flags().StringVar(&outFlag, "out", "", "output directory")
	cmd.Flags().BoolVar(&incrementalFlag, "incremental", false,
		"skip unchanged modules (dev only — not for production builds)")
	cmd.Flags().BoolVar(&dryRunFlag, "dry-run", false,
		"preview what would be generated without writing files")
	return cmd
}

// ── watch ─────────────────────────────────────────────────────────────────────

func newWatchCmd() *cobra.Command {
	var backendFlag, frontendFlag, inputFlag, outFlag string

	cmd := &cobra.Command{
		Use:   "watch",
		Short: "Watch .veld files and auto-regenerate on change",
		Long: "Watches all .veld files for changes and incrementally regenerates only\n" +
			"the affected modules. Safe to run during development — never touches your\n" +
			"application code. Use 'veld generate' for deterministic production builds.",
		RunE: func(cmd *cobra.Command, args []string) error {
			flags := config.FlagOverrides{
				Backend:     backendFlag,
				Frontend:    frontendFlag,
				Input:       inputFlag,
				Out:         outFlag,
				BackendSet:  cmd.Flags().Changed("backend"),
				FrontendSet: cmd.Flags().Changed("frontend"),
				InputSet:    cmd.Flags().Changed("input"),
				OutSet:      cmd.Flags().Changed("out"),
			}
			rc, err := config.BuildResolved(flags)
			if err != nil {
				return err
			}

			fmt.Println(bold("veld watch") + "  •  watching for changes  •  Ctrl-C to stop")
			fmt.Println()

			opts := emitter.EmitOptions{
				BaseUrl: rc.BaseUrl,
			}

			regenerated, initFiles, genErr := runGenerate(rc, false, opts)
			if genErr != nil {
				fmt.Fprintln(os.Stderr, red("error: ")+genErr.Error())
			} else {
				fmt.Printf(green("✓")+" Ready (%d module(s)) → %s\n", len(regenerated), rc.Out)
			}
			fmt.Println()

			mtimes := make(map[string]int64, len(initFiles))
			for _, f := range initFiles {
				if info, statErr := os.Stat(f); statErr == nil {
					mtimes[f] = info.ModTime().UnixNano()
				}
			}

			ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
			defer stop()

			ticker := time.NewTicker(200 * time.Millisecond)
			defer ticker.Stop()

			lastError := false
			var debounceTimer *time.Timer

			for {
				select {
				case <-ctx.Done():
					fmt.Println("\nWatch stopped.")
					return nil

				case <-ticker.C:
					changed := false
					for f, last := range mtimes {
						info, statErr := os.Stat(f)
						if statErr != nil || info.ModTime().UnixNano() != last {
							changed = true
							break
						}
					}
					if !changed {
						continue
					}

					// Update mtimes immediately
					for f := range mtimes {
						if info, statErr := os.Stat(f); statErr == nil {
							mtimes[f] = info.ModTime().UnixNano()
						}
					}

					// Debounce: reset timer on every change, only fire after 500ms of quiet
					if debounceTimer != nil {
						debounceTimer.Stop()
					}
					debounceTimer = time.AfterFunc(500*time.Millisecond, func() {
						ts := dim("[" + time.Now().Format("15:04:05") + "]")

						regen, newFiles, genErr := runGenerate(rc, true, opts)
						if genErr != nil {
							if !lastError {
								fmt.Fprintf(os.Stderr, "%s %s %v\n", ts, red("error:"), genErr)
								fmt.Println()
							}
							lastError = true
						} else if regen == nil {
							fmt.Printf("%s %s nothing to regenerate\n", ts, green("✓"))
							fmt.Println()
							lastError = false
						} else {
							fmt.Printf("%s %s %s\n", ts, green("✓"), strings.Join(regen, ", "))
							fmt.Println()
							lastError = false
						}

						if newFiles != nil {
							mtimes = make(map[string]int64, len(newFiles))
							for _, f := range newFiles {
								if info, statErr := os.Stat(f); statErr == nil {
									mtimes[f] = info.ModTime().UnixNano()
								}
							}
						}
					})
				}
			}
		},
	}
	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend target ("+strings.Join(emitter.ListBackends(), ", ")+")")
	cmd.Flags().StringVar(&frontendFlag, "frontend", "", "frontend SDK ("+strings.Join(emitter.ListFrontends(), ", ")+", none)")
	cmd.Flags().StringVar(&inputFlag, "input", "", "input .veld file")
	cmd.Flags().StringVar(&outFlag, "out", "", "output directory")
	return cmd
}

// ── clean ─────────────────────────────────────────────────────────────────────

func newCleanCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "clean",
		Short:   "Remove the generated output directory",
		Example: "  veld clean",
		RunE: func(cmd *cobra.Command, args []string) error {
			rc, err := config.BuildResolved(config.FlagOverrides{})
			if err != nil {
				return err
			}
			if _, statErr := os.Stat(rc.Out); os.IsNotExist(statErr) {
				fmt.Println(green("✓") + " Nothing to clean — output directory does not exist")
				return nil
			}
			if err := os.RemoveAll(rc.Out); err != nil {
				return fmt.Errorf("failed to remove %s: %w", rc.Out, err)
			}
			// Also remove cache.
			cacheFile := filepath.Join(rc.ConfigDir, ".veld-cache.json")
			os.Remove(cacheFile)
			fmt.Println(green("✓") + " Cleaned " + bold(rc.Out))
			return nil
		},
	}
}

// ── openapi ───────────────────────────────────────────────────────────────────

func newOpenAPICmd() *cobra.Command {
	var outputFile string
	cmd := &cobra.Command{
		Use:     "openapi",
		Short:   "Export an OpenAPI 3.0 spec from the contract",
		Example: "  veld openapi\n  veld openapi -o openapi.json",
		RunE: func(cmd *cobra.Command, args []string) error {
			path, err := config.ResolveInput(args)
			if err != nil {
				return err
			}
			a, _, err := loader.Parse(path)
			if err != nil {
				return err
			}
			if errs := validator.Validate(a); len(errs) > 0 {
				for _, e := range errs {
					fmt.Fprintln(os.Stderr, red("error: ")+e.Error())
				}
				return fmt.Errorf("contract validation failed")
			}
			spec := buildOpenAPISpec(a)
			data, _ := json.MarshalIndent(spec, "", "  ")
			if outputFile != "" {
				if err := os.WriteFile(outputFile, data, 0644); err != nil {
					return err
				}
				fmt.Println(green("✓") + " OpenAPI spec → " + bold(outputFile))
				return nil
			}
			fmt.Println(string(data))
			return nil
		},
	}
	cmd.Flags().StringVarP(&outputFile, "output", "o", "", "write to file instead of stdout")
	return cmd
}

func buildOpenAPISpec(a ast.AST) map[string]interface{} {
	paths := make(map[string]interface{})
	for _, mod := range a.Modules {
		tag := mod.Name
		for _, act := range mod.Actions {
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			// Convert :param to {param} for OpenAPI
			oaPath := emitter.ToOpenAPIPath(routePath)
			pathParams := emitter.ExtractPathParams(routePath)

			op := map[string]interface{}{
				"tags":        []string{tag},
				"operationId": mod.Name + "_" + act.Name,
				"responses": map[string]interface{}{
					"200": map[string]interface{}{"description": "Success"},
				},
			}
			if act.Description != "" {
				op["summary"] = act.Description
			}
			if len(pathParams) > 0 {
				params := make([]map[string]interface{}, 0, len(pathParams))
				for _, p := range pathParams {
					params = append(params, map[string]interface{}{
						"name":     p,
						"in":       "path",
						"required": true,
						"schema":   map[string]interface{}{"type": "string"},
					})
				}
				op["parameters"] = params
			}
			if act.Input != "" {
				op["requestBody"] = map[string]interface{}{
					"required": true,
					"content": map[string]interface{}{
						"application/json": map[string]interface{}{
							"schema": map[string]interface{}{"$ref": "#/components/schemas/" + act.Input},
						},
					},
				}
			}
			method := strings.ToLower(act.Method)
			if _, ok := paths[oaPath]; !ok {
				paths[oaPath] = make(map[string]interface{})
			}
			paths[oaPath].(map[string]interface{})[method] = op
		}
	}

	schemas := make(map[string]interface{})
	for _, m := range a.Models {
		props := make(map[string]interface{})
		var required []string
		for _, f := range m.Fields {
			prop := map[string]interface{}{"type": oaType(f.Type)}
			if f.IsArray {
				prop = map[string]interface{}{
					"type":  "array",
					"items": map[string]interface{}{"type": oaType(f.Type)},
				}
			}
			if f.IsMap {
				prop = map[string]interface{}{
					"type":                 "object",
					"additionalProperties": map[string]interface{}{"type": oaType(f.MapValueType)},
				}
			}
			props[f.Name] = prop
			if !f.Optional {
				required = append(required, f.Name)
			}
		}
		schema := map[string]interface{}{
			"type":       "object",
			"properties": props,
		}
		if len(required) > 0 {
			schema["required"] = required
		}
		if m.Description != "" {
			schema["description"] = m.Description
		}
		schemas[m.Name] = schema
	}
	for _, en := range a.Enums {
		schemas[en.Name] = map[string]interface{}{
			"type": "string",
			"enum": en.Values,
		}
	}

	return map[string]interface{}{
		"openapi": "3.0.3",
		"info": map[string]interface{}{
			"title":   "Veld API",
			"version": "1.0.0",
		},
		"paths": paths,
		"components": map[string]interface{}{
			"schemas": schemas,
		},
	}
}

func oaType(t string) string {
	switch t {
	case "int":
		return "integer"
	case "float":
		return "number"
	case "bool":
		return "boolean"
	case "date", "datetime", "uuid", "string":
		return "string"
	default:
		return t // model reference
	}
}

// ── printErrorWithContext ─────────────────────────────────────────────────────

func printValidationErrors(errs []error, veldFiles []string) {
	// Cache file contents for context printing
	fileCache := make(map[string][]string)
	for _, f := range veldFiles {
		data, err := os.ReadFile(f)
		if err == nil {
			fileCache[filepath.Base(f)] = strings.Split(string(data), "\n")
		}
	}

	for _, e := range errs {
		msg := e.Error()
		fmt.Fprintln(os.Stderr, red("  error: ")+msg)

		// Try to extract file:line from the message
		parts := strings.SplitN(msg, ":", 3)
		if len(parts) >= 3 {
			fileName := parts[0]
			lineStr := parts[1]
			var lineNum int
			if _, err := fmt.Sscanf(lineStr, "%d", &lineNum); err == nil && lineNum > 0 {
				if lines, ok := fileCache[fileName]; ok && lineNum <= len(lines) {
					line := lines[lineNum-1]
					fmt.Fprintf(os.Stderr, "  %s │\n", dim(fmt.Sprintf("%4d", lineNum)))
					fmt.Fprintf(os.Stderr, "  %s │ %s\n", dim(fmt.Sprintf("%4d", lineNum)), line)
					fmt.Fprintf(os.Stderr, "  %s │\n", dim("    "))
				}
			}
		}
	}
}

// ── writeGeneratedReadme ─────────────────────────────────────────────────────

func writeGeneratedReadme(outDir string, a ast.AST) {
	var sb strings.Builder
	sb.WriteString("# Generated by Veld\n\n")
	sb.WriteString("> ⚠️ **DO NOT EDIT** — this entire directory is auto-generated by `veld generate`.\n")
	sb.WriteString("> Any manual changes will be overwritten on the next run.\n\n")
	sb.WriteString("## Structure\n\n")
	sb.WriteString("| Path | Description |\n")
	sb.WriteString("|------|-------------|\n")
	sb.WriteString("| `types/{module}.ts` | TypeScript interfaces and enums per module |\n")
	sb.WriteString("| `types/index.ts` | Barrel re-export of all type files |\n")
	sb.WriteString("| `interfaces/` | Service contracts (one per module) |\n")
	sb.WriteString("| `routes/` | Route registration functions with validation |\n")
	sb.WriteString("| `schemas/schemas.ts` | Zod validation schemas |\n")
	sb.WriteString("| `client/api.ts` | Frontend SDK (fetch-based, zero dependencies) |\n")
	sb.WriteString("| `index.ts` | Barrel export for clean imports |\n")
	sb.WriteString("| `package.json` | Package alias (`@veld/generated`) |\n\n")

	sb.WriteString("## Modules\n\n")
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("- **%s**", mod.Name))
		if mod.Description != "" {
			sb.WriteString(fmt.Sprintf(" — %s", mod.Description))
		}
		sb.WriteString(fmt.Sprintf(" (%d actions)\n", len(mod.Actions)))
	}

	sb.WriteString("\n## Import Alias\n\n")
	sb.WriteString("Add to your `tsconfig.json`:\n\n")
	sb.WriteString("```json\n")
	sb.WriteString("{\n")
	sb.WriteString("  \"compilerOptions\": {\n")
	sb.WriteString("    \"paths\": {\n")
	sb.WriteString("      \"@veld/*\": [\"./generated/*\"]\n")
	sb.WriteString("    }\n")
	sb.WriteString("  }\n")
	sb.WriteString("}\n")
	sb.WriteString("```\n\n")
	sb.WriteString("Then import:\n")
	sb.WriteString("```typescript\n")
	sb.WriteString("import type { User } from '@veld/types/types';\n")
	sb.WriteString("import { api } from '@veld/client/api';\n")
	sb.WriteString("```\n\n")
	sb.WriteString("## Regenerate\n\n")
	sb.WriteString("```bash\nveld generate\n```\n")

	os.WriteFile(filepath.Join(outDir, "README.md"), []byte(sb.String()), 0644)
}

// ── graphql ───────────────────────────────────────────────────────────────────

func newGraphQLCmd() *cobra.Command {
	var outputFile string
	cmd := &cobra.Command{
		Use:     "graphql",
		Short:   "Export a GraphQL SDL schema from the contract",
		Example: "  veld graphql\n  veld graphql -o schema.graphql",
		RunE: func(cmd *cobra.Command, args []string) error {
			path, err := config.ResolveInput(args)
			if err != nil {
				return err
			}
			a, _, err := loader.Parse(path)
			if err != nil {
				return err
			}
			if errs := validator.Validate(a); len(errs) > 0 {
				for _, e := range errs {
					fmt.Fprintln(os.Stderr, red("error: ")+e.Error())
				}
				return fmt.Errorf("contract validation failed")
			}
			sdl := buildGraphQLSchema(a)
			if outputFile != "" {
				if err := os.WriteFile(outputFile, []byte(sdl), 0644); err != nil {
					return err
				}
				fmt.Println(green("✓") + " GraphQL schema → " + bold(outputFile))
				return nil
			}
			fmt.Print(sdl)
			return nil
		},
	}
	cmd.Flags().StringVarP(&outputFile, "output", "o", "", "write to file instead of stdout")
	return cmd
}

func buildGraphQLSchema(a ast.AST) string {
	var sb strings.Builder

	// Enums
	for _, en := range a.Enums {
		if en.Description != "" {
			sb.WriteString(fmt.Sprintf("\"\"\"%s\"\"\"\n", en.Description))
		}
		sb.WriteString(fmt.Sprintf("enum %s {\n", en.Name))
		for _, v := range en.Values {
			sb.WriteString(fmt.Sprintf("  %s\n", v))
		}
		sb.WriteString("}\n\n")
	}

	// Types (models)
	modelMap := make(map[string]ast.Model)
	for _, m := range a.Models {
		modelMap[m.Name] = m
	}

	// Track which models are used as input
	inputModels := make(map[string]bool)
	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			if act.Input != "" {
				inputModels[act.Input] = true
			}
			if act.Query != "" {
				inputModels[act.Query] = true
			}
		}
	}

	for _, m := range a.Models {
		allFields := gqlFlattenFields(m, modelMap)
		keyword := "type"
		if inputModels[m.Name] {
			keyword = "input"
		}
		if m.Description != "" {
			sb.WriteString(fmt.Sprintf("\"\"\"%s\"\"\"\n", m.Description))
		}
		sb.WriteString(fmt.Sprintf("%s %s {\n", keyword, m.Name))
		for _, f := range allFields {
			gqlType := gqlFieldType(f)
			sb.WriteString(fmt.Sprintf("  %s: %s\n", f.Name, gqlType))
		}
		sb.WriteString("}\n\n")
	}

	// Query and Mutation
	var queries []string
	var mutations []string

	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			method := strings.ToUpper(act.Method)
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}

			// Build args
			var args []string
			for _, p := range emitter.ExtractPathParams(routePath) {
				args = append(args, fmt.Sprintf("%s: String!", p))
			}
			if act.Input != "" {
				args = append(args, fmt.Sprintf("input: %s!", act.Input))
			}
			if act.Query != "" {
				args = append(args, fmt.Sprintf("query: %s", act.Query))
			}

			argStr := ""
			if len(args) > 0 {
				argStr = "(" + strings.Join(args, ", ") + ")"
			}

			returnType := gqlReturnType(act)
			opName := lcfirst(mod.Name) + act.Name
			line := fmt.Sprintf("  %s%s: %s", opName, argStr, returnType)

			if method == "GET" {
				queries = append(queries, line)
			} else {
				mutations = append(mutations, line)
			}
		}
	}

	if len(queries) > 0 {
		sb.WriteString("type Query {\n")
		for _, q := range queries {
			sb.WriteString(q + "\n")
		}
		sb.WriteString("}\n\n")
	}

	if len(mutations) > 0 {
		sb.WriteString("type Mutation {\n")
		for _, m := range mutations {
			sb.WriteString(m + "\n")
		}
		sb.WriteString("}\n\n")
	}

	return sb.String()
}

func gqlType(t string) string {
	switch t {
	case "int":
		return "Int"
	case "float":
		return "Float"
	case "bool":
		return "Boolean"
	case "string", "date", "datetime", "uuid":
		return "String"
	default:
		return t
	}
}

func gqlFieldType(f ast.Field) string {
	if f.IsMap {
		return "String" // GraphQL doesn't have native map type
	}
	base := gqlType(f.Type)
	if f.IsArray {
		if f.Optional {
			return fmt.Sprintf("[%s]", base)
		}
		return fmt.Sprintf("[%s!]!", base)
	}
	if f.Optional {
		return base
	}
	return base + "!"
}

func gqlReturnType(act ast.Action) string {
	if act.Output == "" {
		return "Boolean"
	}
	base := gqlType(act.Output)
	if act.OutputArray {
		return fmt.Sprintf("[%s!]!", base)
	}
	return base + "!"
}

func gqlFlattenFields(m ast.Model, models map[string]ast.Model) []ast.Field {
	if m.Extends == "" {
		return m.Fields
	}
	parent, ok := models[m.Extends]
	if !ok {
		return m.Fields
	}
	return append(gqlFlattenFields(parent, models), m.Fields...)
}

func lcfirst(s string) string {
	if s == "" {
		return s
	}
	return strings.ToLower(s[:1]) + s[1:]
}

// ── schema ────────────────────────────────────────────────────────────────────

func newSchemaCmd() *cobra.Command {
	var format, outputFile string
	cmd := &cobra.Command{
		Use:     "schema",
		Short:   "Generate a database schema from the contract",
		Example: "  veld schema --format=prisma\n  veld schema --format=sql -o schema.sql",
		RunE: func(cmd *cobra.Command, args []string) error {
			path, err := config.ResolveInput(args)
			if err != nil {
				return err
			}
			a, _, err := loader.Parse(path)
			if err != nil {
				return err
			}
			if errs := validator.Validate(a); len(errs) > 0 {
				for _, e := range errs {
					fmt.Fprintln(os.Stderr, red("error: ")+e.Error())
				}
				return fmt.Errorf("contract validation failed")
			}

			var output string
			switch format {
			case "prisma":
				output = schema.BuildPrisma(a)
			case "sql":
				output = schema.BuildSQL(a)
			default:
				return fmt.Errorf("unknown schema format %q (supported: prisma, sql)", format)
			}

			if outputFile != "" {
				if err := os.WriteFile(outputFile, []byte(output), 0644); err != nil {
					return err
				}
				fmt.Println(green("✓") + " Schema → " + bold(outputFile))
				return nil
			}
			fmt.Print(output)
			return nil
		},
	}
	cmd.Flags().StringVar(&format, "format", "prisma", "output format (prisma, sql)")
	cmd.Flags().StringVarP(&outputFile, "output", "o", "", "write to file instead of stdout")
	return cmd
}

// ── diff ──────────────────────────────────────────────────────────────────────

func newDiffCmd() *cobra.Command {
	var statOnly, exitCode bool

	cmd := &cobra.Command{
		Use:   "diff",
		Short: "Show changes between current and freshly generated output",
		Long:  "Generates to a temporary directory and compares file-by-file with the\nexisting output. Useful for CI: `veld diff --exit-code` fails if stale.",
		RunE: func(cmd *cobra.Command, args []string) error {
			flags := config.FlagOverrides{}
			rc, err := config.BuildResolved(flags)
			if err != nil {
				return err
			}

			// Generate to temp dir
			tmpDir, err := os.MkdirTemp("", "veld-diff-*")
			if err != nil {
				return err
			}
			defer os.RemoveAll(tmpDir)

			a, _, err := loader.Parse(rc.Input, rc.Aliases)
			if err != nil {
				return err
			}
			if errs := validator.Validate(a); len(errs) > 0 {
				for _, e := range errs {
					fmt.Fprintln(os.Stderr, red("error: ")+e.Error())
				}
				return fmt.Errorf("contract validation failed")
			}

			opts := emitter.EmitOptions{BaseUrl: rc.BaseUrl}
			backend, err := emitter.GetBackend(rc.Backend)
			if err != nil {
				return err
			}
			if err := backend.Emit(a, tmpDir, opts); err != nil {
				return err
			}
			frontend, err := emitter.GetFrontend(rc.Frontend)
			if err != nil {
				return err
			}
			if frontend != nil {
				if err := frontend.Emit(a, tmpDir, opts); err != nil {
					return err
				}
			}

			// Compare
			added, removed, modified := 0, 0, 0
			var diffs []string

			// Walk temp dir for new/modified files
			filepath.Walk(tmpDir, func(path string, info os.FileInfo, err error) error {
				if err != nil || info.IsDir() {
					return nil
				}
				relPath, _ := filepath.Rel(tmpDir, path)
				existingPath := filepath.Join(rc.Out, relPath)

				newData, _ := os.ReadFile(path)
				existData, readErr := os.ReadFile(existingPath)

				if os.IsNotExist(readErr) {
					added++
					diffs = append(diffs, green("+ ")+relPath+" (new)")
				} else if string(newData) != string(existData) {
					modified++
					if !statOnly {
						diffs = append(diffs, yellow("~ ")+relPath+" (modified)")
						// Show simple unified diff
						oldLines := strings.Split(string(existData), "\n")
						newLines := strings.Split(string(newData), "\n")
						diffs = append(diffs, simpleDiff(oldLines, newLines, relPath)...)
					} else {
						diffs = append(diffs, yellow("~ ")+relPath)
					}
				}
				return nil
			})

			// Walk existing dir for removed files
			if _, statErr := os.Stat(rc.Out); statErr == nil {
				filepath.Walk(rc.Out, func(path string, info os.FileInfo, err error) error {
					if err != nil || info.IsDir() {
						return nil
					}
					relPath, _ := filepath.Rel(rc.Out, path)
					tmpPath := filepath.Join(tmpDir, relPath)
					if _, statErr := os.Stat(tmpPath); os.IsNotExist(statErr) {
						removed++
						diffs = append(diffs, red("- ")+relPath+" (removed)")
					}
					return nil
				})
			}

			if added == 0 && removed == 0 && modified == 0 {
				fmt.Println(green("✓") + " Generated output is up to date")
				return nil
			}

			for _, d := range diffs {
				fmt.Println(d)
			}
			fmt.Printf("\n%d added, %d modified, %d removed\n", added, modified, removed)

			if exitCode {
				os.Exit(1)
			}
			return nil
		},
	}
	cmd.Flags().BoolVar(&statOnly, "stat", false, "show summary only (files changed/added/removed)")
	cmd.Flags().BoolVar(&exitCode, "exit-code", false, "exit with code 1 if changes detected (useful for CI)")
	return cmd
}

func simpleDiff(oldLines, newLines []string, filename string) []string {
	var result []string
	result = append(result, dim(fmt.Sprintf("--- a/%s", filename)))
	result = append(result, dim(fmt.Sprintf("+++ b/%s", filename)))

	maxLen := len(oldLines)
	if len(newLines) > maxLen {
		maxLen = len(newLines)
	}

	for i := 0; i < maxLen; i++ {
		oldLine := ""
		newLine := ""
		if i < len(oldLines) {
			oldLine = oldLines[i]
		}
		if i < len(newLines) {
			newLine = newLines[i]
		}
		if oldLine != newLine {
			if i < len(oldLines) {
				result = append(result, red("-"+oldLine))
			}
			if i < len(newLines) {
				result = append(result, green("+"+newLine))
			}
		}
	}
	return result
}

// ── docs ──────────────────────────────────────────────────────────────────────

func newDocsCmd() *cobra.Command {
	var format, outputFile string
	cmd := &cobra.Command{
		Use:     "docs",
		Short:   "Generate API documentation from the contract",
		Example: "  veld docs\n  veld docs --format=html -o docs.html\n  veld docs --format=markdown",
		RunE: func(cmd *cobra.Command, args []string) error {
			path, err := config.ResolveInput(args)
			if err != nil {
				return err
			}
			a, _, err := loader.Parse(path)
			if err != nil {
				return err
			}
			if errs := validator.Validate(a); len(errs) > 0 {
				for _, e := range errs {
					fmt.Fprintln(os.Stderr, red("error: ")+e.Error())
				}
				return fmt.Errorf("contract validation failed")
			}

			var output string
			switch format {
			case "html":
				output = buildDocsHTML(a)
			case "markdown", "md":
				output = buildDocsMarkdown(a)
			default:
				return fmt.Errorf("unknown docs format %q (supported: html, markdown)", format)
			}

			if outputFile != "" {
				if err := os.WriteFile(outputFile, []byte(output), 0644); err != nil {
					return err
				}
				fmt.Println(green("✓") + " Docs → " + bold(outputFile))
				return nil
			}
			fmt.Print(output)
			return nil
		},
	}
	cmd.Flags().StringVar(&format, "format", "html", "output format (html, markdown)")
	cmd.Flags().StringVarP(&outputFile, "output", "o", "", "write to file instead of stdout")
	return cmd
}

func buildDocsMarkdown(a ast.AST) string {
	var sb strings.Builder
	sb.WriteString("# API Documentation\n\n")
	sb.WriteString("*Generated by Veld*\n\n")

	// Table of contents
	sb.WriteString("## Modules\n\n")
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("- [%s](#%s)\n", mod.Name, strings.ToLower(mod.Name)))
	}
	sb.WriteString("\n")

	// Modules
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("## %s\n\n", mod.Name))
		if mod.Description != "" {
			sb.WriteString(mod.Description + "\n\n")
		}

		sb.WriteString("| Method | Path | Name | Description |\n")
		sb.WriteString("|--------|------|------|-------------|\n")
		for _, act := range mod.Actions {
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			desc := act.Description
			sb.WriteString(fmt.Sprintf("| `%s` | `%s` | %s | %s |\n", act.Method, routePath, act.Name, desc))
		}
		sb.WriteString("\n")
	}

	// Models
	sb.WriteString("## Models\n\n")
	for _, m := range a.Models {
		sb.WriteString(fmt.Sprintf("### %s\n\n", m.Name))
		if m.Description != "" {
			sb.WriteString(m.Description + "\n\n")
		}
		if m.Extends != "" {
			sb.WriteString(fmt.Sprintf("*Extends %s*\n\n", m.Extends))
		}

		sb.WriteString("| Field | Type | Optional | Default |\n")
		sb.WriteString("|-------|------|----------|---------|\n")
		for _, f := range m.Fields {
			typeName := f.Type
			if f.IsArray {
				typeName += "[]"
			}
			if f.IsMap {
				typeName = fmt.Sprintf("Map<string, %s>", f.MapValueType)
			}
			opt := ""
			if f.Optional {
				opt = "yes"
			}
			sb.WriteString(fmt.Sprintf("| %s | `%s` | %s | %s |\n", f.Name, typeName, opt, f.Default))
		}
		sb.WriteString("\n")
	}

	// Enums
	if len(a.Enums) > 0 {
		sb.WriteString("## Enums\n\n")
		for _, en := range a.Enums {
			sb.WriteString(fmt.Sprintf("### %s\n\n", en.Name))
			if en.Description != "" {
				sb.WriteString(en.Description + "\n\n")
			}
			sb.WriteString("Values: `" + strings.Join(en.Values, "`, `") + "`\n\n")
		}
	}

	return sb.String()
}

func buildDocsHTML(a ast.AST) string {
	var sb strings.Builder

	sb.WriteString(`<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>API Documentation — Veld</title>
<style>
:root {
  --bg: #ffffff; --fg: #1a1a2e; --sidebar-bg: #f8f9fa; --border: #e0e0e0;
  --accent: #6c5ce7; --accent-light: #a29bfe; --card-bg: #ffffff;
  --method-get: #00b894; --method-post: #0984e3; --method-put: #fdcb6e;
  --method-delete: #d63031; --method-patch: #e17055; --method-ws: #6c5ce7;
  --code-bg: #f1f2f6;
}
[data-theme="dark"] {
  --bg: #1a1a2e; --fg: #e0e0e0; --sidebar-bg: #16213e; --border: #2a2a4a;
  --accent: #a29bfe; --accent-light: #6c5ce7; --card-bg: #16213e;
  --code-bg: #0f3460;
}
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: var(--bg); color: var(--fg); display: flex; min-height: 100vh; }
.sidebar { width: 260px; background: var(--sidebar-bg); border-right: 1px solid var(--border); padding: 24px 16px; position: fixed; height: 100vh; overflow-y: auto; }
.sidebar h2 { font-size: 18px; margin-bottom: 16px; color: var(--accent); }
.sidebar a { display: block; padding: 6px 12px; color: var(--fg); text-decoration: none; border-radius: 6px; margin-bottom: 2px; font-size: 14px; }
.sidebar a:hover { background: var(--accent); color: white; }
.main { margin-left: 260px; padding: 40px; max-width: 900px; width: 100%; }
h1 { font-size: 28px; margin-bottom: 8px; }
h2 { font-size: 22px; margin: 32px 0 12px; border-bottom: 2px solid var(--accent); padding-bottom: 4px; }
h3 { font-size: 18px; margin: 24px 0 8px; }
table { width: 100%; border-collapse: collapse; margin: 12px 0; }
th, td { text-align: left; padding: 8px 12px; border-bottom: 1px solid var(--border); font-size: 14px; }
th { background: var(--sidebar-bg); font-weight: 600; }
.method { display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 12px; font-weight: 700; color: white; }
.method-GET { background: var(--method-get); } .method-POST { background: var(--method-post); }
.method-PUT { background: var(--method-put); color: #333; } .method-DELETE { background: var(--method-delete); }
.method-PATCH { background: var(--method-patch); } .method-WS { background: var(--method-ws); }
code { background: var(--code-bg); padding: 2px 6px; border-radius: 3px; font-size: 13px; }
.desc { color: #888; font-size: 14px; margin: 4px 0 16px; }
.toggle { position: fixed; top: 16px; right: 16px; background: var(--accent); color: white; border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-size: 13px; z-index: 10; }
#search { width: 100%; padding: 8px 12px; border: 1px solid var(--border); border-radius: 6px; margin-bottom: 16px; font-size: 14px; background: var(--bg); color: var(--fg); }
</style>
</head>
<body>
<button class="toggle" onclick="toggleTheme()">Toggle Dark Mode</button>
<nav class="sidebar">
<h2>API Docs</h2>
<input type="text" id="search" placeholder="Search..." oninput="filterNav(this.value)">
`)

	// Sidebar links
	sb.WriteString("<div id=\"nav-links\">\n")
	sb.WriteString("<strong>Modules</strong>\n")
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("<a href=\"#mod-%s\" class=\"nav-link\">%s</a>\n", strings.ToLower(mod.Name), mod.Name))
	}
	sb.WriteString("<br><strong>Models</strong>\n")
	for _, m := range a.Models {
		sb.WriteString(fmt.Sprintf("<a href=\"#model-%s\" class=\"nav-link\">%s</a>\n", strings.ToLower(m.Name), m.Name))
	}
	if len(a.Enums) > 0 {
		sb.WriteString("<br><strong>Enums</strong>\n")
		for _, en := range a.Enums {
			sb.WriteString(fmt.Sprintf("<a href=\"#enum-%s\" class=\"nav-link\">%s</a>\n", strings.ToLower(en.Name), en.Name))
		}
	}
	sb.WriteString("</div>\n</nav>\n<main class=\"main\">\n")

	sb.WriteString("<h1>API Documentation</h1>\n<p class=\"desc\">Generated by Veld</p>\n\n")

	// Modules
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("<h2 id=\"mod-%s\">%s</h2>\n", strings.ToLower(mod.Name), mod.Name))
		if mod.Description != "" {
			sb.WriteString(fmt.Sprintf("<p class=\"desc\">%s</p>\n", mod.Description))
		}
		sb.WriteString("<table><thead><tr><th>Method</th><th>Path</th><th>Name</th><th>Input</th><th>Output</th><th>Description</th></tr></thead><tbody>\n")
		for _, act := range mod.Actions {
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			output := act.Output
			if act.OutputArray {
				output += "[]"
			}
			if output == "" {
				output = "void"
			}
			input := act.Input
			if input == "" {
				input = "—"
			}
			desc := act.Description
			sb.WriteString(fmt.Sprintf("<tr><td><span class=\"method method-%s\">%s</span></td><td><code>%s</code></td><td>%s</td><td><code>%s</code></td><td><code>%s</code></td><td>%s</td></tr>\n",
				act.Method, act.Method, routePath, act.Name, input, output, desc))
		}
		sb.WriteString("</tbody></table>\n\n")
	}

	// Models
	sb.WriteString("<h2>Models</h2>\n")
	for _, m := range a.Models {
		sb.WriteString(fmt.Sprintf("<h3 id=\"model-%s\">%s", strings.ToLower(m.Name), m.Name))
		if m.Extends != "" {
			sb.WriteString(fmt.Sprintf(" <small>extends %s</small>", m.Extends))
		}
		sb.WriteString("</h3>\n")
		if m.Description != "" {
			sb.WriteString(fmt.Sprintf("<p class=\"desc\">%s</p>\n", m.Description))
		}
		sb.WriteString("<table><thead><tr><th>Field</th><th>Type</th><th>Optional</th><th>Default</th></tr></thead><tbody>\n")
		for _, f := range m.Fields {
			typeName := f.Type
			if f.IsArray {
				typeName += "[]"
			}
			if f.IsMap {
				typeName = fmt.Sprintf("Map&lt;string, %s&gt;", f.MapValueType)
			}
			opt := ""
			if f.Optional {
				opt = "yes"
			}
			sb.WriteString(fmt.Sprintf("<tr><td>%s</td><td><code>%s</code></td><td>%s</td><td>%s</td></tr>\n", f.Name, typeName, opt, f.Default))
		}
		sb.WriteString("</tbody></table>\n\n")
	}

	// Enums
	if len(a.Enums) > 0 {
		sb.WriteString("<h2>Enums</h2>\n")
		for _, en := range a.Enums {
			sb.WriteString(fmt.Sprintf("<h3 id=\"enum-%s\">%s</h3>\n", strings.ToLower(en.Name), en.Name))
			if en.Description != "" {
				sb.WriteString(fmt.Sprintf("<p class=\"desc\">%s</p>\n", en.Description))
			}
			sb.WriteString("<p>Values: ")
			for i, v := range en.Values {
				if i > 0 {
					sb.WriteString(", ")
				}
				sb.WriteString(fmt.Sprintf("<code>%s</code>", v))
			}
			sb.WriteString("</p>\n\n")
		}
	}

	sb.WriteString(`</main>
<script>
function toggleTheme() {
  document.documentElement.toggleAttribute('data-theme',
    !document.documentElement.hasAttribute('data-theme'));
  document.documentElement.setAttribute('data-theme',
    document.documentElement.hasAttribute('data-theme') ? 'dark' : '');
}
function filterNav(q) {
  q = q.toLowerCase();
  document.querySelectorAll('.nav-link').forEach(a => {
    a.style.display = a.textContent.toLowerCase().includes(q) ? '' : 'none';
  });
}
</script>
</body>
</html>
`)

	return sb.String()
}

// ── lsp (placeholder) ────────────────────────────────────────────────────────

func newLSPCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "lsp",
		Short: "Start the Veld LSP server (stdin/stdout)",
		RunE: func(cmd *cobra.Command, args []string) error {
			return runLSPServer()
		},
	}
}

func runLSPServer() error {
	server := lsp.NewServer()
	return server.Run()
}

// ── init ──────────────────────────────────────────────────────────────────────

func newInitCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "init",
		Short:   "Scaffold a new Veld project in the current directory",
		Example: "  mkdir my-api && cd my-api && veld init",
		RunE:    func(cmd *cobra.Command, args []string) error { return runInit() },
	}
}

func runInit() error {
	for _, p := range []string{"veld/veld.config.json", "veld.config.json"} {
		if _, err := os.Stat(p); err == nil {
			fmt.Fprintln(os.Stderr, red("Error:")+" veld project already initialized in this directory")
			os.Exit(1)
		}
	}

	type entry struct{ path, content, label string }
	files := []entry{
		{"veld/veld.config.json", veldConfigContent, "veld/veld.config.json"},
		{"veld/app.veld", appVeldContent, "veld/app.veld"},
		{"veld/models/user.veld", modelsUserVeldContent, "veld/models/user.veld"},
		{"veld/models/auth.veld", modelsAuthModelContent, "veld/models/auth.veld"},
		{"veld/models/common.veld", modelsCommonVeldContent, "veld/models/common.veld"},
		{"veld/modules/users.veld", modulesUsersVeldContent, "veld/modules/users.veld"},
		{"veld/modules/auth.veld", modulesAuthVeldContent, "veld/modules/auth.veld"},
		{"README.md", initReadmeContent, "README.md"},
	}

	for _, f := range files {
		if err := os.MkdirAll(filepath.Dir(f.path), 0755); err != nil {
			return err
		}
		if err := os.WriteFile(f.path, []byte(f.content), 0644); err != nil {
			return err
		}
		fmt.Printf(green("✓")+" Created %s\n", f.label)
	}

	fmt.Println()
	fmt.Println("  " + bold("Veld project ready."))
	fmt.Println()
	fmt.Println("  Next steps:")
	fmt.Println("    1. Edit veld/models/ and veld/modules/ to define your API")
	fmt.Println("    2. Run: " + bold("veld generate"))
	return nil
}

// ── init templates ────────────────────────────────────────────────────────────

const veldConfigContent = `{
  "input": "app.veld",
  "backend": "node",
  "frontend": "typescript",
  "out": "../generated",
  "aliases": {
    "models": "models",
    "modules": "modules"
  }
}
`

const appVeldContent = `// ── Veld Entry Point ─────────────────────────────────────────────────
//
// This file is the root of your Veld contract. It imports all modules.
// Each module file imports the model files it needs.
//
// How it works:
//   1. Define data types in veld/models/ (models, enums)
//   2. Define API endpoints in veld/modules/ (modules with actions)
//   3. Run "veld generate" to produce typed code in generated/
//
// Import syntax:
//   import @models/user       → loads veld/models/user.veld
//   import @modules/auth      → loads veld/modules/auth.veld
//   import @models/*          → loads all .veld files in veld/models/
//
// Every file must explicitly import the files that define the types it
// uses. Veld will error if a type is referenced but not imported.
//
// Middleware names (like RequireAuth) are just labels — you provide the
// actual middleware functions when you register routes in your app.
//
// Run "veld validate" at any time to check your contract for errors.
// ─────────────────────────────────────────────────────────────────────

import @modules/users
import @modules/auth
`

const modelsUserVeldContent = `// User domain models and enums.

enum UserRole {
  admin
  user
  guest
}

model User {
  description: "A platform user"
  id:        uuid
  email:     string
  name:      string
  bio?:      string
  role:      UserRole   @default(user)
  verified:  bool       @default(false)
  createdAt: datetime
}

model CreateUserInput {
  description: "Data required to create a new user"
  email:    string
  name:     string
  password: string
}

model UpdateUserInput {
  description: "Fields that can be updated on a user"
  name?: string
  bio?:  string
  role?: UserRole
}
`

const modelsAuthModelContent = `// Authentication request and response models.

import @models/user

model LoginInput {
  description: "Credentials for user login"
  email:    string
  password: string
}

model RegisterInput {
  description: "Data for new account registration"
  email:    string
  name:     string
  password: string
}

model AuthToken {
  description: "Token returned after successful authentication"
  token: string
  user:  User
}
`

const modelsCommonVeldContent = `// Shared types used across multiple modules.

model SuccessMessage {
  description: "Generic success response"
  success: bool
  message?: string
}

model ListQuery {
  description: "Common query parameters for list endpoints"
  search?: string
  limit?:  int
  offset?: int
}
`

const modulesUsersVeldContent = `// Users module — CRUD endpoints for user management.

import @models/user
import @models/common

module Users {
  description: "User management"
  prefix:      /api/users

  action ListUsers {
    description: "List all users with optional filters"
    method:      GET
    path:        /
    query:       ListQuery
    output:      User[]
  }

  action GetUser {
    description: "Get a single user by ID"
    method:      GET
    path:        /:id
    output:      User
  }

  action CreateUser {
    description: "Create a new user"
    method:      POST
    path:        /
    input:       CreateUserInput
    output:      User
  }

  action UpdateUser {
    description: "Update an existing user"
    method:      PUT
    path:        /:id
    input:       UpdateUserInput
    output:      User
  }

  action DeleteUser {
    description: "Delete a user"
    method:      DELETE
    path:        /:id
    output:      SuccessMessage
  }
}
`

const modulesAuthVeldContent = `// Auth module — authentication and session management.
// Middleware names are labels — you provide the actual functions at runtime.

import @models/user
import @models/auth
import @models/common

module Auth {
  description: "Authentication and session management"
  prefix:      /api/auth

  action Login {
    description: "Log in with credentials"
    method:      POST
    path:        /login
    input:       LoginInput
    output:      AuthToken
    middleware:   RateLimit
  }

  action Register {
    description: "Register a new account"
    method:      POST
    path:        /register
    input:       RegisterInput
    output:      AuthToken
    middleware:   RateLimit
  }

  action GetMe {
    description: "Get the currently authenticated user"
    method:      GET
    path:        /me
    output:      User
    middleware:   RequireAuth
  }

  action Logout {
    description: "Log out and invalidate session"
    method:      POST
    path:        /logout
    output:      SuccessMessage
    middleware:   RequireAuth
  }
}
`

const initReadmeContent = "# My Veld Project\n\n" +
	"## Structure\n\n" +
	"| Path | Purpose |\n" +
	"|------|--------|\n" +
	"| `veld/` | Contract source — models, modules, config |\n" +
	"| `veld/models/` | Data type definitions (models, enums) |\n" +
	"| `veld/modules/` | API endpoint definitions |\n" +
	"| `generated/` | Auto-generated code — do not edit |\n\n" +
	"## Import System\n\n" +
	"Every file must explicitly import the files that define the types it uses:\n\n" +
	"```veld\n" +
	"// veld/app.veld — imports modules\n" +
	"import @modules/users\n" +
	"import @modules/auth\n" +
	"```\n\n" +
	"```veld\n" +
	"// veld/modules/users.veld — imports its own models\n" +
	"import @models/user\n" +
	"import @models/common\n\n" +
	"module Users { ... }\n" +
	"```\n\n" +
	"Import paths don't include `.veld` — the parser adds it automatically.\n\n" +
	"## Middleware\n\n" +
	"Middleware names (like `RequireAuth`, `RateLimit`) are just labels in the contract.\n" +
	"Veld generates typed middleware interfaces — you provide the implementations\n" +
	"when registering routes in your app.\n\n" +
	"## Commands\n\n" +
	"| Command | Description |\n" +
	"|---------|-------------|\n" +
	"| `veld generate` | Generate typed code |\n" +
	"| `veld validate` | Check contract for errors |\n" +
	"| `veld watch` | Auto-regenerate on file save |\n" +
	"| `veld clean` | Remove generated output |\n" +
	"| `veld openapi` | Export OpenAPI 3.0 spec |\n" +
	"| `veld ast` | Dump AST JSON for debugging |\n"

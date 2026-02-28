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

	"github.com/spf13/cobra"
	"github.com/veld-dev/veld/internal/ast"
	"github.com/veld-dev/veld/internal/cache"
	"github.com/veld-dev/veld/internal/config"
	"github.com/veld-dev/veld/internal/emitter"
	"github.com/veld-dev/veld/internal/loader"
	"github.com/veld-dev/veld/internal/validator"

	// Register all emitters via init(). To add a new emitter, add one line here.
	_ "github.com/veld-dev/veld/internal/emitter/backend/node"
	_ "github.com/veld-dev/veld/internal/emitter/backend/python"
	_ "github.com/veld-dev/veld/internal/emitter/frontend/typescript"
)

// Version is the current Veld CLI version.
const Version = "0.1.0"

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
	a, veldFiles, err := loader.Parse(rc.Input)
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
  veld openapi                 Export OpenAPI 3.0 spec`,
		SilenceUsage:  true,
		SilenceErrors: true,
	}
	root.AddCommand(
		newValidateCmd(), newASTCmd(), newGenerateCmd(), newWatchCmd(),
		newInitCmd(), newCleanCmd(), newOpenAPICmd(),
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
	sb.WriteString("| `types/types.ts` | All TypeScript interfaces and enum types |\n")
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
  "out": "../generated"
}
`

const appVeldContent = `// Entry point — imports all models and modules.
// Add new import lines here as your API grows.

import "models/user.veld"
import "models/common.veld"
import "modules/users.veld"
import "modules/auth.veld"
`

const modelsUserVeldContent = `// User-related data models.
// Demonstrates: enums, optional fields, descriptions, all scalar types.

enum Role {
  admin
  user
  guest
}

enum Status {
  active
  inactive
  pending
}

model User {
  description: "Represents an authenticated platform user"
  id:         uuid
  email:      string
  name:       string
  bio?:       string
  avatarUrl?: string
  role:       Role        @default(user)
  status:     Status      @default(active)
  age?:       int
  rating?:    float
  verified:   bool        @default(false)
  createdAt:  datetime
  birthDate?: date
}

model LoginInput {
  email:    string
  password: string
}

model RegisterInput {
  email:    string
  password: string
  name:     string
}

model AuthResponse {
  description: "Returned after successful login or registration"
  token: string
  user:  User
}

model SuccessResponse {
  success: bool
  message?: string
}
`

const modelsCommonVeldContent = `// Shared/common types used across multiple modules.
// Demonstrates: generic-style patterns, optional fields.

model PaginatedResponse {
  description: "Wraps any list response with pagination metadata"
  total:    int
  page:     int
  pageSize: int
  hasMore:  bool
}

model ErrorResponse {
  description: "Standard error envelope returned by all endpoints"
  code:     int
  message:  string
  details?: string
}

model UserFilters {
  description: "Query parameters for filtering user lists"
  role?:    string
  status?:  string
  search?:  string
  limit?:   int
  offset?:  int
}
`

const modulesUsersVeldContent = `// Users module — CRUD and listing endpoints.
// Demonstrates: query params, description, various HTTP methods, output arrays.

module Users {
  description: "User management and lookup"
  prefix:      /api

  action List {
    description: "List users with optional filters"
    method:      GET
    path:        /users
    query:       UserFilters
    output:      User[]
    middleware:   AuthGuard
  }

  action GetById {
    description: "Get a single user by ID"
    method:      GET
    path:        /users/:id
    output:      User
    middleware:   AuthGuard
  }

  action Update {
    description: "Update an existing user"
    method:      PUT
    path:        /users/:id
    input:       RegisterInput
    output:      User
    middleware:   AuthGuard
  }

  action Delete {
    description: "Soft-delete a user"
    method:      DELETE
    path:        /users/:id
    output:      SuccessResponse
    middleware:   AuthGuard
  }
}
`

const modulesAuthVeldContent = `// Auth module — authentication and session management.
// Demonstrates: middleware, descriptions, POST/GET patterns.

module Auth {
  description: "Authentication and session management"

  action Login {
    description: "Exchange credentials for a session token"
    method:      POST
    path:        /auth/login
    input:       LoginInput
    output:      AuthResponse
    middleware:   RateLimit
  }

  action Register {
    description: "Create a new user account"
    method:      POST
    path:        /auth/register
    input:       RegisterInput
    output:      AuthResponse
  }

  action Me {
    description: "Get the currently authenticated user"
    method:      GET
    path:        /auth/me
    output:      User
    middleware:   AuthGuard
  }

  action Logout {
    description: "Invalidate the current session"
    method:      POST
    path:        /auth/logout
    output:      SuccessResponse
    middleware:   AuthGuard
  }
}
`

const initReadmeContent = "# My Veld Project\n\n" +
	"## Structure\n\n" +
	"| Path | Owner | Purpose |\n" +
	"|------|-------|--------|\n" +
	"| `veld/` | You | Contract source — models, modules, config |\n" +
	"| `veld/models/` | You | Data type definitions (models, enums) |\n" +
	"| `veld/modules/` | You | API endpoint definitions |\n" +
	"| `generated/` | Veld | Auto-generated — do not edit |\n\n" +
	"## Features\n\n" +
	"- **Enums** — `enum Role { admin user guest }`\n" +
	"- **Optional fields** — `bio?: string`\n" +
	"- **Descriptions** — `description: \"...\"`  → JSDoc/docstrings\n" +
	"- **Query parameters** — `query: UserFilters`\n" +
	"- **Default values** — `role: Role @default(user)`\n" +
	"- **Route prefixes** — `prefix: /api`\n" +
	"- **Array types** — `tags: string[]`, `output: User[]`\n" +
	"- **Map types** — `metadata: Map<string, string>` → `Record<string, string>` / `Dict[str, str]`\n" +
	"- **Model inheritance** — `model Admin extends User { ... }`\n" +
	"- **Rich scalars** — `string`, `int`, `float`, `bool`, `date`, `datetime`, `uuid`\n" +
	"- **Zod schemas** — auto-generated validation wired into route handlers\n" +
	"- **Pydantic schemas** — auto-generated for Python backends\n" +
	"- **Error handling** — try/catch in all route handlers, proper HTTP status codes\n" +
	"- **OpenAPI export** — `veld openapi -o openapi.json`\n" +
	"- **Import aliases** — `@veld/generated` package.json for clean imports\n\n" +
	"## Workflow\n\n" +
	"1. Edit files in `veld/models/` and `veld/modules/`\n" +
	"2. Run `veld generate` to regenerate `generated/`\n" +
	"3. Implement interfaces in your service layer\n" +
	"4. Import the SDK in your frontend from `generated/client/api.ts`\n\n" +
	"## Import system\n\n" +
	"Split your contract across as many files as you like:\n\n" +
	"```\n" +
	"// veld/app.veld\n" +
	"import \"models/user.veld\"\n" +
	"import \"models/common.veld\"\n" +
	"import \"modules/auth.veld\"\n" +
	"import \"modules/users.veld\"\n" +
	"```\n\n" +
	"## Commands\n\n" +
	"| Command | Description |\n" +
	"|---------|-------------|\n" +
	"| `veld generate` | Full regeneration (safe for CI/CD) |\n" +
	"| `veld generate --incremental` | Regenerate changed modules only (dev) |\n" +
	"| `veld watch` | Auto-regenerate on file save (dev) |\n" +
	"| `veld validate` | Check contract for errors |\n" +
	"| `veld clean` | Remove generated output |\n" +
	"| `veld openapi` | Export OpenAPI 3.0 spec |\n" +
	"| `veld ast` | Dump AST JSON for debugging |\n" +
	"| `veld init` | Scaffold a new project |\n"

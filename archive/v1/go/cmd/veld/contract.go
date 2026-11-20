package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/config"
	"github.com/Adhamzineldin/Veld/internal/docsgen"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	vfmt "github.com/Adhamzineldin/Veld/internal/format"
	"github.com/Adhamzineldin/Veld/internal/graphqlgen"
	"github.com/Adhamzineldin/Veld/internal/lint"
	"github.com/Adhamzineldin/Veld/internal/loader"
	"github.com/Adhamzineldin/Veld/internal/schema"
	"github.com/Adhamzineldin/Veld/internal/validator"
	"github.com/spf13/cobra"

	// Register all emitters via init(). To add a new emitter, add one line here.

	// Register tool emitters (auxiliary generators — NOT backends).
	openapigen "github.com/Adhamzineldin/Veld/internal/generators/openapi"
)

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
		Hidden:  true,
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

func newLintCmd() *cobra.Command {
	var exitCodeFlag bool

	cmd := &cobra.Command{
		Use:   "lint",
		Short: "Analyse the contract for quality issues",
		Long: "Runs static analysis on your .veld contract and reports warnings and errors.\n" +
			"Unlike 'veld validate' (which checks structural correctness), 'veld lint'\n" +
			"flags patterns that are legal but likely unintentional — unused models,\n" +
			"empty modules, duplicate routes, missing descriptions, and more.\n\n" +
			"Exits 0 when no issues are found. Use --exit-code to fail on any finding.",
		Example: "  veld lint\n  veld lint --exit-code",
		RunE: func(cmd *cobra.Command, args []string) error {
			rc, err := config.BuildResolved(config.FlagOverrides{})
			if err != nil {
				return err
			}

			a, _, err := loader.Parse(rc.Input, rc.Aliases)
			if err != nil {
				return err
			}
			if errs := validator.Validate(a); len(errs) > 0 {
				printValidationErrors(errs, nil)
				return fmt.Errorf("contract validation failed — fix errors before linting")
			}

			issues := lint.Lint(a)

			if len(issues) == 0 {
				fmt.Println(green("✓") + " No issues found")
				return nil
			}

			// Print errors first (already sorted by lint.Lint), then warnings.
			errCount, warnCount := 0, 0
			for _, iss := range issues {
				if iss.IsError() {
					fmt.Printf("  %s  [%s]  %s  %s\n",
						red("✗"), red(iss.Rule), bold(iss.Path), iss.Message)
					errCount++
				} else {
					fmt.Printf("  %s  [%s]  %s  %s\n",
						yellow("⚠"), yellow(iss.Rule), dim(iss.Path), iss.Message)
					warnCount++
				}
			}

			fmt.Println()
			parts := []string{}
			if errCount > 0 {
				parts = append(parts, fmt.Sprintf("%s %d error(s)", red("✗"), errCount))
			}
			if warnCount > 0 {
				parts = append(parts, fmt.Sprintf("%s %d warning(s)", yellow("⚠"), warnCount))
			}
			fmt.Println(strings.Join(parts, "  "))

			if exitCodeFlag || lint.HasErrors(issues) {
				return fmt.Errorf("lint found %d issue(s)", len(issues))
			}
			return nil
		},
	}
	cmd.Flags().BoolVar(&exitCodeFlag, "exit-code", false,
		"exit with a non-zero status if any issues (including warnings) are found")
	return cmd
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
			a = emitter.ApplyTopLevelPrefix(a)
			spec := openapigen.BuildSpec(a)
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

// ── printErrorWithContext ─────────────────────────────────────────────────────

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
			a = emitter.ApplyTopLevelPrefix(a)
			sdl := graphqlgen.BuildSchema(a)
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

// ── deps ──────────────────────────────────────────────────────────────────────

func newDepsCmd() *cobra.Command {
	var validateOnly bool

	cmd := &cobra.Command{
		Use:     "deps",
		Short:   "Show service dependency graph from workspace consumes declarations",
		Long:    "Reads the workspace config and prints which services consume which.\nUseful for understanding inter-service dependencies at a glance.",
		Example: "  veld deps\n  veld deps --validate",
		RunE: func(cmd *cobra.Command, args []string) error {
			flags := config.FlagOverrides{}
			rc, err := config.BuildResolved(flags)
			if err != nil {
				return err
			}

			if len(rc.Workspace) == 0 {
				return fmt.Errorf("no workspace defined — deps requires a workspace config with multiple services")
			}

			// Validate consumes declarations.
			errs, warns := validator.ValidateWorkspaceConsumes(rc.Workspace)
			for _, w := range warns {
				fmt.Fprintf(os.Stderr, yellow("⚠")+"  %s\n", w)
			}
			for _, e := range errs {
				fmt.Fprintf(os.Stderr, red("✗")+"  %s\n", e)
			}
			if len(errs) > 0 {
				return fmt.Errorf("workspace validation failed")
			}
			if validateOnly {
				fmt.Println(green("✓") + " Workspace dependencies valid")
				return nil
			}

			// Print dependency graph.
			fmt.Printf("\n%s Service Dependencies\n\n", bold("◆"))
			for _, entry := range rc.Workspace {
				if len(entry.Consumes) > 0 {
					fmt.Printf("  %s → %s\n", bold(entry.Name), strings.Join(entry.Consumes, ", "))
				} else {
					fmt.Printf("  %s → %s\n", bold(entry.Name), dim("(none)"))
				}
			}
			fmt.Println()
			return nil
		},
	}
	cmd.Flags().BoolVar(&validateOnly, "validate", false,
		"only validate dependency declarations without printing the graph")
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

			// Generate to temp dir(s)
			tmpBackendDir, err := os.MkdirTemp("", "veld-diff-be-*")
			if err != nil {
				return err
			}
			defer os.RemoveAll(tmpBackendDir)

			tmpFrontendDir := tmpBackendDir
			if rc.SplitOutput() {
				tmpFrontendDir, err = os.MkdirTemp("", "veld-diff-fe-*")
				if err != nil {
					return err
				}
				defer os.RemoveAll(tmpFrontendDir)
			}

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
			a = emitter.ApplyTopLevelPrefix(a)

			opts := emitter.EmitOptions{BaseUrl: rc.BaseUrl}
			backendOrTool, _, err := emitter.GetBackendOrTool(rc.Backend)
			if err != nil {
				return err
			}
			if err := backendOrTool.Emit(a, tmpBackendDir, opts); err != nil {
				return err
			}
			frontend, err := emitter.GetFrontend(rc.Frontend)
			if err != nil {
				return err
			}
			if frontend != nil {
				if err := frontend.Emit(a, tmpFrontendDir, opts); err != nil {
					return err
				}
			}

			// Compare
			added, removed, modified := 0, 0, 0
			var diffs []string

			// diffDir compares a tmpDir against an existing outDir
			diffDir := func(tmpDir, outDir string) {
				// Walk temp dir for new/modified files
				filepath.Walk(tmpDir, func(path string, info os.FileInfo, err error) error {
					if err != nil || info.IsDir() {
						return nil
					}
					relPath, _ := filepath.Rel(tmpDir, path)
					existingPath := filepath.Join(outDir, relPath)

					newData, _ := os.ReadFile(path)
					existData, readErr := os.ReadFile(existingPath)

					if os.IsNotExist(readErr) {
						added++
						diffs = append(diffs, green("+ ")+relPath+" (new)")
					} else if string(newData) != string(existData) {
						modified++
						if !statOnly {
							diffs = append(diffs, yellow("~ ")+relPath+" (modified)")
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
				if _, statErr := os.Stat(outDir); statErr == nil {
					filepath.Walk(outDir, func(path string, info os.FileInfo, err error) error {
						if err != nil || info.IsDir() {
							return nil
						}
						relPath, _ := filepath.Rel(outDir, path)
						tmpPath := filepath.Join(tmpDir, relPath)
						if _, statErr := os.Stat(tmpPath); os.IsNotExist(statErr) {
							removed++
							diffs = append(diffs, red("- ")+relPath+" (removed)")
						}
						return nil
					})
				}
			}

			if rc.SplitOutput() {
				fmt.Println(dim("  Backend output: ") + bold(rc.BackendOut))
				diffDir(tmpBackendDir, rc.BackendOut)
				fmt.Println(dim("  Frontend output: ") + bold(rc.FrontendOut))
				diffDir(tmpFrontendDir, rc.FrontendOut)
			} else {
				diffDir(tmpBackendDir, rc.Out)
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

func newDocsCmd() *cobra.Command {
	var format, outputFile string
	cmd := &cobra.Command{
		Use:   "docs",
		Short: "Generate API documentation from the contract",
		Long: "Generates API documentation from your .veld contract.\n\n" +
			"In workspace/microservices mode, all service contracts are merged into\n" +
			"a single unified document covering every service, module, and model.",
		Example: "  veld docs\n  veld docs -o api-docs.html\n  veld docs --format=markdown",
		RunE: func(cmd *cobra.Command, args []string) error {
			rc, err := config.BuildResolved(config.FlagOverrides{})
			if err != nil {
				return err
			}

			var a ast.AST
			var docServices []docsgen.ServiceInfo

			if len(rc.Workspace) > 0 {
				// ── Workspace mode: parse + merge all service ASTs ────────────
				fmt.Printf("%s workspace: %d services\n", bold("◆"), len(rc.Workspace))

				var allConsumed []emitter.ConsumedServiceInfo
				for _, entry := range rc.Workspace {
					inputPath := entry.Input
					if inputPath == "" {
						continue
					}
					if !filepath.IsAbs(inputPath) {
						inputPath = filepath.Join(rc.ConfigDir, inputPath)
					}
					entryAST, _, err := loader.Parse(inputPath, rc.Aliases)
					if err != nil {
						fmt.Fprintf(os.Stderr, yellow("⚠")+"  skipping %s: %v\n", entry.Name, err)
						continue
					}
					entryAST = emitter.ApplyTopLevelPrefix(entryAST)
					allConsumed = append(allConsumed, emitter.ConsumedServiceInfo{
						Name:    entry.Name,
						AST:     entryAST,
						BaseUrl: entry.BaseUrl,
					})
					// Build per-service info for docs grouping.
					modNames := make([]string, len(entryAST.Modules))
					for i, m := range entryAST.Modules {
						modNames[i] = m.Name
					}
					desc := entry.Description
					if desc == "" {
						desc = entry.Name
					}
					docServices = append(docServices, docsgen.ServiceInfo{
						Name:        entry.Name,
						Description: desc,
						BaseUrl:     entry.BaseUrl,
						ModuleNames: modNames,
					})
				}
				a = emitter.MergeASTs(ast.AST{ASTVersion: "1.0.0"}, allConsumed)
			} else {
				// ── Single service mode ───────────────────────────────────────
				path := rc.Input
				if len(args) == 1 {
					path = args[0]
				}
				parsed, _, err := loader.Parse(path, rc.Aliases)
				if err != nil {
					return err
				}
				a = emitter.ApplyTopLevelPrefix(parsed)
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
				output = docsgen.BuildHTML(a, docServices)
			case "markdown", "md":
				output = docsgen.BuildMarkdown(a, docServices)
			default:
				return fmt.Errorf("unknown docs format %q (supported: html, markdown)", format)
			}

			// Default output file when none specified
			if outputFile == "" {
				switch format {
				case "html":
					outputFile = "docs.html"
				case "markdown", "md":
					outputFile = "docs.md"
				}
			}

			if err := os.WriteFile(outputFile, []byte(output), 0644); err != nil {
				return err
			}
			fmt.Println(green("✓") + " Docs → " + bold(outputFile))
			return nil
		},
	}
	cmd.Flags().StringVar(&format, "format", "html", "output format (html, markdown)")
	cmd.Flags().StringVarP(&outputFile, "output", "o", "", "output file (default: docs.html or docs.md)")
	return cmd
}

// ── lsp (placeholder) ────────────────────────────────────────────────────────

func newFmtCmd() *cobra.Command {
	var writeFlag bool
	cmd := &cobra.Command{
		Use:     "fmt [files...]",
		Short:   "Format .veld contract files",
		Long:    "Reads .veld files and outputs canonically formatted versions.\nUse --write to update files in place.",
		Example: "  veld fmt\n  veld fmt --write\n  veld fmt veld/models/user.veld",
		Hidden:  true,
		RunE: func(cmd *cobra.Command, args []string) error {
			var files []string
			if len(args) > 0 {
				files = args
			} else {
				// Find all .veld files from config
				path, err := config.ResolveInput(nil)
				if err != nil {
					return err
				}
				_, veldFiles, err := loader.Parse(path)
				if err != nil {
					return err
				}
				files = veldFiles
			}

			changed := 0
			for _, f := range files {
				formatted, err := vfmt.File(f)
				if err != nil {
					fmt.Fprintf(os.Stderr, yellow("warning: ")+"could not format %s: %v\n", f, err)
					continue
				}
				original, _ := os.ReadFile(f)
				if string(original) == formatted {
					continue
				}
				changed++
				if writeFlag {
					if err := os.WriteFile(f, []byte(formatted), 0644); err != nil {
						return fmt.Errorf("writing %s: %w", f, err)
					}
					fmt.Printf("  %s %s\n", green("✓"), f)
				} else {
					fmt.Printf("  %s %s (would change)\n", yellow("~"), f)
				}
			}

			if changed == 0 {
				fmt.Println(green("✓") + " All files already formatted")
			} else if !writeFlag {
				fmt.Printf("\n%d file(s) would change — run with %s to apply\n", changed, bold("--write"))
			}
			return nil
		},
	}
	cmd.Flags().BoolVarP(&writeFlag, "write", "w", false, "update files in place")
	return cmd
}

// ── doctor ────────────────────────────────────────────────────────────────────

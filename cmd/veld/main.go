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
	"github.com/veld-dev/veld/internal/emitter/node"
	"github.com/veld-dev/veld/internal/emitter/python"
	"github.com/veld-dev/veld/internal/emitter/typescript"
	"github.com/veld-dev/veld/internal/lexer"
	"github.com/veld-dev/veld/internal/parser"
	"github.com/veld-dev/veld/internal/validator"
)

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

// ── types ─────────────────────────────────────────────────────────────────────

// rawConfig mirrors veld.config.json on disk.
type rawConfig struct {
	Input    string `json:"input"`
	Backend  string `json:"backend"`
	Frontend string `json:"frontend"`
	Out      string `json:"out"`
}

// resolvedConfig has all paths resolved to be absolute.
type resolvedConfig struct {
	Input     string
	Backend   string
	Frontend  string
	Out       string
	ConfigDir string // absolute dir of veld.config.json; used for cache storage
}

// ── file loading ──────────────────────────────────────────────────────────────

// parseVeldFile loads a .veld entry point and recursively follows import
// statements. Returns the merged AST and the absolute paths of every .veld
// file that was loaded (for watch / incremental purposes).
func parseVeldFile(path string) (ast.AST, []string, error) {
	var files []string
	a, err := resolveVeldFile(path, make(map[string]bool), &files)
	return a, files, err
}

func resolveVeldFile(path string, seen map[string]bool, files *[]string) (ast.AST, error) {
	abs, err := filepath.Abs(path)
	if err != nil {
		return ast.AST{}, err
	}
	if seen[abs] {
		return ast.AST{ASTVersion: "1.0.0"}, nil // circular import guard
	}
	seen[abs] = true
	*files = append(*files, abs)

	content, err := os.ReadFile(path)
	if err != nil {
		return ast.AST{}, fmt.Errorf("reading %s: %w", path, err)
	}
	tokens, err := lexer.New(string(content)).Tokenize()
	if err != nil {
		return ast.AST{}, fmt.Errorf("lexing %s: %w", path, err)
	}
	a, err := parser.New(tokens).Parse()
	if err != nil {
		return ast.AST{}, fmt.Errorf("parsing %s: %w", path, err)
	}

	// Tag every definition with the file it came from (used by incremental gen).
	for i := range a.Models {
		a.Models[i].SourceFile = abs
	}
	for i := range a.Modules {
		a.Modules[i].SourceFile = abs
	}
	for i := range a.Enums {
		a.Enums[i].SourceFile = abs
	}

	// Resolve imports relative to this file's directory.
	dir := filepath.Dir(abs)
	merged := ast.AST{ASTVersion: "1.0.0"}
	for _, imp := range a.Imports {
		imported, err := resolveVeldFile(filepath.Join(dir, imp), seen, files)
		if err != nil {
			return ast.AST{}, fmt.Errorf("import %q: %w", imp, err)
		}
		merged.Models = append(merged.Models, imported.Models...)
		merged.Modules = append(merged.Modules, imported.Modules...)
		merged.Enums = append(merged.Enums, imported.Enums...)
	}
	merged.Models = append(merged.Models, a.Models...)
	merged.Modules = append(merged.Modules, a.Modules...)
	merged.Enums = append(merged.Enums, a.Enums...)
	return merged, nil
}

// ── config resolution ─────────────────────────────────────────────────────────

// findConfig locates veld.config.json and returns its contents plus its
// absolute directory path.
func findConfig() (rawConfig, string, error) {
	candidates := []string{
		"veld.config.json",
		filepath.Join("veld", "veld.config.json"),
	}
	for _, p := range candidates {
		data, err := os.ReadFile(p)
		if err != nil {
			continue
		}
		var cfg rawConfig
		if err := json.Unmarshal(data, &cfg); err != nil {
			return rawConfig{}, "", fmt.Errorf("parsing %s: %w", p, err)
		}
		abs, err := filepath.Abs(filepath.Dir(p))
		if err != nil {
			return rawConfig{}, "", err
		}
		return cfg, abs, nil
	}
	abs, _ := filepath.Abs(".")
	return rawConfig{}, abs, nil
}

// buildResolvedConfig merges config file values with flag overrides and
// resolves all paths to absolute form.
func buildResolvedConfig(cmd *cobra.Command, backendFlag, frontendFlag, inputFlag, outFlag string) (resolvedConfig, error) {
	cfg, cfgDir, err := findConfig()
	if err != nil {
		return resolvedConfig{}, err
	}

	if cmd.Flags().Changed("backend") {
		cfg.Backend = backendFlag
	}
	if cmd.Flags().Changed("frontend") {
		cfg.Frontend = frontendFlag
	}
	if cmd.Flags().Changed("input") {
		cfg.Input = inputFlag
		cfgDir, _ = filepath.Abs(".")
	}
	if cmd.Flags().Changed("out") {
		cfg.Out = outFlag
	}

	if cfg.Backend == "" {
		cfg.Backend = "node"
	}
	if cfg.Frontend == "" {
		cfg.Frontend = "react"
	}
	if cfg.Out == "" {
		cfg.Out = "./generated"
	}

	if cfg.Input == "" {
		return resolvedConfig{}, fmt.Errorf("no input file (use --input or create veld/veld.config.json)")
	}

	return resolvedConfig{
		Input:     filepath.Clean(filepath.Join(cfgDir, cfg.Input)),
		Backend:   cfg.Backend,
		Frontend:  cfg.Frontend,
		Out:       filepath.Clean(filepath.Join(cfgDir, cfg.Out)),
		ConfigDir: cfgDir,
	}, nil
}

// resolveInput returns the .veld input path from args or config file.
func resolveInput(args []string) (string, error) {
	if len(args) == 1 {
		return args[0], nil
	}
	cfg, cfgDir, err := findConfig()
	if err != nil {
		return "", err
	}
	if cfg.Input == "" {
		return "", fmt.Errorf("no input file specified and no veld.config.json found")
	}
	return filepath.Clean(filepath.Join(cfgDir, cfg.Input)), nil
}

// ── shared generation logic ────────────────────────────────────────────────────

// runGenerate parses, validates, and emits output.
//
// When incremental is false (the default) every module is regenerated —
// deterministic and safe for production pipelines.
//
// When incremental is true only modules whose source files changed since the
// last run are regenerated; the result is written to .veld-cache.json in
// ConfigDir. This is intended for local development only.
//
// Returns (regeneratedModuleNames, veldFileList, error).
// regeneratedModuleNames is nil when nothing needed to change (incremental only).
func runGenerate(rc resolvedConfig, incremental bool) ([]string, []string, error) {
	a, veldFiles, err := parseVeldFile(rc.Input)
	if err != nil {
		return nil, nil, err
	}
	if errs := validator.Validate(a); len(errs) > 0 {
		for _, e := range errs {
			fmt.Fprintln(os.Stderr, red("  error: ")+e.Error())
		}
		return nil, veldFiles, fmt.Errorf("contract validation failed")
	}

	// ── incremental: compute which modules need regeneration ──────────────────
	var targetModules map[string]bool // nil → regenerate all
	var c *cache.Cache

	if incremental {
		c = cache.Load(rc.ConfigDir)
		changedFiles := c.ChangedFiles(veldFiles)

		if len(changedFiles) == 0 {
			return nil, veldFiles, nil // nothing changed
		}

		changedFileSet := make(map[string]bool, len(changedFiles))
		for _, f := range changedFiles {
			changedFileSet[f] = true
		}

		// If any model-defining file changed, all modules are dirty because the
		// per-module types files include transitive model references.
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
			// Files changed but no modules were affected (e.g. comment-only edit).
			// Update cache so this file is not re-checked next run.
			for _, f := range veldFiles {
				c.Update(f)
			}
			_ = c.Save(rc.ConfigDir)
			return nil, veldFiles, nil
		}
	}

	// ── emit: backend ─────────────────────────────────────────────────────────
	// Filter the AST to only dirty modules when running incrementally.
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

	switch rc.Backend {
	case "node":
		if err := node.New().Emit(emitAST, rc.Out); err != nil {
			return nil, veldFiles, fmt.Errorf("node emitter: %w", err)
		}
	case "python":
		if err := python.New().Emit(emitAST, rc.Out); err != nil {
			return nil, veldFiles, fmt.Errorf("python emitter: %w", err)
		}
	default:
		return nil, veldFiles, fmt.Errorf("unknown backend %q (supported: node, python)", rc.Backend)
	}

	// ── emit: frontend ────────────────────────────────────────────────────────
	// The TypeScript client is one combined file, so always pass the full AST.
	if rc.Frontend == "react" || rc.Frontend == "typescript" {
		if err := typescript.New().Emit(a, rc.Out); err != nil {
			return nil, veldFiles, fmt.Errorf("typescript emitter: %w", err)
		}
	}

	// ── update cache after a fully successful generation ─────────────────────
	// Always write the cache — even on a full (non-incremental) build — so that
	// subsequent incremental runs correctly start from this baseline.
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

// printGenerateSummary prints a detailed breakdown of generated files.
func printGenerateSummary(rc resolvedConfig, modules []string) {
	// Relativize the output dir for nicer display
	relOut := rc.Out
	if cwd, err := os.Getwd(); err == nil {
		if r, err := filepath.Rel(cwd, rc.Out); err == nil {
			relOut = "./" + filepath.ToSlash(r)
		}
	}

	fmt.Println(green("✓") + " Generated → " + bold(relOut))

	// types/
	typeFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		typeFiles = append(typeFiles, strings.ToLower(m)+".ts")
	}
	if len(typeFiles) > 0 {
		fmt.Printf("  %s  %s\n", dim("types/"), strings.Join(typeFiles, ", "))
	}

	// interfaces/
	ifaceFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ifaceFiles = append(ifaceFiles, "I"+m+"Service.ts")
	}
	if len(ifaceFiles) > 0 {
		fmt.Printf("  %s  %s\n", dim("interfaces/"), strings.Join(ifaceFiles, ", "))
	}

	// routes/
	routeFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		routeFiles = append(routeFiles, strings.ToLower(m)+".routes.ts")
	}
	if len(routeFiles) > 0 {
		fmt.Printf("  %s  %s\n", dim("routes/"), strings.Join(routeFiles, ", "))
	}

	// schemas/
	fmt.Printf("  %s  %s\n", dim("schemas/"), "schemas.ts")

	// client/
	fmt.Printf("  %s  %s\n", dim("client/"), "api.ts")
}

// printPythonGenerateSummary prints a detailed breakdown for Python backend.
func printPythonGenerateSummary(rc resolvedConfig, modules []string) {
	relOut := rc.Out
	if cwd, err := os.Getwd(); err == nil {
		if r, err := filepath.Rel(cwd, rc.Out); err == nil {
			relOut = "./" + filepath.ToSlash(r)
		}
	}

	fmt.Println(green("✓") + " Generated → " + bold(relOut))

	typeFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		typeFiles = append(typeFiles, strings.ToLower(m)+".py")
	}
	if len(typeFiles) > 0 {
		fmt.Printf("  %s  %s\n", dim("types/"), strings.Join(typeFiles, ", "))
	}

	ifaceFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		ifaceFiles = append(ifaceFiles, "i_"+strings.ToLower(m)+"_service.py")
	}
	if len(ifaceFiles) > 0 {
		fmt.Printf("  %s  %s\n", dim("interfaces/"), strings.Join(ifaceFiles, ", "))
	}

	routeFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		routeFiles = append(routeFiles, strings.ToLower(m)+"_routes.py")
	}
	if len(routeFiles) > 0 {
		fmt.Printf("  %s  %s\n", dim("routes/"), strings.Join(routeFiles, ", "))
	}

	fmt.Printf("  %s  %s\n", dim("client/"), "api.ts")
}

// ── main ──────────────────────────────────────────────────────────────────────

func main() {
	root := &cobra.Command{
		Use:           "veld",
		Short:         "Contract-first API code generator",
		SilenceUsage:  true,
		SilenceErrors: true,
	}
	root.AddCommand(newValidateCmd(), newASTCmd(), newGenerateCmd(), newWatchCmd(), newInitCmd())
	if err := root.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, red("Error: ")+err.Error())
		os.Exit(1)
	}
}

// ── validate ──────────────────────────────────────────────────────────────────

func newValidateCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "validate [file]",
		Short: "Parse and validate a .veld contract file",
		Args:  cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			path, err := resolveInput(args)
			if err != nil {
				return err
			}
			a, _, err := parseVeldFile(path)
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
		Use:   "ast [file]",
		Short: "Dump the AST JSON for a .veld contract file",
		Args:  cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			path, err := resolveInput(args)
			if err != nil {
				return err
			}
			a, _, err := parseVeldFile(path)
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
	var incrementalFlag bool

	cmd := &cobra.Command{
		Use:   "generate",
		Short: "Generate code from a .veld contract",
		Long: "Generates typed backend interfaces and a frontend SDK from your .veld contract.\n\n" +
			"Every file is (re)generated by default — deterministic and safe for CI/CD.\n" +
			"Pass --incremental to skip modules whose source files have not changed\n" +
			"(intended for local development, not production pipelines).",
		RunE: func(cmd *cobra.Command, args []string) error {
			rc, err := buildResolvedConfig(cmd, backendFlag, frontendFlag, inputFlag, outFlag)
			if err != nil {
				return err
			}

			regenerated, _, err := runGenerate(rc, incrementalFlag)
			if err != nil {
				return err
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

			if rc.Backend == "python" {
				printPythonGenerateSummary(rc, regenerated)
			} else {
				printGenerateSummary(rc, regenerated)
			}
			return nil
		},
	}
	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend framework (node, python)")
	cmd.Flags().StringVar(&frontendFlag, "frontend", "", "frontend framework (react, typescript)")
	cmd.Flags().StringVar(&inputFlag, "input", "", "input .veld file")
	cmd.Flags().StringVar(&outFlag, "out", "", "output directory")
	cmd.Flags().BoolVar(&incrementalFlag, "incremental", false,
		"skip unchanged modules (dev only — not for production builds)")
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
			rc, err := buildResolvedConfig(cmd, backendFlag, frontendFlag, inputFlag, outFlag)
			if err != nil {
				return err
			}

			fmt.Println(bold("veld watch") + "  •  watching for changes  •  Ctrl-C to stop")
			fmt.Println()

			// Initial full generation to establish a clean baseline and warm the cache.
			regenerated, initFiles, genErr := runGenerate(rc, false)
			if genErr != nil {
				fmt.Fprintln(os.Stderr, red("error: ")+genErr.Error())
			} else {
				fmt.Printf(green("✓")+" Ready (%d module(s)) → %s\n", len(regenerated), rc.Out)
			}
			fmt.Println()

			// Seed the mtime map from the initial file list.
			mtimes := make(map[string]int64, len(initFiles))
			for _, f := range initFiles {
				if info, statErr := os.Stat(f); statErr == nil {
					mtimes[f] = info.ModTime().UnixNano()
				}
			}

			ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
			defer stop()

			ticker := time.NewTicker(300 * time.Millisecond)
			defer ticker.Stop()

			// Track whether the last cycle had an error, to avoid printing the
			// same error repeatedly until the file actually changes again.
			lastError := false

			for {
				select {
				case <-ctx.Done():
					fmt.Println("\nWatch stopped.")
					return nil

				case <-ticker.C:
					// Detect any mtime change among the currently watched files.
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

					// Update mtimes BEFORE running generation so that if
					// generation fails we don't re-trigger on the same unchanged
					// file in an infinite loop.
					for f := range mtimes {
						if info, statErr := os.Stat(f); statErr == nil {
							mtimes[f] = info.ModTime().UnixNano()
						}
					}

					ts := dim("[" + time.Now().Format("15:04:05") + "]")

					regen, newFiles, genErr := runGenerate(rc, true)
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

					// Refresh the watched file list to pick up newly added imports.
					if newFiles != nil {
						mtimes = make(map[string]int64, len(newFiles))
						for _, f := range newFiles {
							if info, statErr := os.Stat(f); statErr == nil {
								mtimes[f] = info.ModTime().UnixNano()
							}
						}
					}
				}
			}
		},
	}
	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend framework (node, python)")
	cmd.Flags().StringVar(&frontendFlag, "frontend", "", "frontend framework (react, typescript)")
	cmd.Flags().StringVar(&inputFlag, "input", "", "input .veld file")
	cmd.Flags().StringVar(&outFlag, "out", "", "output directory")
	return cmd
}

// ── init ──────────────────────────────────────────────────────────────────────

func newInitCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "init",
		Short: "Scaffold a new Veld project in the current directory",
		RunE:  func(cmd *cobra.Command, args []string) error { return runInit() },
	}
}

func runInit() error {
	// Guard: refuse if already initialised.
	for _, p := range []string{"veld/veld.config.json", "veld.config.json"} {
		if _, err := os.Stat(p); err == nil {
			fmt.Fprintln(os.Stderr, red("Error:")+" veld project already initialized in this directory")
			os.Exit(1)
		}
	}

	type entry struct{ path, content, label string }
	files := []entry{
		{"veld/veld.config.json", veldConfigContent, "veld/veld.config.json"},
		{"veld/schema.veld", schemaVeldContent, "veld/schema.veld"},
		{"veld/models/user.veld", modelsUserVeldContent, "veld/models/user.veld"},
		{"veld/models/common.veld", modelsCommonVeldContent, "veld/models/common.veld"},
		{"veld/modules/users.veld", modulesUsersVeldContent, "veld/modules/users.veld"},
		{"veld/modules/auth.veld", modulesAuthVeldContent, "veld/modules/auth.veld"},
		{"generated/.gitkeep", "", "generated/"},
		{"app/services/.gitkeep", "", "app/services/"},
		{"app/repositories/.gitkeep", "", "app/repositories/"},
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
  "input": "schema.veld",
  "backend": "node",
  "frontend": "react",
  "out": "../generated"
}
`

const schemaVeldContent = `// Entry point — imports all models and modules.
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
	"| `generated/` | Veld | Auto-generated — do not edit |\n" +
	"| `app/` | You | Business logic — never overwritten |\n\n" +
	"## Features\n\n" +
	"- **Enums** — `enum Role { admin user guest }`\n" +
	"- **Optional fields** — `bio?: string`\n" +
	"- **Descriptions** — `description: \"...\"`  → JSDoc/docstrings\n" +
	"- **Query parameters** — `query: UserFilters`\n" +
	"- **Default values** — `role: Role @default(user)`\n" +
	"- **Route prefixes** — `prefix: /api`\n" +
	"- **Array types** — `tags: string[]`, `output: User[]`\n" +
	"- **Rich scalars** — `string`, `int`, `float`, `bool`, `date`, `datetime`, `uuid`\n" +
	"- **Zod schemas** — auto-generated validation schemas\n\n" +
	"## Workflow\n\n" +
	"1. Edit files in `veld/models/` and `veld/modules/`\n" +
	"2. Run `veld generate` to regenerate `generated/`\n" +
	"3. Implement interfaces in `app/services/`\n" +
	"4. Import the SDK in your frontend from `generated/client/api.ts`\n\n" +
	"## Import system\n\n" +
	"Split your contract across as many files as you like:\n\n" +
	"```\n" +
	"// veld/schema.veld\n" +
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
	"| `veld ast` | Dump AST JSON for debugging |\n" +
	"| `veld init` | Scaffold a new project |\n"

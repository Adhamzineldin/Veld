package main

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/cache"
	"github.com/Adhamzineldin/Veld/internal/config"
	"github.com/Adhamzineldin/Veld/internal/diff"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/lint"
	"github.com/Adhamzineldin/Veld/internal/loader"
	"github.com/Adhamzineldin/Veld/internal/lsp"
	"github.com/Adhamzineldin/Veld/internal/schema"
	"github.com/Adhamzineldin/Veld/internal/setup"
	"github.com/Adhamzineldin/Veld/internal/validator"
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
	_ "github.com/Adhamzineldin/Veld/internal/emitter/cicd"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/database"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/dockerfile"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/envconfig"
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
	_ "github.com/Adhamzineldin/Veld/internal/emitter/openapi"
	_ "github.com/Adhamzineldin/Veld/internal/emitter/scaffold"
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
// Returns (regeneratedModuleNames, veldFileList, breakingChanges, error).
// breakingChanges is non-nil only when a .veld.lock.json baseline exists;
// callers are responsible for printing / acting on them.
func runGenerate(rc config.ResolvedConfig, incremental bool, opts emitter.EmitOptions) ([]string, []string, []diff.Change, error) {
	a, veldFiles, err := loader.Parse(rc.Input, rc.Aliases)
	if err != nil {
		return nil, nil, nil, err
	}
	if errs := validator.Validate(a); len(errs) > 0 {
		printValidationErrors(errs, veldFiles)
		return nil, veldFiles, nil, fmt.Errorf("contract validation failed")
	}

	// ── load previous lock for breaking-change detection ─────────────────
	oldAST, hasLock, lockErr := diff.LoadLock(rc.ConfigDir)
	if lockErr != nil {
		fmt.Fprintf(os.Stderr, yellow("warning: ")+"could not read lock file: %v\n", lockErr)
	}

	// ── incremental: compute which modules need regeneration ──────────────
	var targetModules map[string]bool
	var c *cache.Cache

	if incremental {
		c = cache.Load(rc.ConfigDir)
		changedFiles := c.ChangedFiles(veldFiles)

		if len(changedFiles) == 0 {
			return nil, veldFiles, nil, nil
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
			return nil, veldFiles, nil, nil
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

	// ── apply app-level prefix to module prefixes ────────────────────────
	if emitAST.Prefix != "" {
		for i := range emitAST.Modules {
			emitAST.Modules[i].Prefix = emitAST.Prefix + emitAST.Modules[i].Prefix
		}
	}

	// ── emit: backend ────────────────────────────────────────────────────
	backend, err := emitter.GetBackend(rc.Backend)
	if err != nil {
		return nil, veldFiles, nil, err
	}
	if err := backend.Emit(emitAST, rc.BackendOut, opts); err != nil {
		return nil, veldFiles, nil, fmt.Errorf("%s emitter: %w", rc.Backend, err)
	}
	// When using split output, also emit backend (types, errors, interfaces,
	// routes) into the frontend output dir so the frontend SDK is fully
	// self-contained — no cross-directory imports needed.
	if rc.SplitOutput() && !opts.DryRun {
		if err := backend.Emit(emitAST, rc.FrontendOut, opts); err != nil {
			return nil, veldFiles, nil, fmt.Errorf("%s emitter (frontend copy): %w", rc.Backend, err)
		}
	}

	// ── emit: frontend ───────────────────────────────────────────────────
	frontend, err := emitter.GetFrontend(rc.Frontend)
	if err != nil {
		return nil, veldFiles, nil, err
	}
	if frontend != nil {
		// Frontend SDK always gets the full AST (combined output).
		// App prefix was already applied to emitAST.Modules; apply to `a` too
		// since frontend uses the unfiltered AST.
		frontendAST := a
		if a.Prefix != "" {
			for i := range frontendAST.Modules {
				if !strings.HasPrefix(frontendAST.Modules[i].Prefix, a.Prefix) {
					frontendAST.Modules[i].Prefix = a.Prefix + frontendAST.Modules[i].Prefix
				}
			}
		}
		if err := frontend.Emit(frontendAST, rc.FrontendOut, opts); err != nil {
			return nil, veldFiles, nil, fmt.Errorf("%s emitter: %w", rc.Frontend, err)
		}
	}

	// ── generated/README.md ──────────────────────────────────────────────
	if !opts.DryRun {
		for _, dir := range rc.OutputDirs() {
			writeGeneratedReadme(dir, emitAST)
		}
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

	// ── breaking-change diff ──────────────────────────────────────────────
	// Compare against the previous lock; then persist the new snapshot.
	var changes []diff.Change
	if hasLock && !opts.DryRun {
		changes = diff.Diff(oldAST, a)
	}
	if !opts.DryRun {
		if err := diff.SaveLock(rc.ConfigDir, a); err != nil {
			fmt.Fprintf(os.Stderr, yellow("warning: ")+"lock save failed: %v\n", err)
		}
	}

	// ── lint hint ─────────────────────────────────────────────────────────
	// Run a quick lint pass and surface a one-liner so developers know to
	// investigate. Full details are always available via `veld lint`.
	if !opts.DryRun {
		if issues := lint.Lint(a); len(issues) > 0 {
			errCount := 0
			for _, iss := range issues {
				if iss.IsError() {
					errCount++
				}
			}
			if errCount > 0 {
				fmt.Fprintf(os.Stderr, yellow("⚠")+"  %d lint issue(s) found (%d error(s)) — run %s for details\n",
					len(issues), errCount, bold("veld lint"))
			} else {
				fmt.Fprintf(os.Stderr, dim("ℹ")+"  %d lint warning(s) — run %s for details\n",
					len(issues), bold("veld lint"))
			}
		}
	}

	names := make([]string, 0, len(emitAST.Modules))
	for _, mod := range emitAST.Modules {
		names = append(names, mod.Name)
	}
	return names, veldFiles, changes, nil
}

// printDiffChanges prints breaking changes and additions detected against the
// previous .veld.lock.json. It is a no-op when changes is empty or nil.
func printDiffChanges(changes []diff.Change) {
	if len(changes) == 0 {
		return
	}

	hasBreaking := diff.HasBreaking(changes)
	if hasBreaking {
		fmt.Println()
		fmt.Println(red("⚠  Breaking changes detected:"))
	} else {
		fmt.Println()
		fmt.Println(yellow("↑  Contract changes:"))
	}

	for _, c := range changes {
		if c.Kind == diff.Breaking {
			fmt.Printf("   %s  %s — %s\n", red("✗"), bold(c.Path), c.Message)
		} else {
			fmt.Printf("   %s  %s — %s\n", green("+"), dim(c.Path), c.Message)
		}
	}
	fmt.Println()
}

// printGenerateSummary prints a detailed breakdown of generated files
// by delegating to each emitter's Summary method.
func printGenerateSummary(rc config.ResolvedConfig, modules []string) {
	relPath := func(absDir string) string {
		rel := absDir
		if cwd, err := os.Getwd(); err == nil {
			if r, err := filepath.Rel(cwd, absDir); err == nil {
				rel = "./" + filepath.ToSlash(r)
			}
		}
		return rel
	}

	if rc.SplitOutput() {
		fmt.Println(green("✓") + " Generated:")
		fmt.Println("    backend  → " + bold(relPath(rc.BackendOut)))
		fmt.Println("    frontend → " + bold(relPath(rc.FrontendOut)))
	} else {
		fmt.Println(green("✓") + " Generated → " + bold(relPath(rc.Out)))
	}

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

// printImportInstructions prints language-specific import hints after generation
// for both the backend AND the frontend.
func printImportInstructions(rc config.ResolvedConfig) {
	be := rc.Backend
	fe := rc.Frontend

	hasBackend := be != "" && be != "none" && be != "openapi" && be != "database" &&
		be != "dockerfile" && be != "cicd" && be != "env" && be != "scaffold-tests"
	hasFrontend := fe != "" && fe != "none" && fe != "types-only"

	if !hasBackend && !hasFrontend {
		return
	}

	// ── Relative output paths for display ────────────────────────────────
	toRel := func(absDir string) string {
		rel := absDir
		if cwd, err := os.Getwd(); err == nil {
			if r, err := filepath.Rel(cwd, absDir); err == nil {
				rel = filepath.ToSlash(r)
			}
		}
		return rel
	}
	relBackendOut := toRel(rc.BackendOut)
	relFrontendOut := toRel(rc.FrontendOut)

	fmt.Println()
	fmt.Println(dim("  Import instructions:"))

	// ── Backend ──────────────────────────────────────────────────────────
	if hasBackend {
		fmt.Println()
		fmt.Println(dim("  Backend") + " (" + bold(be) + "):")

		switch be {
		case "node":
			fmt.Println(dim("    Setup:") + ` run ` + bold("veld setup") + ` then ` + bold("npm install"))
			fmt.Println(dim("    Types:    ") + ` import { User } from '@veld/generated/types';`)
			fmt.Println(dim("    Routes:   ") + ` import { usersRoutes } from '@veld/generated/routes/users.routes';`)
			fmt.Println(dim("    Interfaces:") + ` import { IUsersService } from '@veld/generated/interfaces/IUsersService';`)
		case "python":
			pkgName := filepath.Base(relBackendOut)
			fmt.Println(dim("    Setup:") + ` run ` + bold("veld setup") + ` then ` + bold("pip install -e ."))
			fmt.Println(dim("    Models:    ") + ` from ` + pkgName + `.models import User`)
			fmt.Println(dim("    Routes:    ") + ` from ` + pkgName + `.routes.users_routes import register_users_routes`)
			fmt.Println(dim("    Interfaces:") + ` from ` + pkgName + `.interfaces.i_users_service import IUsersService`)
			fmt.Println(dim("    Schemas:   ") + ` from ` + pkgName + `.schemas.schemas import UserSchema`)
		case "go":
			fmt.Println(dim("    Setup:") + ` add to go.mod → replace veld/generated => ./` + relBackendOut)
			fmt.Println(dim("    Types:    ") + ` import "veld/generated/internal/models"`)
			fmt.Println(dim("    Routes:   ") + ` import "veld/generated/internal/routes"`)
			fmt.Println(dim("    Interfaces:") + ` import "veld/generated/internal/interfaces"`)
		case "rust":
			fmt.Println(dim("    Setup:") + ` add to Cargo.toml [workspace] → members = ["` + relBackendOut + `"]`)
			fmt.Println(dim("    Types:    ") + ` use veld_generated::models::User;`)
			fmt.Println(dim("    Routes:   ") + ` use veld_generated::routes;`)
			fmt.Println(dim("    Interfaces:") + ` use veld_generated::services::IUsersService;`)
		case "java":
			fmt.Println(dim("    Setup:") + ` add to pom.xml → <module>` + relBackendOut + `</module>`)
			fmt.Println(dim("    Types:    ") + ` import veld.generated.models.User;`)
			fmt.Println(dim("    Routes:   ") + ` import veld.generated.controllers.UsersController;`)
			fmt.Println(dim("    Interfaces:") + ` import veld.generated.services.IUsersService;`)
		case "csharp":
			fmt.Println(dim("    Setup:") + ` add ProjectReference → ` + relBackendOut + `/` + relBackendOut + `.csproj`)
			fmt.Println(dim("    Types:    ") + ` using Veld.Generated.Models;`)
			fmt.Println(dim("    Routes:   ") + ` using Veld.Generated.Controllers;`)
			fmt.Println(dim("    Interfaces:") + ` using Veld.Generated.Services;`)
		case "php":
			fmt.Println(dim("    Setup:") + ` add to composer.json → "Veld\\Generated\\": "` + relBackendOut + `/"`)
			fmt.Println(dim("    Types:    ") + ` use Veld\Generated\Models\User;`)
			fmt.Println(dim("    Routes:   ") + ` // routes/api.php is auto-registered`)
			fmt.Println(dim("    Interfaces:") + ` use Veld\Generated\Services\IUsersService;`)
		}
	}

	// ── Frontend ─────────────────────────────────────────────────────────
	if hasFrontend {
		fmt.Println()
		fmt.Println(dim("  Frontend") + " (" + bold(fe) + "):")

		switch fe {
		case "typescript", "react", "vue", "angular", "svelte":
			fmt.Println(dim("    Setup:") + ` run ` + bold("veld setup") + ` then ` + bold("npm install"))
		}

		switch fe {
		case "typescript":
			fmt.Println(dim("    Client:") + ` import { api } from '@veld/client';`)
			fmt.Println(dim("    Types: ") + ` import type { User } from '@veld/client/types';`)
			fmt.Println(dim("    Errors:") + ` import { VeldApiError } from '@veld/client/errors';`)
		case "react":
			fmt.Println(dim("    Client:") + ` import { api } from '@veld/client';`)
			fmt.Println(dim("    Types: ") + ` import type { User } from '@veld/client/types';`)
			fmt.Println(dim("    Errors:") + ` import { VeldApiError } from '@veld/client/errors';`)
			fmt.Println(dim("    Hooks: ") + ` import { useUsersListUsers } from '@veld/hooks';`)
			fmt.Println(dim("    Requires:") + ` npm install @tanstack/react-query`)
		case "vue":
			fmt.Println(dim("    Client:     ") + ` import { api } from '@veld/client';`)
			fmt.Println(dim("    Types:      ") + ` import type { User } from '@veld/client/types';`)
			fmt.Println(dim("    Composables:") + ` import { useUsers } from '@veld/composables';`)
		case "angular":
			fmt.Println(dim("    Services:") + ` import { UsersService } from '@veld/services';`)
			fmt.Println(dim("    Types:   ") + ` import type { User } from '@veld/client/types';`)
		case "svelte":
			fmt.Println(dim("    Client:") + ` import { api } from '@veld/client';`)
			fmt.Println(dim("    Types: ") + ` import type { User } from '@veld/client/types';`)
			fmt.Println(dim("    Stores: ") + ` import { createUsersStore } from '@veld/stores';`)
		case "dart", "flutter":
			fmt.Println(dim("    Setup:") + ` add to pubspec.yaml → veld_client: { path: ./` + relFrontendOut + `/client }`)
			fmt.Println(dim("    Then: ") + ` import 'package:veld_client/api_client.dart';`)
		case "kotlin":
			fmt.Println(dim("    Setup:") + ` add to settings.gradle.kts → include(":veld-client")`)
			fmt.Println(dim("    Then: ") + ` import veld.generated.client.*`)
		case "swift":
			fmt.Println(dim("    Setup:") + ` Xcode → File → Add Package Dependencies → Add Local`)
			fmt.Println(dim("    Then: ") + ` import VeldClient`)
		}
	}

	fmt.Println()
	fmt.Println(dim("  Or run: ") + bold("veld setup") + dim(" to auto-configure project files"))
}

// printSetupResults formats setup.Result entries for the terminal.
func printSetupResults(results []setup.Result) {
	if len(results) == 0 {
		return
	}
	fmt.Println()
	fmt.Println(dim("  Setup:"))
	for _, r := range results {
		switch r.Action {
		case "patched":
			fmt.Printf("  %s %s — %s\n", green("✓"), r.File, r.Detail)
		case "skipped":
			fmt.Printf("  %s %s — %s\n", dim("·"), r.File, dim(r.Detail))
		case "not-found":
			fmt.Printf("  %s %s — %s\n", yellow("!"), r.File, r.Detail)
		case "manual":
			fmt.Printf("  %s %s — %s\n", dim("→"), r.File, r.Detail)
		}
	}
}

func newSetupCmd() *cobra.Command {
	var backendDirFlag, frontendDirFlag string
	var backendFlag, frontendFlag, inputFlag, outFlag string

	cmd := &cobra.Command{
		Use:   "setup",
		Short: "Auto-configure project files for seamless imports",
		Long: "Patches config files (tsconfig.json, pubspec.yaml, go.mod, etc.) so that\n" +
			"generated code can be imported without manual edits.\n\n" +
			"Reads backend/frontend from veld.config.json and applies the appropriate patches.\n" +
			"If the generated output path has changed, existing entries are updated in place.\n\n" +
			"Use --backend-dir / --frontend-dir to point at project folders outside the\n" +
			"current directory, so you don't need a config file in each folder.",
		Example: "  veld setup\n" +
			"  veld setup --backend-dir=../server --frontend-dir=../client\n" +
			"  veld setup --out=./output --backend=go --frontend=react",
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
				return fmt.Errorf("could not load config: %w", err)
			}
			projectDir, _ := os.Getwd()

			opts := setup.Options{}
			if cmd.Flags().Changed("backend-dir") {
				abs, err := filepath.Abs(backendDirFlag)
				if err != nil {
					return fmt.Errorf("invalid --backend-dir: %w", err)
				}
				opts.BackendDir = abs
			} else if rc.BackendDir != "" {
				opts.BackendDir = rc.BackendDir
			}
			if cmd.Flags().Changed("frontend-dir") {
				abs, err := filepath.Abs(frontendDirFlag)
				if err != nil {
					return fmt.Errorf("invalid --frontend-dir: %w", err)
				}
				opts.FrontendDir = abs
			} else if rc.FrontendDir != "" {
				opts.FrontendDir = rc.FrontendDir
			}

			results := setup.Run(projectDir, rc.Backend, rc.Frontend, rc.Out, setup.Options{
				BackendDir:     opts.BackendDir,
				FrontendDir:    opts.FrontendDir,
				BackendOutDir:  rc.BackendOut,
				FrontendOutDir: rc.FrontendOut,
			})
			if len(results) == 0 {
				fmt.Println(dim("  No config files to patch for this stack"))
				return nil
			}
			printSetupResults(results)
			return nil
		},
	}
	cmd.Flags().StringVar(&backendDirFlag, "backend-dir", "",
		"directory containing backend project files (default: current directory)")
	cmd.Flags().StringVar(&frontendDirFlag, "frontend-dir", "",
		"directory containing frontend project files (default: current directory)")
	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend target override")
	cmd.Flags().StringVar(&frontendFlag, "frontend", "", "frontend SDK override")
	cmd.Flags().StringVar(&inputFlag, "input", "", "input .veld file")
	cmd.Flags().StringVar(&outFlag, "out", "", "output directory override")
	return cmd
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
  veld lsp                     Start the LSP server
  veld setup                   Auto-configure project imports

Backends:  node, python, go, rust, java, csharp, php
           openapi, database, dockerfile, cicd, env, scaffold-tests
Frontends: typescript, react, vue, angular, svelte, dart, kotlin, swift, types-only, none`,
		SilenceUsage:  true,
		SilenceErrors: true,
	}
	root.AddCommand(
		newValidateCmd(), newASTCmd(), newGenerateCmd(), newWatchCmd(),
		newInitCmd(), newCleanCmd(), newOpenAPICmd(), newGraphQLCmd(),
		newSchemaCmd(), newDiffCmd(), newLintCmd(), newDocsCmd(), newLSPCmd(),
		newSetupCmd(),
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
	var incrementalFlag, dryRunFlag, setupFlag, validateFlag, strictFlag, forceFlag bool

	cmd := &cobra.Command{
		Use:   "generate",
		Short: "Generate code from a .veld contract",
		Long: "Generates typed backend interfaces and a frontend SDK from your .veld contract.\n\n" +
			"Every file is (re)generated by default — deterministic and safe for CI/CD.\n" +
			"Pass --incremental to skip modules whose source files have not changed\n" +
			"(intended for local development, not production pipelines).\n\n" +
			"Backends: node, python, go, rust, java, csharp, php,\n" +
			"          openapi, database, dockerfile, cicd, env, scaffold-tests\n" +
			"Frontends: typescript, react, vue, angular, svelte,\n" +
			"           dart, kotlin, swift, types-only, none",
		Example: "  veld generate\n" +
			"  veld generate --backend=node --frontend=react\n" +
			"  veld generate --backend=go --frontend=vue\n" +
			"  veld generate --frontend=types-only\n" +
			"  veld generate --backend=openapi\n" +
			"  veld generate --backend=dockerfile\n" +
			"  veld generate --dry-run",
		RunE: func(cmd *cobra.Command, args []string) error {
			flags := config.FlagOverrides{
				Backend:     backendFlag,
				Frontend:    frontendFlag,
				Input:       inputFlag,
				Out:         outFlag,
				Validate:    validateFlag,
				BackendSet:  cmd.Flags().Changed("backend"),
				FrontendSet: cmd.Flags().Changed("frontend"),
				InputSet:    cmd.Flags().Changed("input"),
				OutSet:      cmd.Flags().Changed("out"),
				ValidateSet: cmd.Flags().Changed("validate"),
			}
			rc, err := config.BuildResolved(flags)
			if err != nil {
				return err
			}

			opts := emitter.EmitOptions{
				BaseUrl:  rc.BaseUrl,
				DryRun:   dryRunFlag,
				Validate: rc.Validate,
			}

			// ── Pre-emit breaking-change gate ────────────────────────────────────
			// Runs before any files are written so the developer can decide whether
			// to proceed. Skipped for dry-run and incremental (dev-mode) builds.
			if !dryRunFlag && !incrementalFlag {
				if preChanges := computePreChanges(rc); diff.HasBreaking(preChanges) {
					printDiffChanges(preChanges)
					switch {
					case strictFlag:
						// CI mode: always fail, no human interaction possible.
						fmt.Fprintln(os.Stderr, red("✗")+" Breaking changes detected — generation blocked by --strict")
						return fmt.Errorf("breaking changes blocked by --strict (use --force to override in dev)")
					case forceFlag:
						// Developer explicitly opted in — warn but continue.
						fmt.Fprintln(os.Stderr, yellow("⚠")+"  --force: generating despite breaking changes")
					default:
						// Interactive: ask the developer.
						if !promptContinue("Generate anyway?") {
							return fmt.Errorf("generation aborted")
						}
					}
				}
			}

			regenerated, _, changes, err := runGenerate(rc, incrementalFlag, opts)
			if err != nil {
				return err
			}

			if dryRunFlag {
				fmt.Println(green("✓") + " Dry run — no files written")
				printGenerateSummary(rc, regenerated)
				printDiffChanges(changes)
				return nil
			}

			if incrementalFlag {
				if regenerated == nil {
					fmt.Println(green("✓") + " Nothing changed")
				} else if rc.SplitOutput() {
					fmt.Printf(green("✓")+" Regenerated %s → backend: %s, frontend: %s\n",
						strings.Join(regenerated, ", "), rc.BackendOut, rc.FrontendOut)
				} else {
					fmt.Printf(green("✓")+" Regenerated %s → %s\n",
						strings.Join(regenerated, ", "), rc.Out)
				}
				printDiffChanges(changes)
				return nil
			}

			printGenerateSummary(rc, regenerated)
			// Breaking changes were already shown (and accepted) in the pre-emit
			// gate above. Only surface non-breaking additions here.
			var additions []diff.Change
			for _, c := range changes {
				if c.Kind == diff.Added {
					additions = append(additions, c)
				}
			}
			printDiffChanges(additions)
			printImportInstructions(rc)

			if setupFlag {
				projectDir, _ := os.Getwd()
				setupOpts := setup.Options{
					BackendDir:     rc.BackendDir,
					FrontendDir:    rc.FrontendDir,
					BackendOutDir:  rc.BackendOut,
					FrontendOutDir: rc.FrontendOut,
				}
				results := setup.Run(projectDir, rc.Backend, rc.Frontend, rc.Out, setupOpts)
				printSetupResults(results)
			}
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
	cmd.Flags().BoolVar(&setupFlag, "setup", false,
		"auto-configure project files for seamless imports after generation")
	cmd.Flags().BoolVar(&validateFlag, "validate", false,
		"emit zero-dep runtime validators and wire into route handlers (overrides config)")
	cmd.Flags().BoolVar(&strictFlag, "strict", false,
		"exit non-zero if any breaking changes are detected (ideal for CI/CD pipelines)")
	cmd.Flags().BoolVar(&forceFlag, "force", false,
		"generate despite breaking changes without prompting (overrides interactive gate)")
	return cmd
}

// computePreChanges loads the previous lock file and diffs it against the
// current contract WITHOUT emitting any files. Returns nil if no lock exists.
func computePreChanges(rc config.ResolvedConfig) []diff.Change {
	oldAST, hasLock, err := diff.LoadLock(rc.ConfigDir)
	if err != nil || !hasLock {
		return nil
	}
	a, _, err := loader.Parse(rc.Input, rc.Aliases)
	if err != nil {
		return nil
	}
	return diff.Diff(oldAST, a)
}

// promptContinue prints a [y/N] prompt and returns true only if the user
// types "y" or "Y". Any other input (including Enter) returns false.
func promptContinue(question string) bool {
	fmt.Printf("%s  %s [y/N]: ", yellow("⚠"), question)
	reader := bufio.NewReader(os.Stdin)
	line, _ := reader.ReadString('\n')
	return strings.ToLower(strings.TrimSpace(line)) == "y"
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
				BaseUrl:  rc.BaseUrl,
				Validate: rc.Validate,
			}

			regenerated, initFiles, changes, genErr := runGenerate(rc, false, opts)
			if genErr != nil {
				fmt.Fprintln(os.Stderr, red("error: ")+genErr.Error())
			} else if rc.SplitOutput() {
				fmt.Printf(green("✓")+" Ready (%d module(s)) → backend: %s, frontend: %s\n",
					len(regenerated), rc.BackendOut, rc.FrontendOut)
			} else {
				fmt.Printf(green("✓")+" Ready (%d module(s)) → %s\n", len(regenerated), rc.Out)
			}
			printDiffChanges(changes)
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

						regen, newFiles, changes, genErr := runGenerate(rc, true, opts)
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
							printDiffChanges(changes)
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

			cleaned := false
			for _, dir := range rc.OutputDirs() {
				if _, statErr := os.Stat(dir); os.IsNotExist(statErr) {
					continue
				}
				if err := os.RemoveAll(dir); err != nil {
					return fmt.Errorf("failed to remove %s: %w", dir, err)
				}
				fmt.Println(green("✓") + " Cleaned " + bold(dir))
				cleaned = true
			}

			// Also remove cache and lock file.
			cacheFile := filepath.Join(rc.ConfigDir, ".veld-cache.json")
			os.Remove(cacheFile)
			if err := diff.DeleteLock(rc.ConfigDir); err != nil {
				fmt.Fprintf(os.Stderr, yellow("warning: ")+"could not remove lock file: %v\n", err)
			}

			if !cleaned {
				fmt.Println(green("✓") + " Nothing to clean — output directory does not exist")
			}
			return nil
		},
	}
}

// ── lint ──────────────────────────────────────────────────────────────────────

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

			opts := emitter.EmitOptions{BaseUrl: rc.BaseUrl}
			backend, err := emitter.GetBackend(rc.Backend)
			if err != nil {
				return err
			}
			if err := backend.Emit(a, tmpBackendDir, opts); err != nil {
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
		Example: "  veld docs\n  veld docs -o api-docs.html\n  veld docs --format=markdown",
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
*,*::before,*::after{margin:0;padding:0;box-sizing:border-box}
:root{
  --bg:#f9fafb;--fg:#111827;--sidebar-bg:#ffffff;--sidebar-border:#e5e7eb;
  --card:#ffffff;--card-border:#e5e7eb;--card-shadow:0 1px 3px rgba(0,0,0,.06);
  --accent:#4f46e5;--accent-hover:#4338ca;--accent-bg:#eef2ff;--accent-fg:#3730a3;
  --muted:#6b7280;--muted-light:#9ca3af;
  --code-bg:#f3f4f6;--code-fg:#1f2937;
  --table-stripe:#f9fafb;--table-hover:#f3f4f6;
  --get:#059669;--get-bg:#ecfdf5;--post:#2563eb;--post-bg:#eff6ff;
  --put:#d97706;--put-bg:#fffbeb;--delete:#dc2626;--delete-bg:#fef2f2;
  --patch:#ea580c;--patch-bg:#fff7ed;--ws:#7c3aed;--ws-bg:#f5f3ff;
  --radius:8px;--transition:150ms ease;
}
[data-theme="dark"]{
  --bg:#0f172a;--fg:#e2e8f0;--sidebar-bg:#1e293b;--sidebar-border:#334155;
  --card:#1e293b;--card-border:#334155;--card-shadow:0 1px 3px rgba(0,0,0,.3);
  --accent:#818cf8;--accent-hover:#a5b4fc;--accent-bg:#1e1b4b;--accent-fg:#c7d2fe;
  --muted:#94a3b8;--muted-light:#64748b;
  --code-bg:#334155;--code-fg:#e2e8f0;
  --table-stripe:#1e293b;--table-hover:#334155;
  --get:#34d399;--get-bg:#064e3b;--post:#60a5fa;--post-bg:#1e3a5f;
  --put:#fbbf24;--put-bg:#451a03;--delete:#f87171;--delete-bg:#450a0a;
  --patch:#fb923c;--patch-bg:#431407;--ws:#a78bfa;--ws-bg:#2e1065;
}
html{scroll-behavior:smooth}
body{font-family:Inter,-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:var(--bg);color:var(--fg);line-height:1.6;display:flex;min-height:100vh}

/* Sidebar */
.sidebar{width:280px;background:var(--sidebar-bg);border-right:1px solid var(--sidebar-border);position:fixed;top:0;left:0;height:100vh;display:flex;flex-direction:column;z-index:20}
.sidebar-header{padding:24px 20px 16px;border-bottom:1px solid var(--sidebar-border)}
.sidebar-header h1{font-size:20px;font-weight:700;letter-spacing:-.02em}
.sidebar-header h1 span{color:var(--accent)}
.sidebar-header p{font-size:12px;color:var(--muted);margin-top:2px}
.search-box{padding:12px 16px}
.search-box input{width:100%;padding:8px 12px 8px 36px;border:1px solid var(--card-border);border-radius:var(--radius);font-size:13px;background:var(--bg);color:var(--fg);outline:none;transition:border-color var(--transition)}
.search-box input:focus{border-color:var(--accent)}
.search-box{position:relative}
.search-box::before{content:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' fill='%236b7280' viewBox='0 0 24 24'%3E%3Cpath d='M21 21l-4.35-4.35M11 19a8 8 0 100-16 8 8 0 000 16z' stroke='%236b7280' stroke-width='2' fill='none' stroke-linecap='round'/%3E%3C/svg%3E");position:absolute;left:28px;top:50%;transform:translateY(-50%)}
.nav-scroll{flex:1;overflow-y:auto;padding:8px 12px 24px}
.nav-group{margin-bottom:16px}
.nav-group-title{font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.05em;color:var(--muted-light);padding:4px 8px;margin-bottom:4px}
.nav-link{display:block;padding:6px 12px;font-size:13px;color:var(--fg);text-decoration:none;border-radius:6px;transition:all var(--transition);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.nav-link:hover{background:var(--accent-bg);color:var(--accent-fg)}
.nav-link .method-dot{display:inline-block;width:8px;height:8px;border-radius:50%;margin-right:8px;vertical-align:middle}

/* Main content */
.main{margin-left:280px;flex:1;padding:48px 56px;max-width:960px}
.main h2{font-size:24px;font-weight:700;margin:48px 0 8px;letter-spacing:-.02em}
.main h2:first-child{margin-top:0}
.section-desc{color:var(--muted);font-size:14px;margin-bottom:24px}

/* Endpoint cards */
.endpoint{background:var(--card);border:1px solid var(--card-border);border-radius:var(--radius);margin-bottom:12px;overflow:hidden;box-shadow:var(--card-shadow);transition:box-shadow var(--transition)}
.endpoint:hover{box-shadow:0 4px 12px rgba(0,0,0,.08)}
.endpoint-header{display:flex;align-items:center;gap:12px;padding:14px 20px;cursor:pointer;user-select:none}
.endpoint-header:hover{background:var(--table-hover)}
.method-badge{display:inline-flex;align-items:center;justify-content:center;min-width:56px;padding:4px 10px;border-radius:4px;font-size:11px;font-weight:700;letter-spacing:.03em;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.method-GET{background:var(--get-bg);color:var(--get)}
.method-POST{background:var(--post-bg);color:var(--post)}
.method-PUT{background:var(--put-bg);color:var(--put)}
.method-DELETE{background:var(--delete-bg);color:var(--delete)}
.method-PATCH{background:var(--patch-bg);color:var(--patch)}
.method-WS{background:var(--ws-bg);color:var(--ws)}
.endpoint-path{font-family:'SF Mono',SFMono-Regular,Consolas,monospace;font-size:13px;font-weight:500;flex:1}
.endpoint-path .param{color:var(--accent);font-weight:600}
.endpoint-name{font-size:12px;color:var(--muted);font-weight:500}
.endpoint-detail{padding:0 20px 16px;display:none;border-top:1px solid var(--card-border)}
.endpoint-detail.open{display:block;padding-top:16px}
.detail-row{display:flex;gap:8px;margin-bottom:8px;font-size:13px}
.detail-label{font-weight:600;min-width:80px;color:var(--muted)}
.detail-value code{background:var(--code-bg);color:var(--code-fg);padding:2px 8px;border-radius:4px;font-size:12px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}

/* Model cards */
.model-card{background:var(--card);border:1px solid var(--card-border);border-radius:var(--radius);margin-bottom:16px;overflow:hidden;box-shadow:var(--card-shadow)}
.model-header{padding:16px 20px;border-bottom:1px solid var(--card-border)}
.model-header h3{font-size:16px;font-weight:600}
.model-header h3 .extends{font-weight:400;color:var(--muted);font-size:13px;margin-left:8px}
.model-header .model-desc{color:var(--muted);font-size:13px;margin-top:4px}
.model-table{width:100%;border-collapse:collapse}
.model-table th{text-align:left;padding:10px 20px;font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.05em;color:var(--muted-light);background:var(--table-stripe);border-bottom:1px solid var(--card-border)}
.model-table td{padding:10px 20px;font-size:13px;border-bottom:1px solid var(--card-border)}
.model-table tr:last-child td{border-bottom:none}
.model-table tr:hover td{background:var(--table-hover)}
.model-table code{background:var(--code-bg);color:var(--code-fg);padding:2px 6px;border-radius:4px;font-size:12px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}
.optional-badge{display:inline-block;padding:1px 6px;border-radius:3px;font-size:10px;font-weight:600;background:var(--put-bg);color:var(--put)}
.default-value{color:var(--accent);font-size:12px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace}

/* Enum cards */
.enum-values{display:flex;flex-wrap:wrap;gap:6px;padding:16px 20px}
.enum-value{background:var(--code-bg);color:var(--code-fg);padding:4px 12px;border-radius:4px;font-size:12px;font-family:'SF Mono',SFMono-Regular,Consolas,monospace;font-weight:500}

/* Theme toggle */
.toolbar{position:fixed;top:16px;right:24px;z-index:30;display:flex;gap:8px}
.theme-btn{background:var(--card);border:1px solid var(--card-border);color:var(--fg);width:36px;height:36px;border-radius:var(--radius);cursor:pointer;display:flex;align-items:center;justify-content:center;font-size:16px;box-shadow:var(--card-shadow);transition:all var(--transition)}
.theme-btn:hover{border-color:var(--accent);color:var(--accent)}

/* Stats bar */
.stats{display:flex;gap:24px;margin-bottom:32px}
.stat{background:var(--card);border:1px solid var(--card-border);border-radius:var(--radius);padding:16px 24px;box-shadow:var(--card-shadow);flex:1;text-align:center}
.stat-value{font-size:28px;font-weight:700;color:var(--accent)}
.stat-label{font-size:12px;color:var(--muted);margin-top:2px}

/* Responsive */
@media(max-width:768px){
  .sidebar{display:none}
  .main{margin-left:0;padding:24px 16px}
}
</style>
</head>
<body>
<div class="toolbar">
  <button class="theme-btn" onclick="toggleTheme()" title="Toggle dark mode">
    <svg id="theme-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/></svg>
  </button>
</div>
<nav class="sidebar">
  <div class="sidebar-header">
    <h1><span>Veld</span> API</h1>
    <p>Auto-generated documentation</p>
  </div>
  <div class="search-box">
    <input type="text" id="search" placeholder="Search endpoints..." oninput="filterNav(this.value)">
  </div>
  <div class="nav-scroll">
`)

	// Sidebar — Modules with action dots
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("    <div class=\"nav-group\"><div class=\"nav-group-title\">%s</div>\n", mod.Name))
		for _, act := range mod.Actions {
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			dotColor := "var(--get)"
			switch strings.ToUpper(act.Method) {
			case "POST":
				dotColor = "var(--post)"
			case "PUT":
				dotColor = "var(--put)"
			case "DELETE":
				dotColor = "var(--delete)"
			case "PATCH":
				dotColor = "var(--patch)"
			case "WS":
				dotColor = "var(--ws)"
			}
			sb.WriteString(fmt.Sprintf("      <a href=\"#action-%s-%s\" class=\"nav-link\"><span class=\"method-dot\" style=\"background:%s\"></span>%s <span style=\"color:var(--muted-light);font-size:11px;margin-left:4px\">%s</span></a>\n",
				strings.ToLower(mod.Name), strings.ToLower(act.Name), dotColor, act.Name, routePath))
		}
		sb.WriteString("    </div>\n")
	}

	// Sidebar — Models
	if len(a.Models) > 0 {
		sb.WriteString("    <div class=\"nav-group\"><div class=\"nav-group-title\">Models</div>\n")
		for _, m := range a.Models {
			sb.WriteString(fmt.Sprintf("      <a href=\"#model-%s\" class=\"nav-link\">%s</a>\n", strings.ToLower(m.Name), m.Name))
		}
		sb.WriteString("    </div>\n")
	}
	if len(a.Enums) > 0 {
		sb.WriteString("    <div class=\"nav-group\"><div class=\"nav-group-title\">Enums</div>\n")
		for _, en := range a.Enums {
			sb.WriteString(fmt.Sprintf("      <a href=\"#enum-%s\" class=\"nav-link\">%s</a>\n", strings.ToLower(en.Name), en.Name))
		}
		sb.WriteString("    </div>\n")
	}
	sb.WriteString("  </div>\n</nav>\n<main class=\"main\">\n")

	// Stats
	totalActions := 0
	for _, mod := range a.Modules {
		totalActions += len(mod.Actions)
	}
	sb.WriteString("<div class=\"stats\">\n")
	sb.WriteString(fmt.Sprintf("  <div class=\"stat\"><div class=\"stat-value\">%d</div><div class=\"stat-label\">Modules</div></div>\n", len(a.Modules)))
	sb.WriteString(fmt.Sprintf("  <div class=\"stat\"><div class=\"stat-value\">%d</div><div class=\"stat-label\">Endpoints</div></div>\n", totalActions))
	sb.WriteString(fmt.Sprintf("  <div class=\"stat\"><div class=\"stat-value\">%d</div><div class=\"stat-label\">Models</div></div>\n", len(a.Models)))
	sb.WriteString(fmt.Sprintf("  <div class=\"stat\"><div class=\"stat-value\">%d</div><div class=\"stat-label\">Enums</div></div>\n", len(a.Enums)))
	sb.WriteString("</div>\n\n")

	// Modules — Endpoint cards
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("<h2 id=\"mod-%s\">%s</h2>\n", strings.ToLower(mod.Name), mod.Name))
		if mod.Description != "" {
			sb.WriteString(fmt.Sprintf("<p class=\"section-desc\">%s</p>\n", mod.Description))
		}
		for _, act := range mod.Actions {
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			method := strings.ToUpper(act.Method)
			// Highlight path params
			highlightedPath := routePath
			for _, seg := range strings.Split(routePath, "/") {
				if strings.HasPrefix(seg, ":") {
					highlightedPath = strings.Replace(highlightedPath, seg, "<span class=\"param\">"+seg+"</span>", 1)
				}
			}
			sb.WriteString(fmt.Sprintf("<div class=\"endpoint\" id=\"action-%s-%s\">\n", strings.ToLower(mod.Name), strings.ToLower(act.Name)))
			sb.WriteString(fmt.Sprintf("  <div class=\"endpoint-header\" onclick=\"this.nextElementSibling.classList.toggle('open')\">\n"))
			sb.WriteString(fmt.Sprintf("    <span class=\"method-badge method-%s\">%s</span>\n", method, method))
			sb.WriteString(fmt.Sprintf("    <span class=\"endpoint-path\">%s</span>\n", highlightedPath))
			sb.WriteString(fmt.Sprintf("    <span class=\"endpoint-name\">%s</span>\n", act.Name))
			sb.WriteString("  </div>\n")
			sb.WriteString("  <div class=\"endpoint-detail\">\n")
			if act.Description != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Description</span><span>%s</span></div>\n", act.Description))
			}
			if act.Input != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Input</span><span class=\"detail-value\"><code>%s</code></span></div>\n", act.Input))
			}
			output := act.Output
			if act.OutputArray {
				output += "[]"
			}
			if output == "" {
				output = "void"
			}
			sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Output</span><span class=\"detail-value\"><code>%s</code></span></div>\n", output))
			if act.Query != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Query</span><span class=\"detail-value\"><code>%s</code></span></div>\n", act.Query))
			}
			if len(act.Middleware) > 0 {
				sb.WriteString(fmt.Sprintf("    <div class=\"detail-row\"><span class=\"detail-label\">Middleware</span><span class=\"detail-value\"><code>%s</code></span></div>\n", strings.Join(act.Middleware, ", ")))
			}
			sb.WriteString("  </div>\n</div>\n")
		}
		sb.WriteString("\n")
	}

	// Models
	if len(a.Models) > 0 {
		sb.WriteString("<h2>Models</h2>\n")
		for _, m := range a.Models {
			sb.WriteString(fmt.Sprintf("<div class=\"model-card\" id=\"model-%s\">\n", strings.ToLower(m.Name)))
			sb.WriteString("  <div class=\"model-header\">\n")
			sb.WriteString(fmt.Sprintf("    <h3>%s", m.Name))
			if m.Extends != "" {
				sb.WriteString(fmt.Sprintf("<span class=\"extends\">extends %s</span>", m.Extends))
			}
			sb.WriteString("</h3>\n")
			if m.Description != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"model-desc\">%s</div>\n", m.Description))
			}
			sb.WriteString("  </div>\n")
			if len(m.Fields) > 0 {
				sb.WriteString("  <table class=\"model-table\"><thead><tr><th>Field</th><th>Type</th><th>Attributes</th></tr></thead><tbody>\n")
				for _, f := range m.Fields {
					typeName := f.Type
					if f.IsArray {
						typeName += "[]"
					}
					if f.IsMap {
						typeName = fmt.Sprintf("Map&lt;string, %s&gt;", f.MapValueType)
					}
					attrs := ""
					if f.Optional {
						attrs += "<span class=\"optional-badge\">optional</span> "
					}
					if f.Default != "" {
						attrs += fmt.Sprintf("<span class=\"default-value\">= %s</span>", f.Default)
					}
					if attrs == "" {
						attrs = "&mdash;"
					}
					sb.WriteString(fmt.Sprintf("    <tr><td><strong>%s</strong></td><td><code>%s</code></td><td>%s</td></tr>\n", f.Name, typeName, attrs))
				}
				sb.WriteString("  </tbody></table>\n")
			}
			sb.WriteString("</div>\n")
		}
		sb.WriteString("\n")
	}

	// Enums
	if len(a.Enums) > 0 {
		sb.WriteString("<h2>Enums</h2>\n")
		for _, en := range a.Enums {
			sb.WriteString(fmt.Sprintf("<div class=\"model-card\" id=\"enum-%s\">\n", strings.ToLower(en.Name)))
			sb.WriteString("  <div class=\"model-header\">\n")
			sb.WriteString(fmt.Sprintf("    <h3>%s</h3>\n", en.Name))
			if en.Description != "" {
				sb.WriteString(fmt.Sprintf("    <div class=\"model-desc\">%s</div>\n", en.Description))
			}
			sb.WriteString("  </div>\n")
			sb.WriteString("  <div class=\"enum-values\">\n")
			for _, v := range en.Values {
				sb.WriteString(fmt.Sprintf("    <span class=\"enum-value\">%s</span>\n", v))
			}
			sb.WriteString("  </div>\n</div>\n")
		}
		sb.WriteString("\n")
	}

	sb.WriteString(`<div style="text-align:center;padding:48px 0 24px;color:var(--muted-light);font-size:12px">Generated by Veld</div>
</main>
<script>
function toggleTheme(){
  const html=document.documentElement;
  const isDark=html.getAttribute('data-theme')==='dark';
  html.setAttribute('data-theme',isDark?'':'dark');
  localStorage.setItem('veld-theme',isDark?'':'dark');
}
(function(){const t=localStorage.getItem('veld-theme');if(t)document.documentElement.setAttribute('data-theme',t)})();
function filterNav(q){
  q=q.toLowerCase();
  document.querySelectorAll('.nav-link').forEach(a=>{a.style.display=a.textContent.toLowerCase().includes(q)?'':'none'});
  document.querySelectorAll('.nav-group').forEach(g=>{
    const visible=g.querySelectorAll('.nav-link[style=""],.nav-link:not([style])');
    g.style.display=visible.length||!q?'':'none';
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

	reader := bufio.NewReader(os.Stdin)

	fmt.Println()
	fmt.Println(bold("  Veld") + " — project setup")
	fmt.Println()

	// ── Backend selection ──────────────────────────────────────────────────
	backends := emitter.ListBackends()
	fmt.Println("  " + bold("Backend") + " — which server runtime?")
	for i, b := range backends {
		label := b
		if b == "node" {
			label += dim(" (default)")
		}
		fmt.Printf("    %s%d%s  %s\n", colorGreen, i+1, colorReset, label)
	}
	fmt.Print("\n  Choose [1]: ")
	backendChoice := readChoice(reader, len(backends), 1)
	selectedBackend := backends[backendChoice-1]
	fmt.Printf("  → %s\n\n", green(selectedBackend))

	// ── Frontend selection ─────────────────────────────────────────────────
	frontends := append(emitter.ListFrontends(), "none")
	fmt.Println("  " + bold("Frontend SDK") + " — which client language?")
	for i, f := range frontends {
		label := f
		if f == "typescript" {
			label += dim(" (default)")
		}
		fmt.Printf("    %s%d%s  %s\n", colorGreen, i+1, colorReset, label)
	}
	// Default: find "typescript" index
	defaultFrontend := 1
	for i, f := range frontends {
		if f == "typescript" {
			defaultFrontend = i + 1
			break
		}
	}
	fmt.Printf("\n  Choose [%d]: ", defaultFrontend)
	frontendChoice := readChoice(reader, len(frontends), defaultFrontend)
	selectedFrontend := frontends[frontendChoice-1]
	fmt.Printf("  → %s\n\n", green(selectedFrontend))

	// ── Runtime validation ─────────────────────────────────────────────────
	// Only relevant for node and python — statically-typed backends (go, rust,
	// java, csharp) already enforce contract correctness at compile time.
	enableValidate := false
	if selectedBackend == "node" || selectedBackend == "python" {
		fmt.Println("  " + bold("Runtime validation") + " — validate input/output shapes at runtime?")
		fmt.Printf("    %s1%s  disabled %s\n", colorGreen, colorReset, dim("(default — zero overhead, TypeScript/Python types enforce the contract)"))
		fmt.Printf("    %s2%s  enabled  %s\n", colorGreen, colorReset, dim("(adds zero-dep validators to routes: 400 on bad input, 500 on contract violation)"))
		fmt.Print("\n  Choose [1]: ")
		if readChoice(reader, 2, 1) == 2 {
			enableValidate = true
		}
		if enableValidate {
			fmt.Printf("  → %s\n\n", green("enabled"))
		} else {
			fmt.Printf("  → %s\n\n", dim("disabled"))
		}
	}

	// ── Generate config with selections ────────────────────────────────────
	// For Python, default to "veld_gen" as the output directory name
	// so the folder itself is a valid Python package importable from cwd.
	defaultOut := "../generated"
	if selectedBackend == "python" {
		defaultOut = "../veld_gen"
	}

	validateLine := ""
	if enableValidate {
		validateLine = "\n  \"validate\": true,"
	}

	configJSON := fmt.Sprintf(`{
  "input": "app.veld",
  "backend": "%s",
  "frontend": "%s",
  "out": "%s",
  "backendDir": "",
  "frontendDir": "",
  "baseUrl": "",%s
  "aliases": {
    "models": "models",
    "modules": "modules"
  }
}
`, selectedBackend, selectedFrontend, defaultOut, validateLine)

	type entry struct{ path, content, label string }
	files := []entry{
		{"veld/veld.config.json", configJSON, "veld/veld.config.json"},
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
		fmt.Printf("  "+green("✓")+" %s\n", f.label)
	}

	fmt.Println()
	fmt.Println("  " + bold("Veld project ready."))
	fmt.Printf("    backend:  %s\n", bold(selectedBackend))
	fmt.Printf("    frontend: %s\n", bold(selectedFrontend))
	fmt.Println()

	// ── Setup prompt ──────────────────────────────────────────────────────
	fmt.Print("  Run setup to configure imports? [Y/n]: ")
	setupLine, _ := reader.ReadString('\n')
	setupLine = strings.TrimSpace(strings.ToLower(setupLine))
	if setupLine == "" || setupLine == "y" || setupLine == "yes" {
		var backendDirPath, frontendDirPath string

		// ── Ask for backend project directory ──────────────────────────
		fmt.Println()
		fmt.Print("  " + bold("Backend project directory") + dim(" (leave empty for current dir)") + ": ")
		bLine, _ := reader.ReadString('\n')
		bLine = strings.TrimSpace(bLine)
		if bLine != "" {
			abs, err := filepath.Abs(bLine)
			if err == nil {
				backendDirPath = abs
			}
		}

		// ── Ask for frontend project directory ─────────────────────────
		if selectedFrontend != "none" {
			fmt.Print("  " + bold("Frontend project directory") + dim(" (leave empty for current dir)") + ": ")
			fLine, _ := reader.ReadString('\n')
			fLine = strings.TrimSpace(fLine)
			if fLine != "" {
				abs, err := filepath.Abs(fLine)
				if err == nil {
					frontendDirPath = abs
				}
			}
		}

		// ── Update config file with backendDir / frontendDir ───────────
		if backendDirPath != "" || frontendDirPath != "" {
			cfgDir, _ := filepath.Abs("veld")
			relBackend := ""
			relFrontend := ""
			relBackendOut := ""
			relFrontendOut := ""
			if backendDirPath != "" {
				if r, err := filepath.Rel(cfgDir, backendDirPath); err == nil {
					relBackend = filepath.ToSlash(r)
				} else {
					relBackend = filepath.ToSlash(backendDirPath)
				}
			}
			if frontendDirPath != "" {
				if r, err := filepath.Rel(cfgDir, frontendDirPath); err == nil {
					relFrontend = filepath.ToSlash(r)
				} else {
					relFrontend = filepath.ToSlash(frontendDirPath)
				}
			}

			// When backend and frontend are in different directories, auto-set
			// backendOut / frontendOut so generated code lands inside each project.
			if backendDirPath != "" && frontendDirPath != "" && backendDirPath != frontendDirPath {
				genName := "generated"
				if selectedBackend == "python" {
					genName = "veld_gen"
				}
				relBackendOut = relBackend + "/src/" + genName
				relFrontendOut = relFrontend + "/src/" + genName

				fmt.Println()
				fmt.Println(dim("  Split output detected:"))
				fmt.Printf("    backend output:  %s\n", bold(relBackendOut))
				fmt.Printf("    frontend output: %s\n", bold(relFrontendOut))
			}

			backendOutLine := ""
			frontendOutLine := ""
			if relBackendOut != "" {
				backendOutLine = fmt.Sprintf("\n  \"backendOut\": \"%s\",", relBackendOut)
			}
			if relFrontendOut != "" {
				frontendOutLine = fmt.Sprintf("\n  \"frontendOut\": \"%s\",", relFrontendOut)
			}

			updatedCfg := fmt.Sprintf(`{
  "input": "app.veld",
  "backend": "%s",
  "frontend": "%s",
  "out": "%s",%s%s
  "backendDir": "%s",
  "frontendDir": "%s",
  "aliases": {
    "models": "models",
    "modules": "modules"
  }
}
`, selectedBackend, selectedFrontend, defaultOut, backendOutLine, frontendOutLine, relBackend, relFrontend)
			_ = os.WriteFile("veld/veld.config.json", []byte(updatedCfg), 0644)
			fmt.Println("  " + green("✓") + " updated veld.config.json with project paths")
		}

		// ── Run setup ──────────────────────────────────────────────────
		projectDir, _ := os.Getwd()
		setupOpts := setup.Options{
			BackendDir:  backendDirPath,
			FrontendDir: frontendDirPath,
		}
		// If split output was detected, compute absolute paths for setup
		if backendDirPath != "" && frontendDirPath != "" && backendDirPath != frontendDirPath {
			genName := "generated"
			if selectedBackend == "python" {
				genName = "veld_gen"
			}
			setupOpts.BackendOutDir = filepath.Join(backendDirPath, "src", genName)
			setupOpts.FrontendOutDir = filepath.Join(frontendDirPath, "src", genName)
		}
		results := setup.Run(projectDir, selectedBackend, selectedFrontend, defaultOut, setupOpts)
		if len(results) > 0 {
			printSetupResults(results)
		}
	}

	fmt.Println()
	fmt.Println("  Next steps:")
	fmt.Println("    1. Edit veld/models/ and veld/modules/ to define your API")
	fmt.Println("    2. Run: " + bold("veld generate"))
	return nil
}

// readChoice reads a number from stdin, returning def if the user presses Enter.
func readChoice(reader *bufio.Reader, max, def int) int {
	line, _ := reader.ReadString('\n')
	line = strings.TrimSpace(line)
	if line == "" {
		return def
	}
	n, err := strconv.Atoi(line)
	if err != nil || n < 1 || n > max {
		return def
	}
	return n
}

// ── init templates ────────────────────────────────────────────────────────────

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

package main

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/cache"
	"github.com/Adhamzineldin/Veld/internal/config"
	"github.com/Adhamzineldin/Veld/internal/diff"
	"github.com/Adhamzineldin/Veld/internal/docsgen"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	vfmt "github.com/Adhamzineldin/Veld/internal/format"
	"github.com/Adhamzineldin/Veld/internal/graphqlgen"
	"github.com/Adhamzineldin/Veld/internal/lint"
	"github.com/Adhamzineldin/Veld/internal/loader"
	"github.com/Adhamzineldin/Veld/internal/lsp"
	"github.com/Adhamzineldin/Veld/internal/openapigen"
	"github.com/Adhamzineldin/Veld/internal/registry"
	"github.com/Adhamzineldin/Veld/internal/schema"
	"github.com/Adhamzineldin/Veld/internal/server"
	"github.com/Adhamzineldin/Veld/internal/server/email"
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
	_ "github.com/Adhamzineldin/Veld/internal/generators/openapi"
	_ "github.com/Adhamzineldin/Veld/internal/generators/scaffold"
)

// Version is the current Veld CLI version.
// Version is set at build time via: go build -ldflags "-X main.Version=v1.2.3"
// Falls back to "dev" for local builds without ldflags.
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

func green(s string) string  { return colorGreen + s + colorReset }
func red(s string) string    { return colorRed + s + colorReset }
func yellow(s string) string { return colorYellow + s + colorReset }
func dim(s string) string    { return colorDim + s + colorReset }
func bold(s string) string   { return colorBold + s + colorReset }

// ── shared generation logic ───────────────────────────────────────────────────

// runPostGenerate executes the postGenerate hook if configured.
func runPostGenerate(rc config.ResolvedConfig) {
	if rc.PostGenerate == "" {
		return
	}
	fmt.Printf(dim("⚙")+"  Running postGenerate: %s\n", rc.PostGenerate)
	cmd := exec.Command("sh", "-c", rc.PostGenerate)
	cmd.Dir = rc.ConfigDir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		fmt.Fprintf(os.Stderr, yellow("warning: ")+"postGenerate hook failed: %v\n", err)
	}
}

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
	backendOrTool, isRealBackend, err := emitter.GetBackendOrTool(rc.Backend)
	if err != nil {
		return nil, veldFiles, nil, err
	}
	if err := backendOrTool.Emit(emitAST, rc.BackendOut, opts); err != nil {
		return nil, veldFiles, nil, fmt.Errorf("%s emitter: %w", rc.Backend, err)
	}
	// When using split output, also emit backend (types, errors, interfaces,
	// routes) into the frontend output dir so the frontend SDK is fully
	// self-contained — no cross-directory imports needed.
	if isRealBackend && rc.SplitOutput() && !opts.DryRun {
		if err := backendOrTool.Emit(emitAST, rc.FrontendOut, opts); err != nil {
			return nil, veldFiles, nil, fmt.Errorf("%s emitter (frontend copy): %w", rc.Backend, err)
		}
	}

	// ── emit: frontend ───────────────────────────────────────────────────
	// New syntax: --frontend=typescript --frontend-framework=react
	// Routes "typescript" + frontendFramework="react" to the existing "react" emitter.
	frontendName := rc.Frontend
	if opts.FrontendFramework != "" && (frontendName == "typescript" || frontendName == "javascript") {
		frontendName = opts.FrontendFramework
	}
	frontend, err := emitter.GetFrontend(frontendName)
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

// runGenerateWithAST is like runGenerate but takes a pre-built AST instead of
// calling loader.Parse. Used by frontend workspace entries whose AST has been
// merged from all consumed backend services.
func runGenerateWithAST(rc config.ResolvedConfig, a ast.AST, opts emitter.EmitOptions) error {
	if opts.DryRun {
		return nil
	}

	emitAST := a
	if emitAST.Prefix != "" {
		for i := range emitAST.Modules {
			emitAST.Modules[i].Prefix = emitAST.Prefix + emitAST.Modules[i].Prefix
		}
	}

	// Emit backend (types, routes, etc.).
	backendOrTool, isRealBackend, err := emitter.GetBackendOrTool(rc.Backend)
	if err != nil {
		return err
	}
	if err := backendOrTool.Emit(emitAST, rc.BackendOut, opts); err != nil {
		return fmt.Errorf("%s emitter: %w", rc.Backend, err)
	}
	if isRealBackend && rc.SplitOutput() {
		if err := backendOrTool.Emit(emitAST, rc.FrontendOut, opts); err != nil {
			return fmt.Errorf("%s emitter (frontend copy): %w", rc.Backend, err)
		}
	}

	// Emit frontend SDK.
	frontendName := rc.Frontend
	if opts.FrontendFramework != "" && (frontendName == "typescript" || frontendName == "javascript") {
		frontendName = opts.FrontendFramework
	}
	frontend, err := emitter.GetFrontend(frontendName)
	if err != nil {
		return err
	}
	if frontend != nil {
		if err := frontend.Emit(emitAST, rc.FrontendOut, opts); err != nil {
			return fmt.Errorf("%s emitter: %w", rc.Frontend, err)
		}
	}

	// Generated README.
	for _, dir := range rc.OutputDirs() {
		writeGeneratedReadme(dir, emitAST)
	}
	return nil
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
	if be, _, err := emitter.GetBackendOrTool(rc.Backend); err == nil {
		if s, ok := be.(emitter.Summarizer); ok {
			for _, line := range s.Summary(modules) {
				fmt.Printf("  %s  %s\n", dim(line.Dir), line.Files)
			}
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
		case "node-ts":
			fmt.Println(dim("    Setup:") + ` run ` + bold("veld setup") + ` then ` + bold("npm install"))
			fmt.Println(dim("    Types:    ") + ` import { User } from '@veld/generated/types';`)
			fmt.Println(dim("    Routes:   ") + ` import { usersRoutes } from '@veld/generated/routes/users.routes';`)
			fmt.Println(dim("    Interfaces:") + ` import { IUsersService } from '@veld/generated/interfaces/IUsersService';`)
		case "node-js":
			fmt.Println(dim("    Setup:") + ` run ` + bold("veld setup") + ` then ` + bold("npm install"))
			fmt.Println(dim("    Types:    ") + ` const { User } = require('@veld/generated/types');  // JSDoc typedefs`)
			fmt.Println(dim("    Routes:   ") + ` const { usersRouter } = require('@veld/generated/routes/users.routes');`)
			fmt.Println(dim("    Interfaces:") + ` // JSDoc @typedef in interfaces/IUsersService.js`)
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
			fmt.Println(dim("    Setup:") + ` run ` + bold("veld setup") + ` (adds build-helper-maven-plugin to pom.xml)`)
			fmt.Println(dim("    Types:    ") + ` import maayn.veld.generated.models.User;`)
			fmt.Println(dim("    Routes:   ") + ` import maayn.veld.generated.controllers.UsersController;`)
			fmt.Println(dim("    Interfaces:") + ` import maayn.veld.generated.services.IUsersService;`)
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

			// Workspace mode: run setup for each entry that has a backend or frontend.
			if len(rc.Workspace) > 0 {
				anyPatched := false
				for _, wEntry := range rc.Workspace {
					beTarget := wEntry.Backend
					if beTarget == "" && wEntry.BackendCfg != nil {
						beTarget = wEntry.BackendCfg.Target
					}
					feTarget := wEntry.Frontend
					if feTarget == "" && wEntry.FrontendCfg != nil {
						feTarget = wEntry.FrontendCfg.Target
					}
					if beTarget == "" && feTarget == "" {
						continue
					}
					outDir := wEntry.Out
					if outDir == "" && wEntry.BackendCfg != nil && wEntry.BackendCfg.Out != "" {
						outDir = wEntry.BackendCfg.Out
					}
					if outDir == "" {
						outDir = filepath.Join(rc.ConfigDir, "generated", wEntry.Name)
					} else if !filepath.IsAbs(outDir) {
						outDir = filepath.Clean(filepath.Join(rc.ConfigDir, outDir))
					}
					// Explicit backendConfig.dir / frontendConfig.dir take priority.
					// When absent, setup.Run auto-detects the service root by walking
					// up from outDir looking for pom.xml / package.json / go.mod / etc.
					beDir := ""
					if wEntry.BackendCfg != nil && wEntry.BackendCfg.Dir != "" {
						beDir = filepath.Clean(filepath.Join(rc.ConfigDir, wEntry.BackendCfg.Dir))
					}
					feOutDir := outDir
					if wEntry.FrontendCfg != nil && wEntry.FrontendCfg.Out != "" {
						feOutDir = filepath.Clean(filepath.Join(rc.ConfigDir, wEntry.FrontendCfg.Out))
					}
					feDir := ""
					if wEntry.FrontendCfg != nil && wEntry.FrontendCfg.Dir != "" {
						feDir = filepath.Clean(filepath.Join(rc.ConfigDir, wEntry.FrontendCfg.Dir))
					}

					entryResults := setup.Run(projectDir, beTarget, feTarget, outDir, setup.Options{
						BackendDir:     beDir,
						FrontendDir:    feDir,
						BackendOutDir:  outDir,
						FrontendOutDir: feOutDir,
					})
					if len(entryResults) > 0 {
						fmt.Printf("  %s %s\n", bold("→"), wEntry.Name)
						printSetupResults(entryResults)
						anyPatched = true
					}
				}
				if !anyPatched {
					fmt.Println(dim("  No config files to patch for this workspace"))
				}
				return nil
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

func newGenerateCmd() *cobra.Command {
	var backendFlag, frontendFlag, inputFlag, outFlag string
	var backendFrameworkFlag, frontendFrameworkFlag string
	var incrementalFlag, dryRunFlag, setupFlag, validateFlag, strictFlag, forceFlag, serverSdkFlag, serviceSdkFlag bool

	cmd := &cobra.Command{
		Use:   "generate",
		Short: "Generate code from a .veld contract",
		Long: "Generates typed backend interfaces and a frontend SDK from your .veld contract.\n\n" +
			"Every file is (re)generated by default — deterministic and safe for CI/CD.\n" +
			"Pass --incremental to skip modules whose source files have not changed\n" +
			"(intended for local development, not production pipelines).\n\n" +
			"Backends: node-ts, node-js, python, go, rust, java, csharp, php,\n" +
			"          openapi, database, dockerfile, cicd, env, scaffold-tests\n" +
			"Frontends: typescript, javascript, react, vue, angular, svelte,\n" +
			"           dart, kotlin, swift, types-only, none\n" +
			"Aliases:   node → node-ts, js/javascript → node-js",
		Example: "  veld generate\n" +
			"  veld generate --backend=node --frontend=react\n" +
			"  veld generate --backend=go --frontend=vue\n" +
			"  veld generate --frontend=types-only\n" +
			"  veld generate --backend=openapi\n" +
			"  veld generate --backend=dockerfile\n" +
			"  veld generate --dry-run",
		RunE: func(cmd *cobra.Command, args []string) error {
			flags := config.FlagOverrides{
				Backend:              backendFlag,
				Frontend:             frontendFlag,
				Input:                inputFlag,
				Out:                  outFlag,
				Validate:             validateFlag,
				BackendFramework:     backendFrameworkFlag,
				FrontendFramework:    frontendFrameworkFlag,
				BackendSet:           cmd.Flags().Changed("backend"),
				FrontendSet:          cmd.Flags().Changed("frontend"),
				InputSet:             cmd.Flags().Changed("input"),
				OutSet:               cmd.Flags().Changed("out"),
				ValidateSet:          cmd.Flags().Changed("validate"),
				BackendFrameworkSet:  cmd.Flags().Changed("backend-framework"),
				FrontendFrameworkSet: cmd.Flags().Changed("frontend-framework"),
			}
			rc, err := config.BuildResolved(flags)
			if err != nil {
				return err
			}

			if serverSdkFlag {
				rc.ServerSdk = true
			}

			opts := emitter.EmitOptions{
				BaseUrl:           rc.BaseUrl,
				DryRun:            dryRunFlag,
				Validate:          rc.Validate,
				BackendFramework:  rc.BackendFramework,
				FrontendFramework: rc.FrontendFramework,
				Services:          rc.Services,
				ServerSdk:         rc.ServerSdk,
				Description:       rc.Description,
			}

			// ── Workspace multi-service mode ─────────────────────────────────────
			if len(rc.Workspace) > 0 {
				fmt.Printf("\n%s workspace: %d services\n\n", bold("◆"), len(rc.Workspace))

				// ── Validate consumes declarations ───────────────────────────────
				if errs, warns := validator.ValidateWorkspaceConsumes(rc.Workspace); len(errs) > 0 || len(warns) > 0 {
					for _, w := range warns {
						fmt.Fprintf(os.Stderr, yellow("⚠")+"  %s\n", w)
					}
					for _, e := range errs {
						fmt.Fprintf(os.Stderr, red("✗")+"  %s\n", e)
					}
					if len(errs) > 0 {
						return fmt.Errorf("workspace validation failed")
					}
				}

				// ── Pass 1: parse all workspace entries and cache ASTs ───────────
				type parsedEntry struct {
					rc     config.ResolvedConfig
					ast    ast.AST
					flags  config.FlagOverrides
					outDir string
				}
				parsed := make(map[string]*parsedEntry, len(rc.Workspace))

				for _, entry := range rc.Workspace {
					entryFlags := flags
					entryFlags.InputSet = true
					entryFlags.Input = filepath.Join(rc.ConfigDir, entry.Input)

					// Support both flat (entry.Backend) and nested (entry.BackendCfg.Target) format.
					backendTarget := entry.Backend
					if backendTarget == "" && entry.BackendCfg != nil && entry.BackendCfg.Target != "" {
						backendTarget = entry.BackendCfg.Target
					}
					if backendTarget != "" {
						entryFlags.BackendSet = true
						entryFlags.Backend = backendTarget
					}

					frontendTarget := entry.Frontend
					if frontendTarget == "" && entry.FrontendCfg != nil && entry.FrontendCfg.Target != "" {
						frontendTarget = entry.FrontendCfg.Target
					}
					if frontendTarget != "" {
						entryFlags.FrontendSet = true
						entryFlags.Frontend = frontendTarget
					}

					// Apply per-entry framework from nested config.
					if entry.BackendCfg != nil && entry.BackendCfg.Framework != "" {
						entryFlags.BackendFrameworkSet = true
						entryFlags.BackendFramework = entry.BackendCfg.Framework
					}

					// Resolve out directory: check flat Out, then BackendCfg.Out.
					// Always make absolute relative to rc.ConfigDir BEFORE passing as a flag,
					// because BuildResolved resets cfgDir to CWD when InputSet=true.
					outDir := entry.Out
					if outDir == "" && entry.BackendCfg != nil && entry.BackendCfg.Out != "" {
						outDir = entry.BackendCfg.Out
					}
					if outDir == "" {
						outDir = filepath.Join(rc.ConfigDir, "generated", entry.Name)
					} else if !filepath.IsAbs(outDir) {
						outDir = filepath.Clean(filepath.Join(rc.ConfigDir, outDir))
					}
					entryFlags.OutSet = true
					entryFlags.Out = outDir

					// Resolve split frontend out if specified.
					if entry.FrontendCfg != nil && entry.FrontendCfg.Out != "" {
						feOut := entry.FrontendCfg.Out
						if !filepath.IsAbs(feOut) {
							feOut = filepath.Clean(filepath.Join(rc.ConfigDir, feOut))
						}
						entryFlags.FrontendOutSet = true
						entryFlags.FrontendOut = feOut
					}

					entryRC, err := config.BuildResolved(entryFlags)
					if err != nil {
						return fmt.Errorf("workspace entry %q: %w", entry.Name, err)
					}
					if entry.BaseUrl != "" {
						entryRC.BaseUrl = entry.BaseUrl
					}
					entryRC.ServerSdk = entry.ServerSdk || rc.ServerSdk
					entryRC.Description = rc.Description
					entryRC.Services = rc.Services

					// Build a merged alias map that includes cross-service aliases.
					// This enables `import @iam/models/*` in any sibling service.
					// @<service-name> → absolute path to that service's source directory
					// (two directories up from its input file, e.g. services/iam/ for
					// services/iam/modules/iam.veld).
					crossAliases := make(map[string]string, len(entryRC.Aliases)+len(rc.Workspace))
					for k, v := range entryRC.Aliases {
						crossAliases[k] = v
					}
					for _, sibling := range rc.Workspace {
						if sibling.Name == entry.Name || sibling.Input == "" {
							continue
						}
						siblingInput := sibling.Input
						if !filepath.IsAbs(siblingInput) {
							siblingInput = filepath.Join(rc.ConfigDir, siblingInput)
						}
						// Service root is two levels up: services/iam/modules/iam.veld → services/iam/
						serviceRoot := filepath.Dir(filepath.Dir(siblingInput))
						crossAliases[sibling.Name] = serviceRoot
					}

					// Parse the AST for this entry (needed for consumes resolution).
					// For frontend-only entries, the input file may not exist — the
					// frontend AST will be built entirely from consumed services.
					isFrontend := frontendTarget != "" && frontendTarget != "none"
					a, _, err := loader.Parse(entryRC.Input, crossAliases)
					if err != nil && isFrontend {
						// Frontend entry with missing input file — use empty AST.
						// It will be populated via MergeASTs from consumed services.
						a = ast.AST{ASTVersion: "1.0.0"}
					} else if err != nil {
						return fmt.Errorf("workspace entry %q: %w", entry.Name, err)
					}

					parsed[entry.Name] = &parsedEntry{
						rc:     entryRC,
						ast:    a,
						flags:  entryFlags,
						outDir: outDir,
					}
				}

				// ── Pass 2: generate each entry with consumed service resolution ─
				for _, entry := range rc.Workspace {
					fmt.Printf("  %s %s\n", bold("→"), entry.Name)
					pe := parsed[entry.Name]

					entryOpts := emitter.EmitOptions{
						BaseUrl:           pe.rc.BaseUrl,
						DryRun:            dryRunFlag,
						Validate:          pe.rc.Validate,
						BackendFramework:  pe.rc.BackendFramework,
						FrontendFramework: pe.rc.FrontendFramework,
						Services:          pe.rc.Services,
						ServerSdk:         pe.rc.ServerSdk,
						Description:       pe.rc.Description,
					}

					// Resolve consumed services from the parsed AST cache.
					consumesList := entry.Consumes

					// Auto-consume logic:
					// 1. --service-sdk flag: backend entries consume ALL other siblings.
					// 2. Frontend entries (frontend != "none") ALWAYS consume all
					//    backend siblings so the frontend SDK covers every service.
					isFrontendEntry := entry.Frontend != "" && entry.Frontend != "none"
					if isFrontendEntry || (serviceSdkFlag && len(consumesList) == 0) {
						if isFrontendEntry || len(consumesList) == 0 {
							consumesList = nil // rebuild fresh
							for _, other := range rc.Workspace {
								if other.Name != entry.Name && (other.Frontend == "" || other.Frontend == "none") {
									consumesList = append(consumesList, other.Name)
								}
							}
						}
					}

					if len(consumesList) > 0 {
						var consumed []emitter.ConsumedServiceInfo
						for _, depName := range consumesList {
							dep := parsed[depName]
							if dep == nil {
								continue // validated above
							}
							consumed = append(consumed, emitter.ConsumedServiceInfo{
								Name:    depName,
								AST:     dep.ast,
								BaseUrl: dep.rc.BaseUrl,
							})
						}
						entryOpts.ConsumedServices = consumed
					}

					// For frontend entries: merge all consumed service ASTs into
					// the frontend's own AST so the frontend SDK (React/Vue/Angular/etc.)
					// gets typed clients for EVERY service in one unified import.
					// Also build a per-module Services map so each module's SDK
					// client points to the correct service URL.
					if isFrontendEntry && len(entryOpts.ConsumedServices) > 0 {
						mergedAST := emitter.MergeASTs(pe.ast, entryOpts.ConsumedServices)
						pe.ast = mergedAST

						// Re-parse into the loader isn't needed — we override the
						// resolved config's input AST by calling runGenerateWithAST.

						// Build per-module base URL map so the frontend SDK knows
						// which URL goes to which service module.
						if entryOpts.Services == nil {
							entryOpts.Services = make(map[string]string)
						}
						for _, consumed := range entryOpts.ConsumedServices {
							for _, mod := range consumed.AST.Modules {
								if consumed.BaseUrl != "" {
									entryOpts.Services[mod.Name] = consumed.BaseUrl
								}
							}
						}
					}

					if isFrontendEntry {
						// Frontend entries: skip loader.Parse (the input file may not
						// exist if the frontend is fully driven by consumed services).
						// Emit directly with the (possibly merged) AST.
						if err := runGenerateWithAST(pe.rc, pe.ast, entryOpts); err != nil {
							return fmt.Errorf("workspace entry %q: %w", entry.Name, err)
						}
					} else {
						if _, _, _, err := runGenerate(pe.rc, false, entryOpts); err != nil {
							return fmt.Errorf("workspace entry %q: %w", entry.Name, err)
						}
					}

					// Emit service SDK clients for consumed services via the backend emitter.
					// (Only for backend entries — the frontend already got everything via AST merge.)
					if !isFrontendEntry && len(entryOpts.ConsumedServices) > 0 {
						backend, err := emitter.GetBackend(pe.rc.Backend)
						if err != nil {
							return fmt.Errorf("workspace entry %q: %w", entry.Name, err)
						}
						if err := backend.EmitServiceSdk(entryOpts.ConsumedServices, pe.rc.BackendOut, entryOpts); err != nil {
							return fmt.Errorf("workspace entry %q service sdk: %w", entry.Name, err)
						}
						consumedNames := make([]string, len(entryOpts.ConsumedServices))
						for i, c := range entryOpts.ConsumedServices {
							consumedNames[i] = c.Name
						}
						fmt.Printf("  %s %s → sdk/ (%s)\n", green("✓"), entry.Name, strings.Join(consumedNames, ", "))
					} else if isFrontendEntry && len(entryOpts.ConsumedServices) > 0 {
						consumedNames := make([]string, len(entryOpts.ConsumedServices))
						for i, c := range entryOpts.ConsumedServices {
							consumedNames[i] = c.Name
						}
						fmt.Printf("  %s %s → unified frontend SDK (%s)\n", green("✓"), entry.Name, strings.Join(consumedNames, ", "))
					} else {
						fmt.Printf("  %s %s → %s\n", green("✓"), entry.Name, pe.outDir)
					}
				}
				fmt.Printf("\n%s All services generated\n", green("✓"))
				return nil
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

			// Run postGenerate hook if configured.
			if !dryRunFlag {
				runPostGenerate(rc)
			}
			return nil
		},
	}
	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend target ("+strings.Join(emitter.ListAllTargets(), ", ")+")")
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
	cmd.Flags().StringVar(&backendFrameworkFlag, "backend-framework", "",
		"framework for the backend emitter (express, flask, chi, spring, axum, aspnet, laravel — default: plain/none)")
	cmd.Flags().StringVar(&frontendFrameworkFlag, "frontend-framework", "",
		"framework wrapper for the frontend SDK (react, vue, angular, svelte — default: none)")
	cmd.Flags().BoolVar(&serverSdkFlag, "server-sdk", false,
		"also emit a server-to-server typed client in generated/server-client/")
	cmd.Flags().BoolVar(&serviceSdkFlag, "service-sdk", false,
		"generate typed service SDKs for all workspace siblings (inter-service communication)")
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
	// Apply app-level prefix to module prefixes so the comparison matches
	// what was persisted in the lock file (runGenerate mutates module
	// prefixes before SaveLock).
	if a.Prefix != "" {
		for i := range a.Modules {
			a.Modules[i].Prefix = a.Prefix + a.Modules[i].Prefix
		}
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
		Long: "Watches all .veld files and the config for changes, then performs a full\n" +
			"regeneration of all outputs. This ensures shared artifacts (types, barrels,\n" +
			"middleware, _internal.ts) are always consistent. Safe to run during development.",
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

			// ── initial full generation (never incremental) ─────────────
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
			runPostGenerate(rc)
			fmt.Println()

			// ── build the watched file set ──────────────────────────────
			// Includes all .veld files + the config file itself.
			// In workspace mode, runGenerate returns no files; collect them here.
			if len(rc.Workspace) > 0 && len(initFiles) == 0 {
				for _, wEntry := range rc.Workspace {
					if wEntry.Input == "" {
						continue
					}
					entryInput := wEntry.Input
					if !filepath.IsAbs(entryInput) {
						entryInput = filepath.Join(rc.ConfigDir, entryInput)
					}
					_ = filepath.Walk(filepath.Dir(entryInput), func(p string, info os.FileInfo, _ error) error {
						if info != nil && !info.IsDir() && strings.HasSuffix(p, ".veld") {
							absP, _ := filepath.Abs(p)
							initFiles = append(initFiles, absP)
						}
						return nil
					})
				}
				// Also include shared files in the config root.
				_ = filepath.Walk(rc.ConfigDir, func(p string, info os.FileInfo, _ error) error {
					if info != nil && !info.IsDir() && strings.HasSuffix(p, ".veld") {
						absP, _ := filepath.Abs(p)
						initFiles = append(initFiles, absP)
					}
					return nil
				})
			}
			var mtimesMu sync.Mutex
			mtimes := make(map[string]int64, len(initFiles)+2)
			for _, f := range initFiles {
				if info, statErr := os.Stat(f); statErr == nil {
					mtimes[f] = info.ModTime().UnixNano()
				}
			}
			// Also watch the config file(s).
			configCandidates := []string{
				filepath.Join(rc.ConfigDir, "veld.config.json"),
				filepath.Join(rc.ConfigDir, "veld", "veld.config.json"),
			}
			for _, cf := range configCandidates {
				if info, statErr := os.Stat(cf); statErr == nil {
					mtimes[cf] = info.ModTime().UnixNano()
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
					var changedNames []string
					configChanged := false

					mtimesMu.Lock()

					// Check existing tracked files for modifications.
					for f, last := range mtimes {
						info, statErr := os.Stat(f)
						if statErr != nil {
							// File was deleted — still counts as a change.
							changedNames = append(changedNames, filepath.Base(f))
							continue
						}
						if info.ModTime().UnixNano() != last {
							changedNames = append(changedNames, filepath.Base(f))
							if strings.HasSuffix(f, "veld.config.json") {
								configChanged = true
							}
						}
					}

					// Discover NEW .veld files that didn't exist at startup.
					// Re-scan input directories for any new .veld files.
					// In workspace mode scan each service's input directory.
					var scanDirs []string
					if len(rc.Workspace) > 0 {
						for _, wEntry := range rc.Workspace {
							if wEntry.Input != "" {
								entryInput := wEntry.Input
								if !filepath.IsAbs(entryInput) {
									entryInput = filepath.Join(rc.ConfigDir, entryInput)
								}
								scanDirs = append(scanDirs, filepath.Dir(entryInput))
							}
						}
						// Also scan the veld source root for shared models/enums.
						scanDirs = append(scanDirs, rc.ConfigDir)
					} else if rc.Input != "" {
						scanDirs = append(scanDirs, filepath.Dir(rc.Input))
					}
					for _, scanDir := range scanDirs {
						_ = filepath.Walk(scanDir, func(path string, info os.FileInfo, walkErr error) error {
							if walkErr != nil || info.IsDir() {
								return nil
							}
							if strings.HasSuffix(path, ".veld") {
								absPath, _ := filepath.Abs(path)
								if _, tracked := mtimes[absPath]; !tracked {
									// New file found — treat as a change.
									mtimes[absPath] = info.ModTime().UnixNano()
									changedNames = append(changedNames, filepath.Base(absPath))
								}
							}
							return nil
						})
					}

					if len(changedNames) > 0 {
						// Update all mtimes immediately to avoid re-triggering.
						for f := range mtimes {
							if info, statErr := os.Stat(f); statErr == nil {
								mtimes[f] = info.ModTime().UnixNano()
							}
						}
					}
					mtimesMu.Unlock()

					if len(changedNames) == 0 {
						continue
					}

					// Debounce: reset timer on every change, fire after 300ms of quiet.
					if debounceTimer != nil {
						debounceTimer.Stop()
					}

					// Capture for the closure.
					capturedChanged := changedNames
					capturedConfigChanged := configChanged

					debounceTimer = time.AfterFunc(300*time.Millisecond, func() {
						ts := dim("[" + time.Now().Format("15:04:05") + "]")

						// ── reload config if it changed ─────────────────
						currentRC := rc
						if capturedConfigChanged {
							fmt.Printf("%s %s config changed, reloading...\n", ts, yellow("⟳"))
							newRC, reloadErr := config.BuildResolved(flags)
							if reloadErr != nil {
								fmt.Fprintf(os.Stderr, "%s %s failed to reload config: %v\n", ts, red("✗"), reloadErr)
								return
							}
							rc = newRC
							currentRC = newRC
							// Update emit options from new config.
							opts = emitter.EmitOptions{
								BaseUrl:  currentRC.BaseUrl,
								Validate: currentRC.Validate,
							}
						}

						// ── always full regeneration ────────────────────
						// Watch mode NEVER does incremental generation.
						// Any .veld change can affect shared types, barrels,
						// middleware interfaces, _internal.ts, error _base.ts,
						// cross-module type imports, and app-level prefix.
						// A full regen takes <100ms for typical projects.
						fmt.Printf("%s %s change in %s — regenerating all...\n",
							ts, yellow("⟳"), strings.Join(dedup(capturedChanged), ", "))

						start := time.Now()
						regen, newFiles, changes, genErr := runGenerate(currentRC, false, opts)

						if genErr != nil {
							if !lastError {
								fmt.Fprintf(os.Stderr, "%s %s %v\n", ts, red("error:"), genErr)
								fmt.Println()
							}
							lastError = true
						} else {
							elapsed := time.Since(start).Round(time.Millisecond)
							if regen == nil || len(regen) == 0 {
								fmt.Printf("%s %s nothing to regenerate (%s)\n", ts, green("✓"), elapsed)
							} else {
								fmt.Printf("%s %s regenerated %s (%s)\n", ts, green("✓"), strings.Join(regen, ", "), elapsed)
								printDiffChanges(changes)
							}
							runPostGenerate(currentRC)
							fmt.Println()
							lastError = false
						}

						// Refresh tracked file set — picks up new/deleted .veld files.
						if newFiles != nil {
							mtimesMu.Lock()
							// Rebuild mtimes from scratch with new file list + config.
							fresh := make(map[string]int64, len(newFiles)+2)
							for _, f := range newFiles {
								if info, statErr := os.Stat(f); statErr == nil {
									fresh[f] = info.ModTime().UnixNano()
								}
							}
							for _, cf := range configCandidates {
								if info, statErr := os.Stat(cf); statErr == nil {
									fresh[cf] = info.ModTime().UnixNano()
								}
							}
							mtimes = fresh
							mtimesMu.Unlock()
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

// dedup returns a slice with duplicate strings removed, preserving order.
func dedup(ss []string) []string {
	seen := make(map[string]bool, len(ss))
	out := make([]string, 0, len(ss))
	for _, s := range ss {
		if !seen[s] {
			seen[s] = true
			out = append(out, s)
		}
	}
	return out
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

	sb.WriteString("## Modules\n\n")
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("- **%s**", mod.Name))
		if mod.Description != "" {
			sb.WriteString(fmt.Sprintf(" — %s", mod.Description))
		}
		sb.WriteString(fmt.Sprintf(" (%d actions)\n", len(mod.Actions)))
	}

	sb.WriteString("\n## Regenerate\n\n")
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
				output = docsgen.BuildHTML(a)
			case "markdown", "md":
				output = docsgen.BuildMarkdown(a)
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
.method-DELETE{background:var(--delete-bg);color:var,--delete)}
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

// ── fmt ───────────────────────────────────────────────────────────────────────

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

func newDoctorCmd() *cobra.Command {
	return &cobra.Command{
		Use:    "doctor",
		Short:  "Diagnose project health and check for common issues",
		Long:   "Runs a series of checks on your Veld project to identify\ncommon misconfigurations, missing files, and environment issues.",
		Hidden: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			fmt.Println()
			fmt.Println(bold("  Veld Doctor"))
			fmt.Println()
			passed, failed := 0, 0

			check := func(name string, fn func() error) {
				if err := fn(); err != nil {
					fmt.Printf("  %s  %s — %s\n", red("✗"), name, err)
					failed++
				} else {
					fmt.Printf("  %s  %s\n", green("✓"), name)
					passed++
				}
			}

			// 1. Config file
			check("Config file found", func() error {
				_, _, err := config.FindConfig()
				if err != nil {
					return err
				}
				return nil
			})

			// 2. Config valid
			check("Config valid", func() error {
				_, err := config.BuildResolved(config.FlagOverrides{})
				return err
			})

			// 3. Input file parses
			check("Contract parses", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				_, _, err = loader.Parse(rc.Input, rc.Aliases)
				return err
			})

			// 4. Validation passes
			check("Contract validates", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				a, _, err := loader.Parse(rc.Input, rc.Aliases)
				if err != nil {
					return err
				}
				if errs := validator.Validate(a); len(errs) > 0 {
					return fmt.Errorf("%d validation error(s)", len(errs))
				}
				return nil
			})

			// 5. Backend emitter available
			check("Backend emitter registered", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				_, _, err = emitter.GetBackendOrTool(rc.Backend)
				return err
			})

			// 6. Frontend emitter available
			check("Frontend emitter registered", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				_, err = emitter.GetFrontend(rc.Frontend)
				return err
			})

			// 7. Output directory writable
			check("Output directory writable", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				for _, dir := range rc.OutputDirs() {
					if err := os.MkdirAll(dir, 0755); err != nil {
						return fmt.Errorf("cannot create %s: %w", dir, err)
					}
				}
				return nil
			})

			// 8. Lint check
			check("Lint clean (no errors)", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				a, _, err := loader.Parse(rc.Input, rc.Aliases)
				if err != nil {
					return err
				}
				issues := lint.Lint(a)
				errCount := 0
				for _, iss := range issues {
					if iss.IsError() {
						errCount++
					}
				}
				if errCount > 0 {
					return fmt.Errorf("%d lint error(s)", errCount)
				}
				return nil
			})

			fmt.Println()
			if failed > 0 {
				fmt.Printf("  %s %d passed, %s %d failed\n", green("✓"), passed, red("✗"), failed)
				return fmt.Errorf("%d check(s) failed", failed)
			}
			fmt.Printf("  %s All %d checks passed\n", green("✓"), passed)
			return nil
		},
	}
}

// ── completion ────────────────────────────────────────────────────────────────

func newCompletionCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "completion [bash|zsh|fish|powershell]",
		Short: "Generate shell completion scripts",
		Long: "Generate shell completion scripts for Veld.\n\n" +
			"  bash:       source <(veld completion bash)\n" +
			"  zsh:        veld completion zsh > \"${fpath[1]}/_veld\"\n" +
			"  fish:       veld completion fish | source\n" +
			"  powershell: veld completion powershell | Out-String | Invoke-Expression",
		Args:      cobra.ExactValidArgs(1),
		ValidArgs: []string{"bash", "zsh", "fish", "powershell"},
		RunE: func(cmd *cobra.Command, args []string) error {
			switch args[0] {
			case "bash":
				return cmd.Root().GenBashCompletion(os.Stdout)
			case "zsh":
				return cmd.Root().GenZshCompletion(os.Stdout)
			case "fish":
				return cmd.Root().GenFishCompletion(os.Stdout, true)
			case "powershell":
				return cmd.Root().GenPowerShellCompletionWithDesc(os.Stdout)
			default:
				return fmt.Errorf("unsupported shell: %s", args[0])
			}
		},
	}
	return cmd
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

	// ── Auto-detect project type ───────────────────────────────────────────
	detectedBackend := ""
	if _, err := os.Stat("package.json"); err == nil {
		detectedBackend = "node-ts"
	}
	if _, err := os.Stat("go.mod"); err == nil {
		detectedBackend = "go"
	}
	if _, err := os.Stat("requirements.txt"); err == nil {
		detectedBackend = "python"
	}
	if _, err := os.Stat("pyproject.toml"); err == nil {
		detectedBackend = "python"
	}
	if _, err := os.Stat("Cargo.toml"); err == nil {
		detectedBackend = "rust"
	}
	if _, err := os.Stat("pom.xml"); err == nil {
		detectedBackend = "java"
	}
	if _, err := os.Stat("build.gradle"); err == nil {
		detectedBackend = "java"
	}
	if _, err := os.Stat("composer.json"); err == nil {
		detectedBackend = "php"
	}
	csprojFiles, _ := filepath.Glob("*.csproj")
	if len(csprojFiles) > 0 {
		detectedBackend = "csharp"
	}
	if detectedBackend != "" {
		fmt.Printf("  %s Detected project type: %s\n\n", dim("ℹ"), bold(detectedBackend))
	}

	// ── Project type selection ─────────────────────────────────────────────
	fmt.Println("  " + bold("◆ Project type"))
	fmt.Printf("    %s1%s  Single service %s\n", colorGreen, colorReset, dim("(monolith — one backend, one frontend)"))
	fmt.Printf("    %s2%s  Microservices workspace %s\n", colorGreen, colorReset, dim("(multiple services with inter-service SDK)"))
	fmt.Print("\n  Choose [1]: ")
	projectTypeIdx := readChoice(reader, 2, 1)
	isMicroservices := projectTypeIdx == 2
	if isMicroservices {
		fmt.Printf("  → %s\n\n", green("microservices workspace"))
	} else {
		fmt.Printf("  → %s\n\n", green("single service"))
	}

	// ── Framework option tables ────────────────────────────────────────────
	type initFWOpt struct{ name, desc string }
	nodeFrameworkOpts := []initFWOpt{
		{"plain", "router: any — wire your own Express / Fastify / Hono / NestJS"},
		{"express", "Express 4.x"},
	}
	backendFrameworkOpts := map[string][]initFWOpt{
		"node-ts": nodeFrameworkOpts,
		"node-js": nodeFrameworkOpts,
		"python": {
			{"plain", "pure typed functions — no HTTP framework"},
			{"flask", "Flask blueprints + jsonify"},
			{"fastapi", "FastAPI router + Pydantic"},
		},
		"go": {
			{"plain", "net/http (Go 1.22+) — zero external dependencies"},
			{"chi", "Chi v5 router"},
			{"gin", "Gin framework"},
		},
		"java": {
			{"plain", "interfaces only — no HTTP framework"},
			{"spring", "Spring Boot 3.x / 4.x controllers + service interfaces"},
		},
		"rust": {
			{"plain", "trait definitions only — no HTTP framework"},
			{"axum", "Axum async handlers + Tokio"},
		},
		"csharp": {
			{"plain", "interfaces only — no HTTP framework"},
			{"aspnet", "ASP.NET Core controllers"},
		},
		"php": {
			{"plain", "interfaces only — no HTTP framework"},
			{"laravel", "Laravel routes + controllers"},
		},
	}
	frontendFrameworkOpts := map[string][]initFWOpt{
		"typescript": {
			{"", "pure fetch SDK — no framework wrapper"},
			{"react", "React Query hooks"},
			{"vue", "Vue 3 composables"},
			{"angular", "Angular services"},
			{"svelte", "Svelte stores"},
		},
		"javascript": {
			{"", "pure fetch SDK — no framework wrapper"},
			{"react", "React hooks (plain JavaScript)"},
		},
	}
	backendDisplayLabel := map[string]string{
		"node-ts": "node         Node.js",
		"python":  "python       Python 3",
		"go":      "go           Go",
		"rust":    "rust         Rust",
		"java":    "java         Java 17+",
		"csharp":  "csharp       C# / .NET 8",
		"php":     "php          PHP 8",
	}

	// ── Backend language selection ─────────────────────────────────────────
	// node-ts and node-js are merged under "node"; language is a sub-prompt.
	allBackends := emitter.ListBackends()
	var backends []string
	for _, b := range allBackends {
		if b != "node-js" {
			backends = append(backends, b)
		}
	}
	fmt.Println("  " + bold("Backend language") + " — which server language?")
	defaultBackendIdx := 1
	for i, b := range backends {
		if b == "node-ts" {
			defaultBackendIdx = i + 1
		}
		if detectedBackend != "" && b == detectedBackend {
			defaultBackendIdx = i + 1
		}
	}
	for i, b := range backends {
		label := backendDisplayLabel[b]
		if label == "" {
			label = b
		}
		if detectedBackend != "" && b == detectedBackend {
			label += dim(" ← detected")
		}
		if i+1 == defaultBackendIdx {
			label += dim(" (default)")
		}
		fmt.Printf("    %s%2d%s  %s\n", colorGreen, i+1, colorReset, label)
	}
	fmt.Printf("\n  Choose [%d]: ", defaultBackendIdx)
	backendChoice := readChoice(reader, len(backends), defaultBackendIdx)
	selectedBackend := backends[backendChoice-1]
	fmt.Printf("  → %s\n\n", green(selectedBackend))

	// ── Node language sub-prompt (TypeScript / JavaScript) ────────────────
	if selectedBackend == "node-ts" {
		fmt.Println("  " + bold("Node language") + " — TypeScript or JavaScript?")
		fmt.Printf("    %s 1%s  TypeScript  %s\n", colorGreen, colorReset, dim("(default)"))
		fmt.Printf("    %s 2%s  JavaScript  %s\n", colorGreen, colorReset, dim("plain JS, JSDoc types"))
		fmt.Print("\n  Choose [1]: ")
		langIdx := readChoice(reader, 2, 1)
		if langIdx == 2 {
			selectedBackend = "node-js"
			fmt.Printf("  → %s\n\n", green("JavaScript"))
		} else {
			fmt.Printf("  → %s\n\n", green("TypeScript"))
		}
	}

	// ── Backend framework selection ────────────────────────────────────────
	selectedBackendFramework := ""
	if fwOpts := backendFrameworkOpts[selectedBackend]; len(fwOpts) > 0 {
		fmt.Println("  " + bold("Backend framework") + " — which HTTP framework? (plain = no framework)")
		for i, fw := range fwOpts {
			lbl := fw.name
			if lbl == "plain" || lbl == "" {
				lbl = "plain"
			}
			fmt.Printf("    %s%2d%s  %-12s %s\n", colorGreen, i+1, colorReset, lbl, dim(fw.desc))
		}
		fmt.Print("\n  Choose [1]: ")
		fwIdx := readChoice(reader, len(fwOpts), 1)
		selectedBackendFramework = fwOpts[fwIdx-1].name
		if selectedBackendFramework == "plain" {
			selectedBackendFramework = ""
		}
		display := fwOpts[fwIdx-1].name
		if display == "" || display == "plain" {
			display = "plain (no framework)"
		}
		fmt.Printf("  → %s\n\n", green(display))
	}

	// ── Frontend language selection ────────────────────────────────────────
	// Show language choices only; framework-specific emitters are sub-options below.
	fwSpecificFrontends := map[string]bool{"react": true, "vue": true, "angular": true, "svelte": true}
	var frontends []string
	for _, f := range append(emitter.ListFrontends(), "none") {
		if !fwSpecificFrontends[f] {
			frontends = append(frontends, f)
		}
	}
	fmt.Println("  " + bold("Frontend language") + " — which client language / SDK?")
	defaultFrontend := 1
	for i, f := range frontends {
		if f == "typescript" {
			defaultFrontend = i + 1
			break
		}
	}
	for i, f := range frontends {
		label := f
		if i+1 == defaultFrontend {
			label += dim(" (default)")
		}
		fmt.Printf("    %s%2d%s  %s\n", colorGreen, i+1, colorReset, label)
	}
	fmt.Printf("\n  Choose [%d]: ", defaultFrontend)
	frontendChoice := readChoice(reader, len(frontends), defaultFrontend)
	selectedFrontend := frontends[frontendChoice-1]
	fmt.Printf("  → %s\n\n", green(selectedFrontend))

	// ── Frontend framework selection ───────────────────────────────────────
	selectedFrontendFramework := ""
	if fwOpts := frontendFrameworkOpts[selectedFrontend]; len(fwOpts) > 0 {
		fmt.Println("  " + bold("Frontend framework") + " — wrap the SDK with a UI framework?")
		for i, fw := range fwOpts {
			lbl := fw.name
			if lbl == "" {
				lbl = "none"
			}
			fmt.Printf("    %s%2d%s  %-12s %s\n", colorGreen, i+1, colorReset, lbl, dim(fw.desc))
		}
		fmt.Print("\n  Choose [1]: ")
		fwIdx := readChoice(reader, len(fwOpts), 1)
		selectedFrontendFramework = fwOpts[fwIdx-1].name
		display := fwOpts[fwIdx-1].name
		if display == "" {
			display = "none (pure SDK)"
		}
		fmt.Printf("  → %s\n\n", green(display))
	}

	// ── Runtime validation ─────────────────────────────────────────────────
	// Only relevant for node and python — statically-typed backends (go, rust,
	// java, csharp) already enforce contract correctness at compile time.
	enableValidate := false
	if selectedBackend == "node-ts" || selectedBackend == "node-js" || selectedBackend == "python" {
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

	// ── Description prompt ──────────────────────────────────────────────────
	fmt.Print("  " + bold("◆ Description") + dim(" (optional)") + ": ")
	descLine, _ := reader.ReadString('\n')
	description := strings.TrimSpace(descLine)
	if description != "" {
		fmt.Printf("  → %s\n\n", dim(description))
	} else {
		fmt.Println()
	}

	type entry struct{ path, content, label string }
	var files []entry

	if isMicroservices {
		// ── Microservices workspace flow ──────────────────────────────────
		fmt.Print("  " + bold("◆ How many backend services?") + " [2]: ")
		numServices := readChoice(reader, 20, 2)
		fmt.Printf("  → %s\n\n", green(fmt.Sprintf("%d services", numServices)))

		// Ask whether all services share the same backend/framework.
		fmt.Print("  " + bold("◆ Are all services the same backend?") + " [Y/n]: ")
		sameLine, _ := reader.ReadString('\n')
		sameLine = strings.TrimSpace(strings.ToLower(sameLine))
		allSameBackend := sameLine == "" || sameLine == "y" || sameLine == "yes"
		if allSameBackend {
			fmt.Printf("  → %s\n\n", green(fmt.Sprintf("yes — all use %s", selectedBackend)))
		} else {
			fmt.Printf("  → %s\n\n", dim("per-service selection"))
		}

		type svcDef struct {
			name, backend, framework, baseUrl string
			consumes                          []string
		}
		var services []svcDef
		allSvcNames := []string{}

		for s := 0; s < numServices; s++ {
			fmt.Printf("  " + bold(fmt.Sprintf("◆ Service %d", s+1)) + "\n")

			fmt.Printf("    Name: ")
			nameLine, _ := reader.ReadString('\n')
			svcName := strings.TrimSpace(nameLine)
			if svcName == "" {
				svcName = fmt.Sprintf("service%d", s+1)
			}
			allSvcNames = append(allSvcNames, svcName)

			svcBackend := selectedBackend
			svcFramework := selectedBackendFramework

			if !allSameBackend {
				// Show the same backend menu as the top-level prompt.
				fmt.Printf("    Backend — choose language:\n")
				for i, b := range backends {
					fmt.Printf("      %s%2d%s  %s\n", colorGreen, i+1, colorReset, b)
				}
				defaultIdx := 1
				for i, b := range backends {
					if b == selectedBackend {
						defaultIdx = i + 1
						break
					}
				}
				fmt.Printf("    Choose [%d]: ", defaultIdx)
				bIdx := readChoice(reader, len(backends), defaultIdx)
				svcBackend = backends[bIdx-1]

				// Node TS/JS sub-choice
				if svcBackend == "node-ts" {
					fmt.Printf("      TypeScript [1] or JavaScript [2]? ")
					if readChoice(reader, 2, 1) == 2 {
						svcBackend = "node-js"
					}
				}

				// Framework for this backend
				svcFramework = ""
				if fwOpts := backendFrameworkOpts[svcBackend]; len(fwOpts) > 0 {
					fmt.Printf("    Framework:\n")
					for i, fw := range fwOpts {
						lbl := fw.name
						if lbl == "" || lbl == "plain" {
							lbl = "plain (no framework)"
						}
						fmt.Printf("      %s%2d%s  %-14s %s\n", colorGreen, i+1, colorReset, lbl, dim(fw.desc))
					}
					fmt.Printf("    Choose [1]: ")
					fwIdx := readChoice(reader, len(fwOpts), 1)
					svcFramework = fwOpts[fwIdx-1].name
					if svcFramework == "plain" {
						svcFramework = ""
					}
				}
				fmt.Printf("    → %s\n", green(svcBackend))
			}

			defaultPort := 3001 + s
			fmt.Printf("    Base URL [http://%s-service:%d]: ", svcName, defaultPort)
			urlLine, _ := reader.ReadString('\n')
			svcUrl := strings.TrimSpace(urlLine)
			if svcUrl == "" {
				svcUrl = fmt.Sprintf("http://%s-service:%d", svcName, defaultPort)
			}

			var svcConsumes []string
			if len(allSvcNames) > 1 {
				fmt.Printf("    Consumes %s: ", dim("(comma-separated, empty for none)"))
				consumesLine, _ := reader.ReadString('\n')
				consumesStr := strings.TrimSpace(consumesLine)
				if consumesStr != "" {
					for _, c := range strings.Split(consumesStr, ",") {
						c = strings.TrimSpace(c)
						if c != "" {
							svcConsumes = append(svcConsumes, c)
						}
					}
				}
			}

			services = append(services, svcDef{
				name: svcName, backend: svcBackend, framework: svcFramework, baseUrl: svcUrl, consumes: svcConsumes,
			})
			fmt.Println()
		}

		// ── Frontend entry prompt ────────────────────────────────────────
		fmt.Print("  " + bold("◆ Include frontend entry?") + " [Y/n]: ")
		feLine, _ := reader.ReadString('\n')
		feLine = strings.TrimSpace(strings.ToLower(feLine))
		includeFrontend := feLine == "" || feLine == "y" || feLine == "yes"
		if includeFrontend {
			fmt.Printf("  → %s\n\n", green("yes — "+selectedFrontend))
		} else {
			fmt.Printf("  → %s\n\n", dim("no frontend"))
		}

		// ── Build workspace config JSON ──────────────────────────────────
		var wsEntries []string
		for _, svc := range services {
			consumesJSON := "[]"
			if len(svc.consumes) > 0 {
				parts := make([]string, len(svc.consumes))
				for i, c := range svc.consumes {
					parts[i] = fmt.Sprintf("%q", c)
				}
				consumesJSON = "[" + strings.Join(parts, ", ") + "]"
			}
			consumesLine := ""
			if len(svc.consumes) > 0 {
				consumesLine = fmt.Sprintf(",\n      \"consumes\": %s", consumesJSON)
			}
			// dir is always the parent of out so veld setup can find pom.xml / package.json / etc.
			svcDir := fmt.Sprintf("../backend/%s-service", svc.name)
			backendCfgJSON := fmt.Sprintf(`{ "target": %q, "dir": %q }`, svc.backend, svcDir)
			if svc.framework != "" {
				backendCfgJSON = fmt.Sprintf(`{ "target": %q, "framework": %q, "dir": %q }`, svc.backend, svc.framework, svcDir)
			}
			wsEntries = append(wsEntries, fmt.Sprintf(`    {
      "name": %q,
      "description": "",
      "input": "services/%s/modules/%s.veld",
      "backendConfig": %s,
      "out": "../backend/%s-service/generated",
      "baseUrl": %q%s
    }`, svc.name, svc.name, svc.name, backendCfgJSON, svc.name, svc.baseUrl, consumesLine))
		}

		if includeFrontend {
			// Frontend auto-consumes all backend services
			parts := make([]string, len(allSvcNames))
			for i, n := range allSvcNames {
				parts[i] = fmt.Sprintf("%q", n)
			}
			consumesAll := "[" + strings.Join(parts, ", ") + "]"
			feTarget := selectedFrontend
			if selectedFrontendFramework != "" {
				feTarget = selectedFrontendFramework
			}
			wsEntries = append(wsEntries, fmt.Sprintf(`    {
      "name": "frontend",
      "description": "Frontend SDK — auto-consumes all backend services",
      "frontendConfig": { "target": %q },
      "out": "../frontend/src/generated",
      "baseUrl": "http://localhost:3000",
      "consumes": %s
    }`, feTarget, consumesAll))
		}

		descLine := ""
		if description != "" {
			descLine = fmt.Sprintf("\n  \"description\": %q,", description)
		}
		configJSON := fmt.Sprintf(`{
  "$schema": "https://veld.dev/schemas/veld.config.schema.json",%s

  "workspace": [
%s
  ]
}
`, descLine, strings.Join(wsEntries, ",\n"))

		files = append(files, entry{"veld/veld.config.json", configJSON, "veld/veld.config.json"})

		// ── Create service .veld stubs ────────────────────────────────────
		for _, svc := range services {
			moduleName := strings.ToUpper(svc.name[:1]) + svc.name[1:]
			modelContent := fmt.Sprintf(`// %s domain models

model %sItem {
  description: "A %s entity"
  id:        uuid
  name:      string
  createdAt: datetime
}

model Create%sInput {
  name: string
}
`, moduleName, moduleName, svc.name, moduleName)

			moduleContent := fmt.Sprintf(`// %s service module

import @models/*

module %s {
  description: "%s service"
  prefix: /%s

  action List%s {
    method: GET
    path: /
    output: %sItem
  }

  action Get%s {
    method: GET
    path: /:id
    output: %sItem
  }

  action Create%s {
    method: POST
    path: /
    input: Create%sInput
    output: %sItem
  }
}
`, moduleName, moduleName, moduleName, svc.name, moduleName, moduleName, moduleName, moduleName, moduleName, moduleName, moduleName)

			modelsDir := fmt.Sprintf("veld/services/%s/models", svc.name)
			modulesDir := fmt.Sprintf("veld/services/%s/modules", svc.name)
			files = append(files,
				entry{modelsDir + "/" + svc.name + ".model.veld", modelContent, modelsDir + "/" + svc.name + ".model.veld"},
				entry{modulesDir + "/" + svc.name + ".veld", moduleContent, modulesDir + "/" + svc.name + ".veld"},
			)
		}
		files = append(files, entry{"README.md", initReadmeContent, "README.md"})

	} else {
		// ── Single service flow ──────────────────────────────────────────

		// Build backend config block
		backendFrameworkLine := ""
		if selectedBackendFramework != "" {
			backendFrameworkLine = fmt.Sprintf(",\n    \"framework\": %q", selectedBackendFramework)
		}
		validateLine := ""
		if enableValidate {
			validateLine = ",\n    \"validate\": true"
		}

		// Build frontend config block
		feTarget := selectedFrontend
		if selectedFrontendFramework != "" {
			feTarget = selectedFrontendFramework
		}
		frontendBlock := fmt.Sprintf(`"frontendConfig": {
    "target": %q
  }`, feTarget)
		if selectedFrontend == "none" {
			frontendBlock = `"frontendConfig": { "target": "none" }`
		}

		descLine := ""
		if description != "" {
			descLine = fmt.Sprintf("\n  \"description\": %q,", description)
		}

		configJSON := fmt.Sprintf(`{
  "$schema": "https://veld.dev/schemas/veld.config.schema.json",%s
  "input": "app.veld",

  "backendConfig": {
    "target": %q%s,
    "out": %q%s
  },

  %s,

  "baseUrl": "",
  "aliases": {}
}
`, descLine, selectedBackend, backendFrameworkLine, defaultOut, validateLine, frontendBlock)

		files = []entry{
			{"veld/veld.config.json", configJSON, "veld/veld.config.json"},
			{"veld/app.veld", appVeldContent, "veld/app.veld"},
			{"veld/models/user.model.veld", modelsUserVeldContent, "veld/models/user.model.veld"},
			{"veld/models/auth.model.veld", modelsAuthModelContent, "veld/models/auth.model.veld"},
			{"veld/models/common.model.veld", modelsCommonVeldContent, "veld/models/common.model.veld"},
			{"veld/modules/users.veld", modulesUsersVeldContent, "veld/modules/users.veld"},
			{"veld/modules/auth.veld", modulesAuthVeldContent, "veld/modules/auth.veld"},
			{"README.md", initReadmeContent, "README.md"},
		}
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
	if isMicroservices {
		fmt.Printf("    mode:     %s\n", bold("microservices workspace"))
	} else {
		fmt.Printf("    backend:  %s\n", bold(selectedBackend))
		fmt.Printf("    frontend: %s\n", bold(selectedFrontend))
	}
	if description != "" {
		fmt.Printf("    desc:     %s\n", dim(description))
	}
	fmt.Println()

	// ── Setup prompt (single-service only) ────────────────────────────────
	// Microservices workspaces configure paths per-service in the config.
	if !isMicroservices {
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
					backendOutLine = fmt.Sprintf(",\n    \"out\": %q", relBackendOut)
				}
				if relFrontendOut != "" {
					frontendOutLine = fmt.Sprintf(",\n    \"out\": %q", relFrontendOut)
				}

				backendFWLine := ""
				if selectedBackendFramework != "" {
					backendFWLine = fmt.Sprintf(",\n    \"framework\": %q", selectedBackendFramework)
				}

				feTarget := selectedFrontend
				if selectedFrontendFramework != "" {
					feTarget = selectedFrontendFramework
				}

				descLine := ""
				if description != "" {
					descLine = fmt.Sprintf("\n  \"description\": %q,", description)
				}

				updatedCfg := fmt.Sprintf(`{
  "$schema": "https://veld.dev/schemas/veld.config.schema.json",%s
  "input": "app.veld",

  "backendConfig": {
    "target": %q%s,
    "dir": %q%s
  },

  "frontendConfig": {
    "target": %q,
    "dir": %q%s
  },

  "baseUrl": "",
  "aliases": {}
}
`, descLine, selectedBackend, backendFWLine, relBackend, backendOutLine, feTarget, relFrontend, frontendOutLine)
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
	} // end if !isMicroservices

	fmt.Println()
	fmt.Println("  Next steps:")
	if isMicroservices {
		fmt.Println("    1. Edit veld/services/<name>/models/ and modules/ to define each service's API")
		fmt.Println("    2. Run: " + bold("veld setup") + "  — patch tsconfig / package.json / pom.xml in each service")
		fmt.Println("    3. Run: " + bold("veld generate"))
		fmt.Println("    4. Run: " + bold("veld deps") + " to view the service dependency graph")
	} else {
		fmt.Println("    1. Edit veld/models/ and veld/modules/ to define your API")
		fmt.Println("    2. Run: " + bold("veld generate"))
	}
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

import @models/user.model

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

import @models/user.model
import @models/common.model

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

import @models/user.model
import @models/auth.model
import @models/common.model

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
	"import @models/user.model\n" +
	"import @models/common.model\n\n" +
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
	"| `veld lint` | Analyse contract quality |\n" +
	"| `veld fmt` | Format .veld files |\n" +
	"| `veld watch` | Auto-regenerate on file save |\n" +
	"| `veld clean` | Remove generated output |\n" +
	"| `veld openapi` | Export OpenAPI 3.0 spec |\n" +
	"| `veld diff` | Show changes since last gen |\n" +
	"| `veld setup` | Auto-configure project imports |\n" +
	"| `veld doctor` | Diagnose project health |\n" +
	"| `veld ast` | Dump AST JSON for debugging |\n"

// ── login ─────────────────────────────────────────────────────────────────────

func newLoginCmd() *cobra.Command {
	var registryURL, token string
	cmd := &cobra.Command{
		Use:   "login",
		Short: "Authenticate with a Veld Registry using an API token",
		Long: `Authenticate the CLI with a Veld Registry using an API token.

To create a token:
  1. Open the registry web UI in your browser
  2. Go to Settings → API Tokens → New Token
  3. Copy the generated token (it is only shown once)
  4. Run: veld registry login --registry <url> --token vtk_...`,
		Example: "  veld registry login --registry https://registry.veld.dev --token vtk_...\n" +
			"  veld registry login --registry http://localhost:8080 --token vtk_...",
		RunE: func(cmd *cobra.Command, args []string) error {
			if registryURL == "" {
				return fmt.Errorf("--registry is required (e.g. --registry https://registry.veld.dev)")
			}
			if token == "" {
				registryBase := strings.TrimRight(registryURL, "/")
				fmt.Printf("To log in, create an API token in the web UI:\n")
				fmt.Printf("  %s/#/tokens\n\n", registryBase)
				fmt.Printf("Then run:\n")
				fmt.Printf("  veld registry login --registry %s --token vtk_...\n", registryURL)
				return nil
			}
			client := registry.NewClient(registryURL, token)
			me, err := client.Me()
			if err != nil {
				return fmt.Errorf("token validation failed: %w", err)
			}
			username, _ := me["username"].(string)
			if err := registry.SetToken(registryURL, token, username); err != nil {
				return err
			}
			fmt.Printf(green("✓")+" Logged in to %s as %s\n", registryURL, username)
			return nil
		},
	}
	cmd.Flags().StringVar(&registryURL, "registry", "", "registry URL")
	cmd.Flags().StringVar(&token, "token", "", "API token (vtk_...)")
	return cmd
}

// ── logout ────────────────────────────────────────────────────────────────────

func newLogoutCmd() *cobra.Command {
	var registryURL string
	cmd := &cobra.Command{
		Use:   "logout",
		Short: "Remove stored credentials for a registry",
		RunE: func(cmd *cobra.Command, args []string) error {
			if registryURL == "" {
				registryURL = registry.DefaultRegistry()
			}
			if registryURL == "" {
				return fmt.Errorf("no registry configured")
			}
			if err := registry.ClearToken(registryURL); err != nil {
				return err
			}
			fmt.Printf(green("✓")+" Logged out from %s\n", registryURL)
			return nil
		},
	}
	cmd.Flags().StringVar(&registryURL, "registry", "", "registry URL (defaults to current)")
	return cmd
}

// ── push ──────────────────────────────────────────────────────────────────────

func newPushCmd() *cobra.Command {
	var registryURL, orgName, pkgName, version string
	cmd := &cobra.Command{
		Use:   "push",
		Short: "Publish .veld contracts to the registry",
		Example: "  veld registry push\n" +
			"  veld registry push --registry https://registry.veld.dev\n" +
			"  veld registry push --org acme --name auth --version 1.2.0",
		RunE: func(cmd *cobra.Command, args []string) error {
			// Resolve config for org/name/version defaults
			rc, err := config.BuildResolved(config.FlagOverrides{})
			if err != nil {
				return fmt.Errorf("could not load veld.config.json: %w", err)
			}

			// Defaults from config
			if orgName == "" {
				orgName = rc.Registry.Org
			}
			if pkgName == "" {
				pkgName = rc.Registry.Package
			}
			if version == "" {
				version = rc.Registry.Version
			}
			if registryURL == "" {
				registryURL = rc.Registry.URL
			}
			if orgName == "" || pkgName == "" || version == "" {
				return fmt.Errorf("--org, --name, and --version are required (or set registry.org/package/version in veld.config.json)")
			}

			client, err := registry.NewClientFromCreds(registryURL)
			if err != nil {
				return err
			}

			fmt.Printf(dim("⬡")+"  Packing contracts from %s…\n", rc.ConfigDir)
			tarPath, sha, err := registry.Pack(rc.ConfigDir)
			if err != nil {
				return fmt.Errorf("pack failed: %w", err)
			}
			defer func() {
				if err := os.Remove(tarPath); err == nil && verbose {
					fmt.Println(dim("  removed temp tarball"))
				}
			}()

			f, err := os.Open(tarPath)
			if err != nil {
				return err
			}
			defer f.Close()

			fi, _ := f.Stat()
			fmt.Printf(dim("⬡")+"  Publishing @%s/%s@%s (%s)…\n",
				orgName, pkgName, version, fmtBytes(fi.Size()))

			manifestJSON := fmt.Sprintf(`{"org":%q,"name":%q,"version":%q}`, orgName, pkgName, version)
			result, err := client.Publish(manifestJSON, pkgName+"-"+version+".tar.gz", f)
			if err != nil {
				return err
			}
			_ = sha
			fmt.Printf(green("✓")+" Published @%s/%s@%s\n%s\n",
				orgName, pkgName, version, dim(string(result)))
			return nil
		},
	}
	cmd.Flags().StringVar(&registryURL, "registry", "", "registry URL (default: from credentials)")
	cmd.Flags().StringVar(&orgName, "org", "", "organisation name")
	cmd.Flags().StringVar(&pkgName, "name", "", "package name")
	cmd.Flags().StringVar(&version, "version", "", "semver version to publish")
	return cmd
}

// ── pull ──────────────────────────────────────────────────────────────────────

func newPullCmd() *cobra.Command {
	var registryURL, outDir string
	cmd := &cobra.Command{
		Use:   "pull <@org/name[@version]>",
		Short: "Download a contract package from the registry",
		Example: "  veld registry pull @acme/auth\n" +
			"  veld registry pull @acme/auth@1.2.0\n" +
			"  veld registry pull @acme/auth --out veld/packages",
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			orgName, pkgName, version, err := parsePackageRef(args[0])
			if err != nil {
				return err
			}

			client, err := registry.NewClientFromCreds(registryURL)
			if err != nil {
				// Allow unauthenticated for public packages
				if registryURL == "" {
					registryURL = registry.DefaultRegistry()
				}
				if registryURL == "" {
					return err
				}
				client = registry.NewClient(registryURL, "")
			}

			// Resolve "latest" version
			if version == "" || version == "latest" {
				versions, err := client.ListPackageVersions(orgName, pkgName)
				if err != nil {
					return fmt.Errorf("could not fetch versions: %w", err)
				}
				if len(versions) == 0 {
					return fmt.Errorf("no versions published for @%s/%s", orgName, pkgName)
				}
				version, _ = versions[0]["version"].(string)
			}

			if outDir == "" {
				outDir = filepath.Join("veld", "packages", "@"+orgName, pkgName)
			}
			if err := os.MkdirAll(outDir, 0755); err != nil {
				return err
			}

			fmt.Printf(dim("⬡")+"  Pulling @%s/%s@%s → %s\n", orgName, pkgName, version, outDir)

			// Stream tarball to temp file, verify SHA, extract
			tmp, err := os.CreateTemp("", "veld-pull-*.tar.gz")
			if err != nil {
				return err
			}
			defer os.Remove(tmp.Name())

			remoteSHA, err := client.Download(orgName, pkgName, version, tmp)
			tmp.Close()
			if err != nil {
				return fmt.Errorf("download failed: %w", err)
			}
			if remoteSHA != "" {
				if err := registry.VerifySHA(tmp.Name(), remoteSHA); err != nil {
					return fmt.Errorf("integrity check failed: %w", err)
				}
			}

			if err := registry.Unpack(tmp.Name(), outDir); err != nil {
				return fmt.Errorf("extract failed: %w", err)
			}

			fmt.Printf(green("✓")+" Pulled @%s/%s@%s\n", orgName, pkgName, version)
			fmt.Printf(dim("   Import with: import @%s/%s/ModelName\n"), orgName, pkgName)
			return nil
		},
	}
	cmd.Flags().StringVar(&registryURL, "registry", "", "registry URL")
	cmd.Flags().StringVar(&outDir, "out", "", "output directory (default: veld/packages/@org/name)")
	return cmd
}

// ── export (subcommand group) ─────────────────────────────────────────────────

func newExportCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "export",
		Short: "Export contract to other formats (OpenAPI, GraphQL, SQL, docs)",
	}
	cmd.AddCommand(newOpenAPICmd(), newGraphQLCmd(), newSchemaCmd(), newDocsCmd(), newAgentsCmd())
	return cmd
}

func newAgentsCmd() *cobra.Command {
	var outputFlag string
	cmd := &cobra.Command{
		Use:     "agents",
		Short:   "Generate AGENTS.md for AI discoverability",
		Long:    "Generates a compact Markdown file describing the full API contract,\noptimised for AI assistants (Claude, Copilot, Cursor) to ingest in one read.",
		Example: "  veld export agents\n  veld export agents -o AGENTS.md",
		RunE: func(cmd *cobra.Command, args []string) error {
			rc, err := config.BuildResolved(config.FlagOverrides{})
			if err != nil {
				return err
			}
			a, _, err := loader.Parse(rc.Input, rc.Aliases)
			if err != nil {
				return err
			}
			content := docsgen.BuildAgentsMd(a, rc)
			if outputFlag == "" {
				fmt.Print(content)
				return nil
			}
			if err := os.WriteFile(outputFlag, []byte(content), 0644); err != nil {
				return fmt.Errorf("writing %s: %w", outputFlag, err)
			}
			fmt.Printf("  %s %s\n", green("✓"), outputFlag)
			return nil
		},
	}
	cmd.Flags().StringVarP(&outputFlag, "output", "o", "", "write to file instead of stdout")
	return cmd
}

// ── registry (subcommand group) ───────────────────────────────────────────────

func newRegistryCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "registry",
		Short: "Manage registry connections and packages",
	}

	// veld registry info
	cmd.AddCommand(&cobra.Command{
		Use:   "info",
		Short: "Show current registry and logged-in user",
		RunE: func(cmd *cobra.Command, args []string) error {
			url := registry.DefaultRegistry()
			if url == "" {
				fmt.Println("No registry configured. Run: veld registry login --registry <url>")
				return nil
			}
			token := registry.GetToken(url)
			client := registry.NewClient(url, token)
			me, err := client.Me()
			if err != nil {
				fmt.Printf("Registry: %s\nStatus:   %s\n", url, red("not authenticated"))
				return nil
			}
			fmt.Printf("Registry: %s\nUser:     %s (%s)\n", url, me["username"], me["email"])
			return nil
		},
	})

	// veld registry list
	cmd.AddCommand(&cobra.Command{
		Use:   "list",
		Short: "List all configured registries",
		Run: func(cmd *cobra.Command, args []string) {
			registry.ListRegistries()
		},
	})

	// veld registry versions <@org/name>
	versionsCmd := &cobra.Command{
		Use:   "versions <@org/name>",
		Short: "List all published versions of a package",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			orgName, pkgName, _, err := parsePackageRef(args[0])
			if err != nil {
				return err
			}
			client, err := registry.NewClientFromCreds("")
			if err != nil {
				return err
			}
			versions, err := client.ListPackageVersions(orgName, pkgName)
			if err != nil {
				return err
			}
			if len(versions) == 0 {
				fmt.Printf("No versions published for @%s/%s\n", orgName, pkgName)
				return nil
			}
			fmt.Printf("@%s/%s — %d version(s):\n", orgName, pkgName, len(versions))
			for _, v := range versions {
				ver, _ := v["version"].(string)
				dep, _ := v["deprecated"].(string)
				line := "  " + bold("v"+ver)
				if dep != "" {
					line += " " + yellow("[deprecated: "+dep+"]")
				}
				fmt.Println(line)
			}
			return nil
		},
	}
	cmd.AddCommand(versionsCmd)

	// veld registry login / logout / push / pull / serve
	cmd.AddCommand(newLoginCmd(), newLogoutCmd(), newPushCmd(), newPullCmd(), newServeCmd())

	// veld registry init
	cmd.AddCommand(newRegistryInitCmd())

	// veld registry token create
	tokenCmd := &cobra.Command{
		Use:   "token",
		Short: "Manage API tokens",
	}
	var tokenName string
	var tokenScopes []string
	tokenCreateCmd := &cobra.Command{
		Use:   "create",
		Short: "Create a new API token",
		RunE: func(cmd *cobra.Command, args []string) error {
			if tokenName == "" {
				return fmt.Errorf("--name is required")
			}
			client, err := registry.NewClientFromCreds("")
			if err != nil {
				return err
			}
			body := map[string]interface{}{
				"name":   tokenName,
				"scopes": tokenScopes,
			}
			data, err := client.PostJSON("/tokens", body)
			if err != nil {
				return err
			}
			fmt.Printf(green("✓")+" Token created. Copy it now — it will not be shown again:\n\n  %s\n\n", data)
			return nil
		},
	}
	tokenCreateCmd.Flags().StringVar(&tokenName, "name", "", "token name")
	tokenCreateCmd.Flags().StringSliceVar(&tokenScopes, "scopes", []string{"read"}, "comma-separated scopes: read,write,delete")
	tokenCmd.AddCommand(tokenCreateCmd)
	cmd.AddCommand(tokenCmd)

	return cmd
}

// ── ci ────────────────────────────────────────────────────────────────────────

// newCICmd returns the `veld ci` command — a single non-interactive command
// that runs generate + setup and exits with the correct status code.
//
// Replace this in any Dockerfile, pipeline, or script:
//
//	npx veld generate && npx veld setup
//
// With:
//
//	npx veld ci
func newCICmd() *cobra.Command {
	var backendFlag, frontendFlag, inputFlag, outFlag string
	var strictFlag bool

	cmd := &cobra.Command{
		Use:   "ci",
		Short: "Generate code and configure project paths in one step (non-interactive)",
		Long: `Run veld generate then veld setup in a single non-interactive command.

Reads backend and frontend from veld.config.json automatically.
Never prompts — safe to run in any pipeline, Dockerfile, or script.
Exits 0 on success, 1 on any failure.

Replace this:
  npx veld generate
  npx veld setup

With:
  npx veld ci`,
		Example: "  npx veld ci\n  veld ci\n  veld ci --strict",
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
				return fmt.Errorf("could not load veld.config.json: %w", err)
			}

			// ── step 1: generate ─────────────────────────────────────────────
			fmt.Printf(dim("⬡")+"  Generating (backend=%s frontend=%s)…\n", rc.Backend, rc.Frontend)

			// Check for breaking changes before emitting.
			if preChanges := computePreChanges(rc); diff.HasBreaking(preChanges) {
				printDiffChanges(preChanges)
				if strictFlag {
					fmt.Fprintln(os.Stderr, red("✗")+" Breaking changes detected — aborting (--strict)")
					return fmt.Errorf("breaking changes blocked by --strict")
				}
				// Non-strict: warn but continue — no interactive prompt in CI.
				fmt.Fprintln(os.Stderr, yellow("⚠")+"  Breaking changes detected — continuing (pass --strict to block)")
			}

			opts := emitter.EmitOptions{
				BaseUrl: rc.BaseUrl,
			}
			generatedFiles, _, _, err := runGenerate(rc, false, opts)
			if err != nil {
				return fmt.Errorf("generate failed: %w", err)
			}

			if len(generatedFiles) > 0 {
				fmt.Printf(green("✓")+" Generated %d file(s) → %s\n", len(generatedFiles), rc.Out)
			} else {
				fmt.Printf(green("✓")+" Generated → %s\n", rc.Out)
			}

			runPostGenerate(rc)

			// ── step 2: setup ────────────────────────────────────────────────
			fmt.Printf(dim("⬡") + "  Configuring project paths…\n")

			projectDir, _ := os.Getwd()
			results := setup.Run(projectDir, rc.Backend, rc.Frontend, rc.Out, setup.Options{
				BackendDir:     rc.BackendDir,
				FrontendDir:    rc.FrontendDir,
				BackendOutDir:  rc.BackendOut,
				FrontendOutDir: rc.FrontendOut,
			})

			patched, alreadyOK := 0, 0
			for _, r := range results {
				switch r.Action {
				case "patched":
					patched++
					fmt.Printf("  %s %s — %s\n", green("✓"), r.File, r.Detail)
				case "skipped":
					alreadyOK++
					fmt.Printf("  %s %s — %s\n", dim("·"), r.File, dim(r.Detail))
				case "not-found":
					fmt.Printf("  %s %s — %s\n", yellow("!"), r.File, r.Detail)
				case "manual":
					fmt.Printf("  %s %s — %s\n", dim("→"), r.File, r.Detail)
				}
			}

			switch {
			case patched > 0:
				fmt.Printf(green("✓")+" Setup patched %d file(s)\n", patched)
			case alreadyOK > 0:
				fmt.Printf(green("✓") + " Setup already configured\n")
			default:
				fmt.Printf(dim("·") + "  No config files to patch for this stack\n")
			}

			return nil
		},
	}

	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend override")
	cmd.Flags().StringVar(&frontendFlag, "frontend", "", "frontend override")
	cmd.Flags().StringVar(&inputFlag, "input", "", "input .veld file override")
	cmd.Flags().StringVar(&outFlag, "out", "", "output directory override")
	cmd.Flags().BoolVar(&strictFlag, "strict", false, "exit 1 on breaking changes")
	return cmd
}

// ── registry init ─────────────────────────────────────────────────────────────

func newRegistryInitCmd() *cobra.Command {
	var flagAddr, flagDSN, flagSecret, flagStorage, flagBaseURL, flagOut string

	cmd := &cobra.Command{
		Use:   "init",
		Short: "Scaffold a registry.config.json for self-hosting",
		Long: `Create a registry.config.json file with your server configuration.

Missing required values (DSN, secret) will be prompted interactively.
Pass --yes to skip prompts and write defaults (useful in scripts).`,
		Example: `  veld registry init
  veld registry init --dsn "postgres://localhost/veld?sslmode=disable" --secret $(openssl rand -hex 32)
  veld registry init --out /etc/veld/registry.config.json`,
		RunE: func(cmd *cobra.Command, args []string) error {
			outPath := flagOut
			if outPath == "" {
				outPath = "registry.config.json"
			}

			// Don't overwrite
			if _, err := os.Stat(outPath); err == nil {
				return fmt.Errorf("%s already exists — delete it first or use --out to choose a different path", outPath)
			}

			reader := bufio.NewReader(os.Stdin)

			prompt := func(label, def string) string {
				if def != "" {
					fmt.Printf("  %s [%s]: ", label, dim(def))
				} else {
					fmt.Printf("  %s: ", label)
				}
				line, _ := reader.ReadString('\n')
				line = strings.TrimSpace(line)
				if line == "" {
					return def
				}
				return line
			}

			fmt.Println(bold("Veld Registry — configuration setup"))
			fmt.Println()

			addr := flagAddr
			if addr == "" {
				addr = prompt("Listen address", ":8080")
			}

			dsn := flagDSN
			if dsn == "" {
				dsn = prompt("PostgreSQL DSN", "postgres://localhost/veld?sslmode=disable")
			}
			if dsn == "" {
				return fmt.Errorf("DSN is required")
			}

			secret := flagSecret
			if secret == "" {
				fmt.Println()
				fmt.Println(dim("  Tip: generate a secret with: openssl rand -hex 32"))
				secret = prompt("JWT secret (min 16 chars)", "")
			}
			if secret == "" {
				return fmt.Errorf("JWT secret is required")
			}
			if len(secret) < 16 {
				return fmt.Errorf("JWT secret must be at least 16 characters")
			}

			storage := flagStorage
			if storage == "" {
				storage = prompt("Tarball storage path", "./packages")
			}

			baseURL := flagBaseURL
			if baseURL == "" {
				baseURL = prompt("Public base URL (optional)", "")
			}

			cfg := map[string]interface{}{
				"addr":    addr,
				"dsn":     dsn,
				"secret":  secret,
				"storage": storage,
			}
			if baseURL != "" {
				cfg["base_url"] = baseURL
			}
			cfg["smtp"] = map[string]interface{}{
				"host":     "",
				"port":     587,
				"username": "",
				"password": "",
				"from":     "",
			}

			data, err := json.MarshalIndent(cfg, "", "  ")
			if err != nil {
				return err
			}

			if err := os.WriteFile(outPath, data, 0600); err != nil {
				return err
			}

			fmt.Println()
			fmt.Printf(green("✓")+" Created %s\n", bold(outPath))
			fmt.Println()
			fmt.Println("Next steps:")
			fmt.Printf("  1. %s\n", dim("Start the registry:"))
			fmt.Printf("     veld registry serve --config %s\n", outPath)
			fmt.Printf("  2. %s\n", dim("Open the web UI and create your account:"))
			fmt.Printf("     http://localhost%s\n", addr)
			fmt.Printf("  3. %s\n", dim("Log in from the CLI:"))
			fmt.Printf("     veld registry login --registry http://localhost%s --token vtk_...\n", addr)
			return nil
		},
	}

	cmd.Flags().StringVar(&flagAddr, "addr", "", "listen address (default :8080)")
	cmd.Flags().StringVar(&flagDSN, "dsn", "", "PostgreSQL DSN")
	cmd.Flags().StringVar(&flagSecret, "secret", "", "JWT signing secret (min 16 chars)")
	cmd.Flags().StringVar(&flagStorage, "storage", "", "tarball storage path (default ./packages)")
	cmd.Flags().StringVar(&flagBaseURL, "base-url", "", "public base URL (e.g. https://registry.example.com)")
	cmd.Flags().StringVar(&flagOut, "out", "", "output path (default ./registry.config.json)")
	return cmd
}

// ── serve (registry server) ───────────────────────────────────────────────────

// serveRegistryConfig mirrors registry.config.json on disk.
type serveRegistryConfig struct {
	Addr        string          `json:"addr"`
	DSN         string          `json:"dsn"`
	StoragePath string          `json:"storage"`
	JWTSecret   string          `json:"secret"`
	BaseURL     string          `json:"base_url"`
	SMTP        serveSmtpConfig `json:"smtp"`
}

type serveSmtpConfig struct {
	Host     string `json:"host"`
	Port     int    `json:"port"`
	Username string `json:"username"`
	Password string `json:"password"`
	From     string `json:"from"`
}

func loadServeConfigFile(path string) serveRegistryConfig {
	data, err := os.ReadFile(path)
	if err != nil {
		return serveRegistryConfig{}
	}
	var c serveRegistryConfig
	if err := json.Unmarshal(data, &c); err != nil {
		fmt.Fprintf(os.Stderr, yellow("warning: ")+"could not parse %s: %v\n", path, err)
	}
	return c
}

func resolveServeVal(candidates ...string) string {
	for _, c := range candidates {
		if c != "" {
			return c
		}
	}
	return ""
}

func newServeCmd() *cobra.Command {
	var configFile, flagAddr, flagDSN, flagStorage, flagSecret string

	cmd := &cobra.Command{
		Use:   "serve",
		Short: "Start the Veld Registry server",
		Long: `Start a self-hosted Veld Registry server.

Config is loaded from registry.config.json (current dir or --config flag).
CLI flags and environment variables override config file values.

Priority (highest → lowest): CLI flags > env vars > registry.config.json > defaults`,
		Example: `  # Use a config file (recommended)
  veld registry serve --config registry.config.json

  # All inline
  veld registry serve --addr :9000 --dsn "postgres://localhost/veld?sslmode=disable" --secret mysecret

  # Via environment variables
  VELD_DSN=postgres://localhost/veld VELD_SECRET=mysecret veld registry serve`,
		RunE: func(cmd *cobra.Command, args []string) error {
			// 1. Find config file
			cfgPath := configFile
			if cfgPath == "" {
				for _, candidate := range []string{"registry.config.json", "veld/registry.config.json"} {
					if _, err := os.Stat(candidate); err == nil {
						cfgPath = candidate
						break
					}
				}
			}

			// 2. Load config file
			fileCfg := loadServeConfigFile(cfgPath)
			if cfgPath != "" {
				if _, err := os.Stat(cfgPath); err == nil {
					fmt.Printf(dim("⬡")+"  Config: %s\n", cfgPath)
				}
			}

			// 3. Merge: file → env → flag (highest wins)
			cfg := server.Config{
				Addr:        resolveServeVal(flagAddr, os.Getenv("VELD_ADDR"), fileCfg.Addr, ":8080"),
				DSN:         resolveServeVal(flagDSN, os.Getenv("VELD_DSN"), fileCfg.DSN, ""),
				StoragePath: resolveServeVal(flagStorage, os.Getenv("VELD_STORAGE"), fileCfg.StoragePath, "./packages"),
				JWTSecret:   resolveServeVal(flagSecret, os.Getenv("VELD_SECRET"), fileCfg.JWTSecret, ""),
				BaseURL:     resolveServeVal(os.Getenv("VELD_BASE_URL"), fileCfg.BaseURL, ""),
				Email: email.Config{
					Host:     resolveServeVal(os.Getenv("SMTP_HOST"), fileCfg.SMTP.Host, ""),
					Port:     fileCfg.SMTP.Port,
					Username: resolveServeVal(os.Getenv("SMTP_USERNAME"), fileCfg.SMTP.Username, ""),
					Password: resolveServeVal(os.Getenv("SMTP_PASSWORD"), fileCfg.SMTP.Password, ""),
					From:     resolveServeVal(os.Getenv("SMTP_FROM"), fileCfg.SMTP.From, ""),
				},
			}

			// 4. Validate required fields
			if cfg.DSN == "" {
				return fmt.Errorf(
					"database DSN is required.\n\n" +
						"Set it in registry.config.json:\n" +
						"  { \"dsn\": \"postgres://localhost/veld?sslmode=disable\" }\n\n" +
						"Or via flag:  --dsn \"postgres://localhost/veld?sslmode=disable\"\n" +
						"Or via env:   VELD_DSN=postgres://localhost/veld?sslmode=disable",
				)
			}
			if cfg.JWTSecret == "" {
				return fmt.Errorf(
					"JWT secret is required.\n\n" +
						"Set it in registry.config.json:\n" +
						"  { \"secret\": \"your-secret-here\" }\n\n" +
						"Or via flag:  --secret \"your-secret\"\n" +
						"Or generate: openssl rand -hex 32",
				)
			}

			// 5. Start server
			srv, err := server.New(cfg)
			if err != nil {
				return fmt.Errorf("init: %w", err)
			}
			defer srv.Close()

			ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
			defer stop()

			fmt.Printf(green("✓")+" Veld Registry  →  http://localhost%s\n", cfg.Addr)
			fmt.Printf(dim("   Storage: %s\n"), cfg.StoragePath)

			return srv.Start(ctx)
		},
	}

	cmd.Flags().StringVarP(&configFile, "config", "c", "", "path to registry.config.json (auto-detected if omitted)")
	cmd.Flags().StringVar(&flagAddr, "addr", "", "listen address (default :8080)")
	cmd.Flags().StringVar(&flagDSN, "dsn", "", "PostgreSQL DSN")
	cmd.Flags().StringVar(&flagStorage, "storage", "", "tarball storage directory (default ./packages)")
	cmd.Flags().StringVar(&flagSecret, "secret", "", "JWT signing secret (min 16 chars)")
	return cmd
}

// ── registry helpers ──────────────────────────────────────────────────────────

// parsePackageRef parses "@org/name@version" → (org, name, version, err).
func parsePackageRef(ref string) (org, name, version string, err error) {
	// strip leading @
	s := strings.TrimPrefix(ref, "@")
	// split version
	parts := strings.SplitN(s, "@", 2)
	if len(parts) == 2 {
		version = parts[1]
	}
	// split org/name
	slash := strings.Index(parts[0], "/")
	if slash < 0 {
		err = fmt.Errorf("invalid package reference %q — expected @org/name[@version]", ref)
		return
	}
	org = parts[0][:slash]
	name = parts[0][slash+1:]
	if org == "" || name == "" {
		err = fmt.Errorf("invalid package reference %q — org and name must not be empty", ref)
	}
	return
}

func fmtBytes(n int64) string {
	switch {
	case n >= 1<<20:
		return fmt.Sprintf("%.1f MB", float64(n)/(1<<20))
	case n >= 1<<10:
		return fmt.Sprintf("%.1f kB", float64(n)/(1<<10))
	default:
		return fmt.Sprintf("%d B", n)
	}
}

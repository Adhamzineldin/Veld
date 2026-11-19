package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/cache"
	"github.com/Adhamzineldin/Veld/internal/config"
	"github.com/Adhamzineldin/Veld/internal/diff"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/lint"
	"github.com/Adhamzineldin/Veld/internal/loader"
	"github.com/Adhamzineldin/Veld/internal/output"
	"github.com/Adhamzineldin/Veld/internal/setup"
	"github.com/Adhamzineldin/Veld/internal/validator"
	"github.com/spf13/cobra"
	// Register all emitters via init(). To add a new emitter, add one line here.
	// Register tool emitters (auxiliary generators — NOT backends).
)

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

// emitOptions derives emitter options from a resolved config, so every
// command and workspace entry emits the same output for the same config.
// DryRun is never set: emitters always write, but only into staging
// directories, and the host decides whether anything is committed.
func emitOptions(rc config.ResolvedConfig) emitter.EmitOptions {
	return emitter.EmitOptions{
		BaseUrl:           rc.BaseUrl,
		Validate:          rc.Validate,
		BackendFramework:  rc.BackendFramework,
		FrontendFramework: rc.FrontendFramework,
		Services:          rc.Services,
		ServerSdk:         rc.ServerSdk,
		Description:       rc.Description,
	}
}

// runGenerate parses and validates the contract, then emits into run's
// staging directories. It writes nothing else: the cache and lock updates are
// queued on run and happen only after the output is committed.
//
// When incremental is false every module is regenerated.
// When incremental is true only modules whose source files changed are regenerated.
//
// Returns (regeneratedModuleNames, veldFileList, contractChanges, error).
// contractChanges is non-nil only when a .veld.lock.json baseline exists;
// callers are responsible for printing / acting on them.
func runGenerate(run *genRun, rc config.ResolvedConfig, incremental bool, opts emitter.EmitOptions) ([]string, []string, []diff.Change, error) {
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

	c := cache.Load(rc.ConfigDir)
	saveCache := func() {
		for _, f := range veldFiles {
			c.Update(f)
		}
		if err := c.Save(rc.ConfigDir); err != nil {
			fmt.Fprintf(os.Stderr, yellow("warning: ")+"cache save failed: %v\n", err)
		}
	}

	// ── incremental: compute which modules need regeneration ──────────────
	var targetModules map[string]bool
	if incremental {
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
		for i := range a.Modules {
			if anyModelFileChanged || changedFileSet[a.Modules[i].SourceFile] {
				targetModules[a.Modules[i].Name] = true
			}
		}

		if len(targetModules) == 0 {
			run.afterCommit(saveCache)
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

	// The backend gets the (possibly filtered) AST; the frontend SDK always
	// gets the full AST. Both have the app-level prefix merged into every
	// module so the SDK URLs match what the backend wires up.
	emitAST = emitter.ApplyTopLevelPrefix(emitAST)
	if err := run.emit(rc, emitAST, emitter.ApplyTopLevelPrefix(a), opts); err != nil {
		return nil, veldFiles, nil, err
	}

	// ── breaking-change diff; the new snapshot is saved after commit ─────
	var changes []diff.Change
	if hasLock {
		changes = diff.Diff(oldAST, a)
	}
	run.afterCommit(func() {
		saveCache()
		if err := diff.SaveLock(rc.ConfigDir, a); err != nil {
			fmt.Fprintf(os.Stderr, yellow("warning: ")+"lock save failed: %v\n", err)
		}
	})

	// ── lint hint ─────────────────────────────────────────────────────────
	// Run a quick lint pass and surface a one-liner so developers know to
	// investigate. Full details are always available via `veld lint`.
	if !run.dryRun {
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

// runWorkspaceGenerate handles code generation for workspace/microservice mode.
// It parses every workspace entry, resolves consumes, and emits code per entry.
// With serviceSDK (--service-sdk), backend entries that declare no consumes
// consume every backend sibling. Returns (regeneratedNames, allVeldFiles,
// changes, error).
func runWorkspaceGenerate(run *genRun, rc config.ResolvedConfig, flags config.FlagOverrides, opts emitter.EmitOptions, serviceSDK bool) ([]string, []string, []diff.Change, error) {
	// Validate consumes declarations.
	if errs, warns := validator.ValidateWorkspaceConsumes(rc.Workspace); len(errs) > 0 || len(warns) > 0 {
		for _, w := range warns {
			fmt.Fprintf(os.Stderr, yellow("⚠")+"  %s\n", w)
		}
		for _, e := range errs {
			fmt.Fprintf(os.Stderr, red("✗")+"  %s\n", e)
		}
		if len(errs) > 0 {
			return nil, nil, nil, fmt.Errorf("workspace validation failed")
		}
	}

	// Pass 1: parse all workspace entries and cache ASTs.
	type parsedEntry struct {
		rc     config.ResolvedConfig
		ast    ast.AST
		outDir string
	}
	parsed := make(map[string]*parsedEntry, len(rc.Workspace))
	var allVeldFiles []string

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

		if entry.BackendCfg != nil && entry.BackendCfg.Framework != "" {
			entryFlags.BackendFrameworkSet = true
			entryFlags.BackendFramework = entry.BackendCfg.Framework
		}

		// Output dirs are absolute before being passed as flags, because
		// BuildResolved resets cfgDir to CWD when InputSet=true.
		outDir, feOut := rc.WorkspaceOutDirs(entry)
		entryFlags.OutSet = true
		entryFlags.Out = outDir
		if feOut != "" {
			entryFlags.FrontendOutSet = true
			entryFlags.FrontendOut = feOut
		}

		entryRC, err := config.BuildResolved(entryFlags)
		if err != nil {
			return nil, nil, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
		}
		if entry.BaseUrl != "" {
			entryRC.BaseUrl = entry.BaseUrl
		}
		entryRC.ServerSdk = entry.ServerSdk || rc.ServerSdk
		entryRC.Description = rc.Description
		entryRC.Services = rc.Services

		// Cross-service aliases: `import @iam/models/*` resolves in any
		// sibling. @<service-name> → that service's source directory, two
		// levels up from its input file (services/iam/modules/iam.veld →
		// services/iam/).
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
			crossAliases[sibling.Name] = filepath.Dir(filepath.Dir(siblingInput))
		}

		// A frontend-only entry may have no input file: its AST is built
		// entirely from the services it consumes.
		isFrontend := frontendTarget != "" && frontendTarget != "none"
		a, veldFiles, err := loader.Parse(entryRC.Input, crossAliases)
		if err != nil && isFrontend {
			a = ast.AST{ASTVersion: "1.0.0"}
		} else if err != nil {
			return nil, nil, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
		}
		allVeldFiles = append(allVeldFiles, veldFiles...)

		parsed[entry.Name] = &parsedEntry{rc: entryRC, ast: a, outDir: outDir}
	}

	// Pass 2: generate each entry with consumed service resolution.
	var allRegenerated []string
	for _, entry := range rc.Workspace {
		fmt.Printf("  %s %s\n", bold("→"), entry.Name)
		pe := parsed[entry.Name]

		entryOpts := emitOptions(pe.rc)

		// Auto-consume: frontend entries always consume every backend
		// sibling so one SDK covers all services; with --service-sdk,
		// backend entries without consumes do too.
		consumesList := entry.Consumes
		isFrontendEntry := entry.Frontend != "" && entry.Frontend != "none"
		if isFrontendEntry || (serviceSDK && len(consumesList) == 0) {
			consumesList = nil
			for _, other := range rc.Workspace {
				if other.Name != entry.Name && (other.Frontend == "" || other.Frontend == "none") {
					consumesList = append(consumesList, other.Name)
				}
			}
		}

		for _, depName := range consumesList {
			dep := parsed[depName]
			if dep == nil {
				continue // validated above
			}
			entryOpts.ConsumedServices = append(entryOpts.ConsumedServices, emitter.ConsumedServiceInfo{
				Name:    depName,
				AST:     emitter.ApplyTopLevelPrefix(dep.ast),
				BaseUrl: dep.rc.BaseUrl,
			})
		}

		// Frontend entries get every consumed service merged into their own
		// AST, plus a per-module base URL map so each module's client
		// targets the right service.
		if isFrontendEntry && len(entryOpts.ConsumedServices) > 0 {
			pe.ast = emitter.MergeASTs(emitter.ApplyTopLevelPrefix(pe.ast), entryOpts.ConsumedServices)
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
			// The input file may not exist: emit the merged AST directly.
			emitAST := emitter.ApplyTopLevelPrefix(pe.ast)
			if err := run.emit(pe.rc, emitAST, emitAST, entryOpts); err != nil {
				return nil, allVeldFiles, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
			}
		} else {
			if _, _, _, err := runGenerate(run, pe.rc, false, entryOpts); err != nil {
				return nil, allVeldFiles, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
			}
		}

		consumedNames := make([]string, len(entryOpts.ConsumedServices))
		for i, c := range entryOpts.ConsumedServices {
			consumedNames[i] = c.Name
		}
		switch {
		case !isFrontendEntry && len(consumedNames) > 0:
			// Typed clients for consumed services, in the backend's language.
			backend, err := emitter.GetBackend(pe.rc.Backend)
			if err != nil {
				return nil, allVeldFiles, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
			}
			sdkOut, err := run.stagedDir(pe.rc.BackendOut)
			if err != nil {
				return nil, allVeldFiles, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
			}
			if err := backend.EmitServiceSdk(entryOpts.ConsumedServices, sdkOut, entryOpts); err != nil {
				return nil, allVeldFiles, nil, fmt.Errorf("workspace entry %q service sdk: %w", entry.Name, err)
			}
			fmt.Printf("  %s %s → sdk/ (%s)\n", green("✓"), entry.Name, strings.Join(consumedNames, ", "))
		case isFrontendEntry && len(consumedNames) > 0:
			fmt.Printf("  %s %s → unified frontend SDK (%s)\n", green("✓"), entry.Name, strings.Join(consumedNames, ", "))
		default:
			fmt.Printf("  %s %s → %s\n", green("✓"), entry.Name, pe.outDir)
		}

		allRegenerated = append(allRegenerated, entry.Name)
	}

	return allRegenerated, allVeldFiles, nil, nil
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
func newGenerateCmd() *cobra.Command {
	var backendFlag, frontendFlag, inputFlag, outFlag string
	var backendFrameworkFlag, frontendFrameworkFlag string
	var incrementalFlag, dryRunFlag, setupFlag, validateFlag, strictFlag, forceFlag, serverSdkFlag, serviceSdkFlag bool
	var overwriteUnmanagedFlagValue bool

	cmd := &cobra.Command{
		Use:   "generate",
		Short: "Generate code from a .veld contract",
		Long: "Generates typed backend interfaces and a frontend SDK from your .veld contract.\n\n" +
			"Every file is (re)generated by default — deterministic and safe for CI/CD.\n" +
			"Output is staged first and committed only if every step succeeds; Veld\n" +
			"never deletes or overwrites files it did not write (see .veld-manifest.json).\n" +
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

			opts := emitOptions(rc)
			mode := genMode{
				incremental:        incrementalFlag,
				dryRun:             dryRunFlag,
				overwriteUnmanaged: overwriteUnmanagedFlagValue,
				serviceSDK:         serviceSdkFlag,
			}

			// ── Workspace multi-service mode ─────────────────────────────────────
			if len(rc.Workspace) > 0 {
				fmt.Printf("\n%s workspace: %d services\n\n", bold("◆"), len(rc.Workspace))
				res, err := generate(rc, flags, opts, mode)
				if err != nil {
					return err
				}
				if dryRunFlag {
					fmt.Println("\n" + green("✓") + " Dry run — no files written. Would change:")
					printPlan(res.plan)
					return nil
				}
				printKept(res.plan)
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

			res, err := generate(rc, flags, opts, mode)
			if err != nil {
				return err
			}
			regenerated, changes := res.modules, res.changes

			if dryRunFlag {
				fmt.Println(green("✓") + " Dry run — no files written. Would change:")
				printPlan(res.plan)
				printDiffChanges(changes)
				return nil
			}
			printKept(res.plan)

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

			runPostGenerate(rc)
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
		"list the files that would be created, updated or deleted, writing nothing")
	cmd.Flags().BoolVar(&overwriteUnmanagedFlagValue, overwriteUnmanagedFlag[2:], false,
		"replace files in the output directory that Veld did not write (not listed in .veld-manifest.json)")
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
	a = emitter.ApplyTopLevelPrefix(a)
	return diff.Diff(oldAST, a)
}

// newCleanCmd builds `veld clean`. It deletes only what Veld wrote: the
// files listed in each output directory's .veld-manifest.json whose content
// is unchanged (ADR-007 §3). The breaking-change lock is kept unless --lock
// is given (ADR-007 §7).
func newCleanCmd() *cobra.Command {
	var lockFlag bool
	cmd := &cobra.Command{
		Use:   "clean",
		Short: "Remove the files Veld generated",
		Long: "Removes the files listed in each output directory's .veld-manifest.json,\n" +
			"then directories left empty. Files you added, and generated files you edited\n" +
			"by hand, are left in place. An output directory without a manifest is not\n" +
			"touched and makes the command fail. The breaking-change baseline\n" +
			"(.veld.lock.json) is kept unless --lock is given.",
		Example: "  veld clean\n  veld clean --lock",
		RunE: func(cmd *cobra.Command, args []string) error {
			rc, err := config.BuildResolved(config.FlagOverrides{})
			if err != nil {
				return err
			}

			if len(rc.Workspace) > 0 {
				fmt.Printf("\n%s workspace: %d services\n\n", bold("◆"), len(rc.Workspace))
			}

			var unmanaged []string
			for _, dir := range rc.OutputDirs() {
				res, err := output.Clean(dir)
				if err != nil {
					return fmt.Errorf("clean %s: %w", dir, err)
				}
				switch {
				case res.Managed:
					fmt.Printf("%s Cleaned %s (%d file(s))\n", green("✓"), bold(dir), len(res.Removed))
					for _, p := range res.Kept {
						fmt.Fprintf(os.Stderr, yellow("⚠")+"  kept %s: edited since Veld wrote it\n",
							filepath.Join(dir, filepath.FromSlash(p)))
					}
				case dirHasEntries(dir):
					unmanaged = append(unmanaged, dir)
				default:
					fmt.Printf("%s Nothing to clean in %s\n", green("✓"), dir)
				}
			}

			if err := cache.Delete(rc.ConfigDir); err != nil {
				return fmt.Errorf("remove cache: %w", err)
			}
			if lockFlag {
				if err := diff.DeleteLock(rc.ConfigDir); err != nil {
					return fmt.Errorf("remove lock: %w", err)
				}
				fmt.Println(green("✓") + " Removed the breaking-change baseline (.veld.lock.json)")
			}

			if len(unmanaged) > 0 {
				return fmt.Errorf("nothing removed from %s: no %s, so Veld cannot tell its files from yours. "+
					"Run `veld generate` once to record them, or delete the directory yourself",
					strings.Join(unmanaged, ", "), output.ManifestName)
			}
			return nil
		},
	}
	cmd.Flags().BoolVar(&lockFlag, "lock", false,
		"also remove the breaking-change baseline (.veld.lock.json)")
	return cmd
}

// dirHasEntries reports whether dir exists and is not empty.
func dirHasEntries(dir string) bool {
	entries, err := os.ReadDir(dir)
	return err == nil && len(entries) > 0
}

// ── lint ──────────────────────────────────────────────────────────────────────

// writeGeneratedReadme writes the README of an output directory.
func writeGeneratedReadme(outDir string, a ast.AST) error {
	var sb strings.Builder
	sb.WriteString(readmeTitle + "\n\n")
	sb.WriteString("> ⚠️ **DO NOT EDIT** the files `veld generate` writes here. They are listed in\n")
	sb.WriteString("> `.veld-manifest.json` and are overwritten on the next run. Files you add\n")
	sb.WriteString("> yourself are never modified or deleted by Veld.\n\n")

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

	if err := os.WriteFile(filepath.Join(outDir, "README.md"), []byte(sb.String()), 0o644); err != nil {
		return fmt.Errorf("write generated README: %w", err)
	}
	return nil
}

// ── graphql ───────────────────────────────────────────────────────────────────

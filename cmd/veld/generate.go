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

// runClean removes generated output directories and clears cache/lock files.
// Shared by generate and watch commands so output is always fresh.
func runClean(rc config.ResolvedConfig) {
	for _, dir := range rc.OutputDirs() {
		if _, statErr := os.Stat(dir); os.IsNotExist(statErr) {
			continue
		}
		if err := os.RemoveAll(dir); err != nil {
			fmt.Fprintf(os.Stderr, yellow("warning: ")+"failed to clean %s: %v\n", dir, err)
		}
	}
	cacheFile := filepath.Join(rc.ConfigDir, ".veld-cache.json")
	os.Remove(cacheFile)
	_ = diff.DeleteLock(rc.ConfigDir)
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
	emitAST = emitter.ApplyTopLevelPrefix(emitAST)

	// ── ensure output dirs exist ─────────────────────────────────────────
	// Emitters overwrite their own files (os.WriteFile). No directory wipe —
	// user files are never touched. Use `veld clean` for explicit cleanup.
	if !opts.DryRun {
		for _, dir := range rc.OutputDirs() {
			if dir == "" {
				continue
			}
			if err := os.MkdirAll(dir, 0755); err != nil {
				return nil, veldFiles, nil, fmt.Errorf("create output dir %s: %w", dir, err)
			}
		}
	}

	// ── emit: backend ────────────────────────────────────────────────────
	backendOrTool, _, err := emitter.GetBackendOrTool(rc.Backend)
	if err != nil {
		return nil, veldFiles, nil, err
	}
	if err := backendOrTool.Emit(emitAST, rc.BackendOut, opts); err != nil {
		return nil, veldFiles, nil, fmt.Errorf("%s emitter: %w", rc.Backend, err)
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
		// Frontend SDK always gets the full AST (combined output), with the
		// top-level prefix merged into every module so the SDK URLs match
		// what the backend emitter wires up.
		frontendAST := emitter.ApplyTopLevelPrefix(a)
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

// runWorkspaceGenerate handles code generation for workspace/microservice mode.
// It iterates over all workspace entries, parses each one, resolves consumes,
// and emits code per-entry. Returns (regeneratedNames, allVeldFiles, changes, error).
func runWorkspaceGenerate(rc config.ResolvedConfig, flags config.FlagOverrides, opts emitter.EmitOptions) ([]string, []string, []diff.Change, error) {
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
		flags  config.FlagOverrides
		outDir string
	}
	parsed := make(map[string]*parsedEntry, len(rc.Workspace))
	var allVeldFiles []string

	for _, entry := range rc.Workspace {
		entryFlags := flags
		entryFlags.InputSet = true
		entryFlags.Input = filepath.Join(rc.ConfigDir, entry.Input)

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
			return nil, nil, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
		}
		if entry.BaseUrl != "" {
			entryRC.BaseUrl = entry.BaseUrl
		}
		entryRC.ServerSdk = entry.ServerSdk || rc.ServerSdk
		entryRC.Description = rc.Description
		entryRC.Services = rc.Services

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
			serviceRoot := filepath.Dir(filepath.Dir(siblingInput))
			crossAliases[sibling.Name] = serviceRoot
		}

		isFrontend := frontendTarget != "" && frontendTarget != "none"
		a, veldFiles, err := loader.Parse(entryRC.Input, crossAliases)
		if err != nil && isFrontend {
			a = ast.AST{ASTVersion: "1.0.0"}
		} else if err != nil {
			return nil, nil, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
		}
		allVeldFiles = append(allVeldFiles, veldFiles...)

		parsed[entry.Name] = &parsedEntry{
			rc:     entryRC,
			ast:    a,
			flags:  entryFlags,
			outDir: outDir,
		}
	}

	// Pass 2: generate each entry with consumed service resolution.
	var allRegenerated []string
	for _, entry := range rc.Workspace {
		pe := parsed[entry.Name]

		entryOpts := emitter.EmitOptions{
			BaseUrl:           pe.rc.BaseUrl,
			DryRun:            opts.DryRun,
			Validate:          pe.rc.Validate,
			BackendFramework:  pe.rc.BackendFramework,
			FrontendFramework: pe.rc.FrontendFramework,
			Services:          pe.rc.Services,
			ServerSdk:         pe.rc.ServerSdk,
			Description:       pe.rc.Description,
		}

		consumesList := entry.Consumes
		isFrontendEntry := entry.Frontend != "" && entry.Frontend != "none"
		if isFrontendEntry || len(consumesList) == 0 {
			if isFrontendEntry {
				consumesList = nil
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
					continue
				}
				consumed = append(consumed, emitter.ConsumedServiceInfo{
					Name:    depName,
					AST:     emitter.ApplyTopLevelPrefix(dep.ast),
					BaseUrl: dep.rc.BaseUrl,
				})
			}
			entryOpts.ConsumedServices = consumed
		}

		if isFrontendEntry && len(entryOpts.ConsumedServices) > 0 {
			pe.ast = emitter.ApplyTopLevelPrefix(pe.ast)
			mergedAST := emitter.MergeASTs(pe.ast, entryOpts.ConsumedServices)
			pe.ast = mergedAST

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
			if err := runGenerateWithAST(pe.rc, pe.ast, entryOpts); err != nil {
				return nil, allVeldFiles, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
			}
		} else {
			if _, _, _, err := runGenerate(pe.rc, false, entryOpts); err != nil {
				return nil, allVeldFiles, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
			}
		}

		// Emit service SDK clients for consumed services.
		if !isFrontendEntry && len(entryOpts.ConsumedServices) > 0 {
			backend, err := emitter.GetBackend(pe.rc.Backend)
			if err != nil {
				return nil, allVeldFiles, nil, fmt.Errorf("workspace entry %q: %w", entry.Name, err)
			}
			if err := backend.EmitServiceSdk(entryOpts.ConsumedServices, pe.rc.BackendOut, entryOpts); err != nil {
				return nil, allVeldFiles, nil, fmt.Errorf("workspace entry %q service sdk: %w", entry.Name, err)
			}
		}

		allRegenerated = append(allRegenerated, entry.Name)
	}

	return allRegenerated, allVeldFiles, nil, nil
}

// runGenerateWithAST is like runGenerate but takes a pre-built AST instead of
// calling loader.Parse. Used by frontend workspace entries whose AST has been
// merged from all consumed backend services.
func runGenerateWithAST(rc config.ResolvedConfig, a ast.AST, opts emitter.EmitOptions) error {
	if opts.DryRun {
		return nil
	}

	// Ensure output dirs exist (no wipe — emitters overwrite their own files).
	for _, dir := range rc.OutputDirs() {
		if dir == "" {
			continue
		}
		if err := os.MkdirAll(dir, 0755); err != nil {
			return fmt.Errorf("create output dir %s: %w", dir, err)
		}
	}

	emitAST := emitter.ApplyTopLevelPrefix(a)

	// Emit backend (types, routes, etc.).
	backendOrTool, _, err := emitter.GetBackendOrTool(rc.Backend)
	if err != nil {
		return err
	}
	if err := backendOrTool.Emit(emitAST, rc.BackendOut, opts); err != nil {
		return fmt.Errorf("%s emitter: %w", rc.Backend, err)
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

			// ── Clean before generating (skip for dry-run) ──────────────────────
			if !dryRunFlag {
				runClean(rc)
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
								AST:     emitter.ApplyTopLevelPrefix(dep.ast),
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
						pe.ast = emitter.ApplyTopLevelPrefix(pe.ast)
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
	a = emitter.ApplyTopLevelPrefix(a)
	return diff.Diff(oldAST, a)
}

// promptContinue prints a [y/N] prompt and returns true only if the user
// types "y" or "Y". Any other input (including Enter) returns false.
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

			if len(rc.Workspace) > 0 {
				fmt.Printf("\n%s workspace: %d services\n\n", bold("◆"), len(rc.Workspace))
			}

			cleaned := false
			for _, dir := range rc.OutputDirs() {
				if _, statErr := os.Stat(dir); !os.IsNotExist(statErr) {
					cleaned = true
				}
			}

			runClean(rc)

			if cleaned {
				for _, dir := range rc.OutputDirs() {
					fmt.Println(green("✓") + " Cleaned " + bold(dir))
				}
			} else {
				fmt.Println(green("✓") + " Nothing to clean — output directory does not exist")
			}
			return nil
		},
	}
}

// ── lint ──────────────────────────────────────────────────────────────────────

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

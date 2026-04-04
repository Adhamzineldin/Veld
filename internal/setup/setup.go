// Package setup auto-patches project config files so generated code can be
// imported without manual edits (tsconfig paths, pubspec dependencies, etc.).
package setup

import (
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

// Result describes what happened for a single config file.
type Result struct {
	File   string // e.g. "tsconfig.json"
	Action string // "patched" | "skipped" | "not-found" | "manual"
	Detail string // human-readable explanation
}

// Options carries optional directory overrides for setup.
type Options struct {
	BackendDir     string // directory containing backend project files (default: projectDir)
	FrontendDir    string // directory containing frontend project files (default: projectDir)
	BackendOutDir  string // output dir for backend code (overrides outDir for backend)
	FrontendOutDir string // output dir for frontend code (overrides outDir for frontend)
}

// Run inspects the project directory and patches config files for the given
// backend/frontend combination. outDir is the generated output directory
// (relative or absolute). All patching is idempotent — if the output path
// changed, existing entries are updated in place.
func Run(projectDir, backend, frontend, outDir string, opts ...Options) []Result {
	var o Options
	if len(opts) > 0 {
		o = opts[0]
	}
	backendDir := projectDir
	if o.BackendDir != "" {
		backendDir = o.BackendDir
	}
	frontendDir := projectDir
	if o.FrontendDir != "" {
		frontendDir = o.FrontendDir
	}

	var results []Result

	// relOutFor returns outDir as a path relative to baseDir (slash-normalised).
	// Relative outDir values are first resolved against projectDir so the result
	// is always correct from the perspective of files inside baseDir (e.g. pom.xml).
	relOutFor := func(dir, baseDir string) string {
		abs := dir
		if !filepath.IsAbs(dir) {
			abs = filepath.Join(projectDir, dir)
		}
		rel, err := filepath.Rel(baseDir, abs)
		if err != nil {
			return filepath.ToSlash(dir) // fallback: return as-is
		}
		return filepath.ToSlash(rel)
	}

	backendOutDir := outDir
	if o.BackendOutDir != "" {
		backendOutDir = o.BackendOutDir
	}
	frontendOutDir := outDir
	if o.FrontendOutDir != "" {
		frontendOutDir = o.FrontendOutDir
	}

	relOutBackend := relOutFor(backendOutDir, backendDir)
	relOutFrontend := relOutFor(frontendOutDir, frontendDir)

	type patcher struct {
		fn func() Result
	}

	// ── Backend patchers (run against backendDir) ────────────────────────
	var backendPatchers []patcher

	switch backend {
	case "node-ts", "node-js":
		// Primary: add file: dependency for real package resolution (Prisma-style).
		backendPatchers = append(backendPatchers, patcher{func() Result {
			return patchNodePackageJSON(backendDir, relOutBackend, "@veld/generated")
		}})
		// Fallback: also patch tsconfig for TypeScript path resolution and IDE support.
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchTSConfig(backendDir, relOutBackend) }})
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchViteConfig(backendDir, relOutBackend) }})
	case "python":
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchPythonPath(backendDir, relOutBackend) }})
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchRequirementsTxt(backendDir) }})
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchPyprojectToml(backendDir) }})
	case "go":
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchGoMod(backendDir, relOutBackend) }})
	case "rust":
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchCargoToml(backendDir, relOutBackend) }})
	case "java":
		// Maven: add build-helper-maven-plugin to include generated/src/main/java as a source root
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchPomXML(backendDir, relOutBackend) }})
		// Gradle: add srcDir to sourceSets instead of a separate module
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchGradleSourceSet(backendDir, relOutBackend) }})
	case "csharp":
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchCsproj(backendDir, relOutBackend) }})
	case "php":
		backendPatchers = append(backendPatchers, patcher{func() Result { return patchComposerJSON(backendDir, relOutBackend) }})
	}

	// ── Frontend patchers (run against frontendDir) ──────────────────────
	var frontendPatchers []patcher

	// frontendPkgName returns the correct @veld/* package name for each frontend SDK.
	frontendPkgName := func() string {
		switch frontend {
		case "react":
			return "@veld/hooks"
		case "vue":
			return "@veld/composables"
		case "svelte":
			return "@veld/stores"
		case "angular":
			return "@veld/services"
		default:
			return "@veld/client"
		}
	}

	// frontendSubdir returns the client/ sub-package path within the generated output.
	frontendSubdir := func() string {
		switch frontend {
		case "react":
			return "hooks"
		case "vue":
			return "composables"
		case "svelte":
			return "stores"
		case "angular":
			return "services"
		default:
			return "client"
		}
	}

	switch frontend {
	case "typescript", "javascript", "react", "vue", "angular", "svelte":
		// Primary: add file: dependencies for both the root generated package and the frontend sub-package.
		frontendPatchers = append(frontendPatchers, patcher{func() Result {
			return patchNodePackageJSON(frontendDir, relOutFrontend, "@veld/generated")
		}})
		frontendPatchers = append(frontendPatchers, patcher{func() Result {
			subDir := relOutFrontend + "/" + frontendSubdir()
			return patchNodePackageJSON(frontendDir, subDir, frontendPkgName())
		}})
		// Fallback: also patch tsconfig for TypeScript path resolution and IDE support.
		frontendPatchers = append(frontendPatchers, patcher{func() Result { return patchTSConfig(frontendDir, relOutFrontend) }})
		frontendPatchers = append(frontendPatchers, patcher{func() Result { return patchViteConfig(frontendDir, relOutFrontend) }})
	case "dart", "flutter":
		frontendPatchers = append(frontendPatchers, patcher{func() Result { return patchPubspecYAML(frontendDir, relOutFrontend) }})
	case "kotlin":
		frontendPatchers = append(frontendPatchers, patcher{func() Result {
			return patchGradleSettings(frontendDir, relOutFrontend+"/client", "veld-client")
		}})
		frontendPatchers = append(frontendPatchers, patcher{func() Result { return patchGradleBuildDep(frontendDir, "veld-client") }})
	case "swift":
		frontendPatchers = append(frontendPatchers, patcher{func() Result {
			return Result{
				File:   "Xcode",
				Action: "manual",
				Detail: "add " + relOutFrontend + "/client/ as a local Swift package dependency",
			}
		}})
	}

	// If backend and frontend resolve to the same directory AND the same
	// config file type (e.g. both "node" backend + "react" frontend both
	// need package.json + tsconfig.json), we still run the frontend sub-package
	// patcher but skip duplicate root-level patches.
	skipFrontend := false
	if backendDir == frontendDir && backendOutDir == frontendOutDir {
		switch backend {
		case "node-ts", "node-js":
			switch frontend {
			case "typescript", "javascript", "react", "vue", "angular", "svelte":
				skipFrontend = true // same package.json + tsconfig — backend patcher already covers root
			}
		}
	}

	for _, p := range backendPatchers {
		results = append(results, p.fn())
	}
	if !skipFrontend {
		for _, p := range frontendPatchers {
			results = append(results, p.fn())
		}
	}

	return results
}

// ── helpers ──────────────────────────────────────────────────────────────────

// findFile looks for filename in dir, then one directory up, then in common
// subdirectories (src, backend, frontend). This handles monorepo layouts
// where the user hasn't explicitly set backendDir / frontendDir.
func findFile(dir, filename string) string {
	p := filepath.Join(dir, filename)
	if _, err := os.Stat(p); err == nil {
		return p
	}
	parent := filepath.Dir(dir)
	if parent != dir {
		p = filepath.Join(parent, filename)
		if _, err := os.Stat(p); err == nil {
			return p
		}
	}
	// Try common subdirectories
	for _, sub := range []string{"src", "backend", "frontend", "server", "client", "app"} {
		p = filepath.Join(dir, sub, filename)
		if _, err := os.Stat(p); err == nil {
			return p
		}
	}
	return ""
}

// findFileGlob looks for files matching a glob pattern in dir, then one up,
// then in common subdirectories.
func findFileGlob(dir, pattern string) string {
	matches, _ := filepath.Glob(filepath.Join(dir, pattern))
	if len(matches) > 0 {
		return matches[0]
	}
	parent := filepath.Dir(dir)
	if parent != dir {
		matches, _ = filepath.Glob(filepath.Join(parent, pattern))
		if len(matches) > 0 {
			return matches[0]
		}
	}
	for _, sub := range []string{"src", "backend", "frontend", "server", "client", "app"} {
		matches, _ = filepath.Glob(filepath.Join(dir, sub, pattern))
		if len(matches) > 0 {
			return matches[0]
		}
	}
	return ""
}

// ── patchers ─────────────────────────────────────────────────────────────────

// patchTSConfig adds @veld/* path alias to tsconfig.json compilerOptions.paths.
// If the alias already exists but points to a different directory, it is updated.
func patchTSConfig(dir, outDir string) Result {
	path := findFile(dir, "tsconfig.json")
	if path == "" {
		return Result{File: "tsconfig.json", Action: "not-found", Detail: "create a tsconfig.json first"}
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: "tsconfig.json", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	newMapping := "\"./" + outDir + "/*\""

	// Already configured — check if path needs updating.
	if strings.Contains(content, "@veld/*") {
		if strings.Contains(content, newMapping) {
			return Result{File: "tsconfig.json", Action: "skipped", Detail: "@veld/* already points to " + outDir}
		}
		// Update the existing mapping to the new outDir.
		re := regexp.MustCompile(`("@veld/\*"\s*:\s*\[\s*)"[^"]*"`)
		content = re.ReplaceAllString(content, "${1}"+newMapping)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "tsconfig.json", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "tsconfig.json", Action: "patched", Detail: "updated @veld/* path to " + outDir}
	}

	pathsEntry := `      "@veld/*": ["./' + outDir + '/*"]`
	pathsEntry = "      \"@veld/*\": [\"./" + outDir + "/*\"]"

	// Strategy: find "paths" object and insert, or find "compilerOptions" and add paths.
	if strings.Contains(content, `"paths"`) {
		// Insert into existing paths object — find the line with "paths" and add after the opening brace
		lines := strings.Split(content, "\n")
		var result []string
		for i, line := range lines {
			result = append(result, line)
			trimmed := strings.TrimSpace(line)
			if strings.Contains(trimmed, `"paths"`) {
				// Look for opening brace on this line or next
				if strings.Contains(line, "{") {
					result = append(result, pathsEntry+",")
				} else if i+1 < len(lines) && strings.Contains(lines[i+1], "{") {
					// The { is on the next line; we'll insert after it
					result = append(result, lines[i+1])
					result = append(result, pathsEntry+",")
					// Skip the next line since we already added it
					lines[i+1] = ""
				}
			}
		}
		content = strings.Join(result, "\n")
	} else if strings.Contains(content, `"compilerOptions"`) {
		// Add paths block inside compilerOptions
		lines := strings.Split(content, "\n")
		var result []string
		for _, line := range lines {
			result = append(result, line)
			trimmed := strings.TrimSpace(line)
			if strings.Contains(trimmed, `"compilerOptions"`) && strings.Contains(line, "{") {
				result = append(result, "    \"paths\": {")
				result = append(result, pathsEntry)
				result = append(result, "    },")
			}
		}
		content = strings.Join(result, "\n")
	} else {
		// No compilerOptions — wrap the whole thing
		content = strings.TrimRight(content, " \t\r\n")
		if strings.HasSuffix(content, "}") {
			content = content[:len(content)-1] +
				"  \"compilerOptions\": {\n    \"paths\": {\n" + pathsEntry + "\n    }\n  }\n}"
		}
	}

	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "tsconfig.json", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "tsconfig.json", Action: "patched", Detail: "added @veld/* path alias"}
}

// patchViteConfig adds a resolve.alias entry for @veld to vite.config.ts/js/mjs.
// Vite does NOT read tsconfig paths by default, so projects using Vite need
// resolve.alias in their config for @veld/* imports to work at dev/build time.
// If no vite.config file is found, the result is silently skipped (the project
// may not use Vite at all).
func patchViteConfig(dir, outDir string) Result {
	// Try all common Vite config file names.
	var path string
	for _, name := range []string{"vite.config.ts", "vite.config.js", "vite.config.mjs"} {
		p := filepath.Join(dir, name)
		if _, err := os.Stat(p); err == nil {
			path = p
			break
		}
	}
	if path == "" {
		// No Vite config — project likely doesn't use Vite. Not an error.
		return Result{File: "vite.config.*", Action: "skipped", Detail: "no vite config found (not using Vite)"}
	}

	filename := filepath.Base(path)
	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: filename, Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	// The alias value we want: path.resolve(__dirname, '<outDir>')
	newResolve := `path.resolve(__dirname, '` + outDir + `')`

	// ── Already has @veld alias — check if it needs updating ─────────────
	if strings.Contains(content, `'@veld'`) || strings.Contains(content, `"@veld"`) {
		if strings.Contains(content, outDir) {
			return Result{File: filename, Action: "skipped", Detail: "@veld alias already points to " + outDir}
		}
		// Update the existing alias value.
		re := regexp.MustCompile(`(['"]@veld['"]\s*:\s*)(?:path\.resolve\([^)]*\)|['"][^'"]*['"])`)
		content = re.ReplaceAllString(content, "${1}"+newResolve)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: filename, Action: "not-found", Detail: err.Error()}
		}
		return Result{File: filename, Action: "patched", Detail: "updated @veld alias to " + outDir}
	}

	// ── Need to add the alias ────────────────────────────────────────────

	// Ensure `import path from 'path'` (or `import * as path`) is present.
	needsPathImport := !strings.Contains(content, "'path'") && !strings.Contains(content, `"path"`)
	if needsPathImport {
		// Insert path import at the top, after any existing imports or at line 0.
		// Find the last import line to insert after it.
		lines := strings.Split(content, "\n")
		lastImportIdx := -1
		for i, line := range lines {
			trimmed := strings.TrimSpace(line)
			if strings.HasPrefix(trimmed, "import ") {
				lastImportIdx = i
			}
		}
		pathImport := "import path from 'path';"
		if lastImportIdx >= 0 {
			// Insert after last import.
			newLines := make([]string, 0, len(lines)+1)
			newLines = append(newLines, lines[:lastImportIdx+1]...)
			newLines = append(newLines, pathImport)
			newLines = append(newLines, lines[lastImportIdx+1:]...)
			content = strings.Join(newLines, "\n")
		} else {
			content = pathImport + "\n" + content
		}
	}

	aliasEntry := `      '@veld': ` + newResolve

	// Strategy 1: existing resolve.alias object — insert into it.
	if strings.Contains(content, "resolve") && strings.Contains(content, "alias") {
		// Find the alias: { ... } block and insert our entry.
		re := regexp.MustCompile(`(alias\s*:\s*\{)`)
		if re.MatchString(content) {
			content = re.ReplaceAllString(content, "${1}\n"+aliasEntry+",")
			if err := os.WriteFile(path, []byte(content), 0644); err != nil {
				return Result{File: filename, Action: "not-found", Detail: err.Error()}
			}
			return Result{File: filename, Action: "patched", Detail: "added @veld alias"}
		}
	}

	// Strategy 2: existing resolve block but no alias — add alias inside resolve.
	if strings.Contains(content, "resolve") {
		re := regexp.MustCompile(`(resolve\s*:\s*\{)`)
		if re.MatchString(content) {
			aliasBlock := "${1}\n    alias: {\n" + aliasEntry + ",\n    },"
			content = re.ReplaceAllString(content, aliasBlock)
			if err := os.WriteFile(path, []byte(content), 0644); err != nil {
				return Result{File: filename, Action: "not-found", Detail: err.Error()}
			}
			return Result{File: filename, Action: "patched", Detail: "added resolve.alias with @veld"}
		}
	}

	// Strategy 3: no resolve block — add resolve inside defineConfig({...}) or the exported config.
	// Look for defineConfig({ and insert resolve after the opening.
	reDefineConfig := regexp.MustCompile(`(defineConfig\(\s*\{)`)
	if reDefineConfig.MatchString(content) {
		resolveBlock := "${1}\n  resolve: {\n    alias: {\n" + aliasEntry + ",\n    },\n  },"
		content = reDefineConfig.ReplaceAllString(content, resolveBlock)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: filename, Action: "not-found", Detail: err.Error()}
		}
		return Result{File: filename, Action: "patched", Detail: "added resolve.alias with @veld"}
	}

	// Strategy 4: export default { ... } without defineConfig.
	reExport := regexp.MustCompile(`(export\s+default\s*\{)`)
	if reExport.MatchString(content) {
		resolveBlock := "${1}\n  resolve: {\n    alias: {\n" + aliasEntry + ",\n    },\n  },"
		content = reExport.ReplaceAllString(content, resolveBlock)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: filename, Action: "not-found", Detail: err.Error()}
		}
		return Result{File: filename, Action: "patched", Detail: "added resolve.alias with @veld"}
	}

	return Result{File: filename, Action: "manual", Detail: "add resolve.alias: { '@veld': path.resolve(__dirname, '" + outDir + "') }"}
}

// patchNodePackageJSON adds a file: dependency to the project's package.json
// so that @veld/* packages are real installable local packages (like Prisma).
// This eliminates the need for tsconfig path aliases and works natively with
// node, ts-node, jest, vite, webpack, and all other Node.js tools.
//
// Example: adds `"@veld/generated": "file:./generated"` to dependencies.
func patchNodePackageJSON(dir, outDir, pkgName string) Result {
	pkgPath := findFile(dir, "package.json")
	if pkgPath == "" {
		return Result{File: "package.json", Action: "not-found", Detail: "create a package.json first (npm init)"}
	}

	data, err := os.ReadFile(pkgPath)
	if err != nil {
		return Result{File: "package.json", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	fileRef := "file:" + outDir

	// Already has the dependency — check if path needs updating.
	if strings.Contains(content, `"`+pkgName+`"`) {
		if strings.Contains(content, fileRef) {
			return Result{File: "package.json", Action: "skipped", Detail: pkgName + " already points to " + outDir}
		}
		// Update the existing file: reference.
		re := regexp.MustCompile(`("` + regexp.QuoteMeta(pkgName) + `"\s*:\s*)"[^"]*"`)
		content = re.ReplaceAllString(content, `${1}"`+fileRef+`"`)
		if err := os.WriteFile(pkgPath, []byte(content), 0644); err != nil {
			return Result{File: "package.json", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "package.json", Action: "patched", Detail: "updated " + pkgName + " → " + fileRef}
	}

	depEntry := `    "` + pkgName + `": "` + fileRef + `"`

	// Strategy 1: insert into existing "dependencies" block.
	if strings.Contains(content, `"dependencies"`) {
		lines := strings.Split(content, "\n")
		var result []string
		inserted := false
		for i, line := range lines {
			result = append(result, line)
			if inserted {
				continue
			}
			trimmed := strings.TrimSpace(line)
			if strings.Contains(trimmed, `"dependencies"`) {
				// Look for opening brace on this line or next.
				if strings.Contains(line, "{") {
					// Check if it's an empty object "dependencies": {}
					if strings.Contains(line, "}") {
						// Replace empty object with our entry
						result[len(result)-1] = strings.Replace(line, "{}", "{\n"+depEntry+"\n  }", 1)
					} else {
						result = append(result, depEntry+",")
					}
					inserted = true
				} else if i+1 < len(lines) && strings.Contains(lines[i+1], "{") {
					result = append(result, lines[i+1])
					result = append(result, depEntry+",")
					lines[i+1] = ""
					inserted = true
				}
			}
		}
		if inserted {
			content = strings.Join(result, "\n")
		}
	} else {
		// Strategy 2: no "dependencies" block — add one.
		content = strings.TrimRight(content, " \t\r\n")
		if strings.HasSuffix(content, "}") {
			// Find the last non-whitespace before the closing brace.
			idx := strings.LastIndex(content, "}")
			before := strings.TrimRight(content[:idx], " \t\r\n")
			// Add comma if the last line doesn't end with one or a brace.
			if !strings.HasSuffix(before, ",") && !strings.HasSuffix(before, "{") {
				before += ","
			}
			content = before + "\n  \"dependencies\": {\n" + depEntry + "\n  }\n}"
		}
	}

	if err := os.WriteFile(pkgPath, []byte(content), 0644); err != nil {
		return Result{File: "package.json", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "package.json", Action: "patched", Detail: "added " + pkgName + " → " + fileRef + " (run npm install)"}
}

// patchRequirementsTxt adds pydantic>=2.0 to requirements.txt.
func patchRequirementsTxt(dir string) Result {
	path := findFile(dir, "requirements.txt")
	if path == "" {
		return Result{File: "requirements.txt", Action: "not-found", Detail: "create a requirements.txt first"}
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: "requirements.txt", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	if strings.Contains(strings.ToLower(content), "pydantic") {
		return Result{File: "requirements.txt", Action: "skipped", Detail: "pydantic already listed"}
	}

	content = strings.TrimRight(content, "\n") + "\npydantic>=2.0\n"
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "requirements.txt", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "requirements.txt", Action: "patched", Detail: "added pydantic>=2.0"}
}

// patchPythonPath creates or updates a veld_path.py file that dynamically adds
// the generated package's parent directory to sys.path. This works immediately
// at runtime — no pip install -e . required. The user just imports veld_path at
// the top of their entry point (e.g. `import veld_path`) and the generated
// package becomes importable.
//
// For internal dirs (e.g. "generated"), the parent is "." (the project root).
// For external dirs (e.g. "../generated"), the parent is ".." so that
// `import generated` resolves to `../generated/`.
//
// Re-running setup with a different outDir updates the path in place.
func patchPythonPath(dir, outDir string) Result {
	path := filepath.Join(dir, "veld_path.py")

	// We add the PARENT of outDir to sys.path so `import <pkg>` works.
	// e.g. "generated" → ".", "../generated" → "..", "../../shared/gen" → "../../shared"
	parentDir := filepath.ToSlash(filepath.Dir(outDir))
	if parentDir == "." || parentDir == "" {
		parentDir = "."
	}

	marker := "# veld:generated-path"
	pathLine := `_veld_root = _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), "` + parentDir + `")`
	insertLine := `if _veld_root not in _sys.path:  ` + marker + "\n    _sys.path.insert(0, _veld_root)"

	fullContent := "import os as _os, sys as _sys\n" +
		pathLine + "\n" +
		insertLine + "\n"

	data, err := os.ReadFile(path)
	if err == nil {
		content := string(data)

		// Already has our marker — check if path needs updating.
		if strings.Contains(content, marker) {
			if strings.Contains(content, `"`+parentDir+`"`) {
				return Result{File: "veld_path.py", Action: "skipped", Detail: "sys.path already configured for " + outDir}
			}
			// Path changed — rewrite the whole file (it's small and fully managed by veld).
			if err := os.WriteFile(path, []byte(fullContent), 0644); err != nil {
				return Result{File: "veld_path.py", Action: "not-found", Detail: err.Error()}
			}
			return Result{File: "veld_path.py", Action: "patched", Detail: "updated path for " + outDir}
		}

		// File exists but no marker — rewrite (shouldn't happen, but handle it).
		if err := os.WriteFile(path, []byte(fullContent), 0644); err != nil {
			return Result{File: "veld_path.py", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "veld_path.py", Action: "patched", Detail: "added path setup for " + outDir}
	}

	// File does not exist — create it.
	if err := os.WriteFile(path, []byte(fullContent), 0644); err != nil {
		return Result{File: "veld_path.py", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "veld_path.py", Action: "patched", Detail: "created veld_path.py — add `import veld_path` to your entry point"}
}

// patchGoMod adds a replace directive for the generated module.
// If the directive already exists but points to a different path, it is updated.
func patchGoMod(dir, outDir string) Result {
	path := findFile(dir, "go.mod")
	if path == "" {
		return Result{File: "go.mod", Action: "not-found", Detail: "no go.mod found — run go mod init first"}
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: "go.mod", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	marker := "veld/generated"
	newDirective := "replace veld/generated => ./" + outDir
	if strings.Contains(content, marker) {
		// Check if the path already matches.
		if strings.Contains(content, newDirective) {
			return Result{File: "go.mod", Action: "skipped", Detail: "veld/generated replace already points to " + outDir}
		}
		// Update the existing replace directive.
		re := regexp.MustCompile(`replace\s+veld/generated\s+=>\s+\S+`)
		content = re.ReplaceAllString(content, newDirective)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "go.mod", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "go.mod", Action: "patched", Detail: "updated replace directive to ./" + outDir}
	}

	directive := "\nreplace veld/generated => ./" + outDir + "\n"
	content = strings.TrimRight(content, "\n") + "\n" + directive
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "go.mod", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "go.mod", Action: "patched", Detail: "added replace directive for veld/generated"}
}

// patchCargoToml adds generated dir to workspace members.
// If an existing veld entry points to a different path, it is updated.
func patchCargoToml(dir, outDir string) Result {
	path := findFile(dir, "Cargo.toml")
	if path == "" {
		return Result{File: "Cargo.toml", Action: "not-found", Detail: "no Cargo.toml found"}
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: "Cargo.toml", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	if strings.Contains(content, outDir) {
		return Result{File: "Cargo.toml", Action: "skipped", Detail: "generated dir already in workspace"}
	}

	// Check if there is a previous veld-generated path in members that needs updating.
	// Look for old "generated" or similar path entries containing "generated" in the members array.
	re := regexp.MustCompile(`"[^"]*generated[^"]*"`)
	if strings.Contains(content, "[workspace]") && re.MatchString(content) {
		content = re.ReplaceAllString(content, `"`+outDir+`"`)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "Cargo.toml", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "Cargo.toml", Action: "patched", Detail: "updated workspace member to " + outDir}
	}

	if strings.Contains(content, "[workspace]") {
		// Insert into existing workspace members
		if strings.Contains(content, "members") {
			// Add to existing members array
			content = strings.Replace(content, "members = [", "members = [\n    \""+outDir+"\",", 1)
		} else {
			content = strings.Replace(content, "[workspace]", "[workspace]\nmembers = [\""+outDir+"\"]", 1)
		}
	} else {
		content = strings.TrimRight(content, "\n") + "\n\n[workspace]\nmembers = [\"" + outDir + "\"]\n"
	}

	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "Cargo.toml", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "Cargo.toml", Action: "patched", Detail: "added " + outDir + " to workspace members"}
}

// patchPomXML adds build-helper-maven-plugin to pom.xml so that
// generated/src/main/java is compiled as part of the project without needing
// a separate Maven submodule. This is idempotent: re-running with a different
// outDir updates the existing <source> entry in place.
func patchPomXML(dir, outDir string) Result {
	path := findFile(dir, "pom.xml")
	if path == "" {
		return Result{File: "pom.xml", Action: "not-found", Detail: "no pom.xml found"}
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: "pom.xml", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	// outDir is relative to dir (the veld project dir), but pom.xml may live in
	// a parent directory. ${project.basedir} in Maven equals the pom.xml's own
	// directory, so we must compute sourceDir relative to that — not relative to
	// the veld config dir.
	pomDir := filepath.Dir(path)
	absOut := outDir
	if !filepath.IsAbs(outDir) {
		absOut = filepath.Join(dir, outDir)
	}
	if rel, err2 := filepath.Rel(pomDir, absOut); err2 == nil {
		outDir = filepath.ToSlash(rel)
	}

	sourceDir := outDir + "/src/main/java"

	// Already configured — check if the source path needs updating.
	if strings.Contains(content, "build-helper-maven-plugin") {
		if strings.Contains(content, sourceDir) {
			return Result{File: "pom.xml", Action: "skipped", Detail: "build-helper-maven-plugin already points to " + sourceDir}
		}
		re := regexp.MustCompile(`(<source>\$\{project\.basedir\}/)([^<]*)(</source>)`)
		content = re.ReplaceAllString(content, "${1}"+sourceDir+"${3}")
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "pom.xml", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "pom.xml", Action: "patched", Detail: "updated build-helper source to " + sourceDir}
	}

	plugin := `
      <plugin>
        <groupId>org.codehaus.mojo</groupId>
        <artifactId>build-helper-maven-plugin</artifactId>
        <executions>
          <execution>
            <phase>generate-sources</phase>
            <goals><goal>add-source</goal></goals>
            <configuration>
              <sources>
                <source>${project.basedir}/` + sourceDir + `</source>
              </sources>
            </configuration>
          </execution>
        </executions>
      </plugin>`

	if strings.Contains(content, "<plugins>") {
		content = strings.Replace(content, "<plugins>", "<plugins>"+plugin, 1)
	} else if strings.Contains(content, "<build>") {
		content = strings.Replace(content, "<build>",
			"<build>\n    <plugins>"+plugin+"\n    </plugins>", 1)
	} else if strings.Contains(content, "</project>") {
		content = strings.Replace(content, "</project>",
			"  <build>\n    <plugins>"+plugin+"\n    </plugins>\n  </build>\n</project>", 1)
	}

	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "pom.xml", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "pom.xml", Action: "patched", Detail: "added build-helper-maven-plugin for " + sourceDir}
}

// patchGradleSourceSet adds the generated source directory to the main sourceSet
// in build.gradle.kts (or build.gradle), so Gradle compiles generated Java files
// without a separate subproject.
func patchGradleSourceSet(dir, outDir string) Result {
	var path string
	for _, name := range []string{"build.gradle.kts", "build.gradle"} {
		if p := findFile(dir, name); p != "" {
			path = p
			break
		}
	}
	if path == "" {
		return Result{File: "build.gradle.kts", Action: "not-found", Detail: "no build.gradle.kts found"}
	}

	filename := filepath.Base(path)
	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: filename, Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	sourceDir := outDir + "/src/main/java"
	if strings.Contains(content, sourceDir) {
		return Result{File: filename, Action: "skipped", Detail: "source directory already configured"}
	}

	isKts := strings.HasSuffix(path, ".kts")
	var entry string
	if isKts {
		entry = "\nsourceSets {\n    main {\n        java {\n            srcDir(\"" + sourceDir + "\")\n        }\n    }\n}\n"
	} else {
		entry = "\nsourceSets {\n    main {\n        java {\n            srcDirs += ['" + sourceDir + "']\n        }\n    }\n}\n"
	}

	// If a sourceSets block already exists, insert inside it instead of appending a new one.
	if strings.Contains(content, "sourceSets") {
		reSS := regexp.MustCompile(`(sourceSets\s*\{)`)
		if isKts {
			content = reSS.ReplaceAllString(content,
				"${1}\n    main {\n        java {\n            srcDir(\""+sourceDir+"\")\n        }\n    }")
		} else {
			content = reSS.ReplaceAllString(content,
				"${1}\n    main {\n        java {\n            srcDirs += ['"+sourceDir+"']\n        }\n    }")
		}
	} else {
		content = strings.TrimRight(content, "\n") + "\n" + entry
	}

	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: filename, Action: "not-found", Detail: err.Error()}
	}
	return Result{File: filename, Action: "patched", Detail: "added " + sourceDir + " to sourceSets"}
}

// patchCsproj adds a ProjectReference to the first .csproj found.
// If an existing veld reference points to a different path, it is updated.
func patchCsproj(dir, outDir string) Result {
	path := findFileGlob(dir, "*.csproj")
	if path == "" {
		return Result{File: "*.csproj", Action: "not-found", Detail: "no .csproj file found"}
	}

	filename := filepath.Base(path)
	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: filename, Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	ref := outDir + "/" + outDir + ".csproj"
	if strings.Contains(content, ref) {
		return Result{File: filename, Action: "skipped", Detail: "project reference already points to " + outDir}
	}

	// Check for an existing veld project reference with a different path.
	re := regexp.MustCompile(`<ProjectReference\s+Include="[^"]*generated[^"]*\.csproj"\s*/>`)
	if re.MatchString(content) || strings.Contains(content, "veld") {
		if re.MatchString(content) {
			content = re.ReplaceAllString(content, `<ProjectReference Include="`+ref+`" />`)
			if err := os.WriteFile(path, []byte(content), 0644); err != nil {
				return Result{File: filename, Action: "not-found", Detail: err.Error()}
			}
			return Result{File: filename, Action: "patched", Detail: "updated ProjectReference to " + outDir}
		}
		return Result{File: filename, Action: "skipped", Detail: "project reference already configured"}
	}

	projectRef := "  <ItemGroup>\n    <ProjectReference Include=\"" + ref + "\" />\n  </ItemGroup>"
	if strings.Contains(content, "</Project>") {
		content = strings.Replace(content, "</Project>", projectRef+"\n</Project>", 1)
	}

	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: filename, Action: "not-found", Detail: err.Error()}
	}
	return Result{File: filename, Action: "patched", Detail: "added ProjectReference to " + outDir}
}

// patchComposerJSON adds PSR-4 autoload entry for the generated namespace.
// If the namespace already exists with a different path, it is updated.
func patchComposerJSON(dir, outDir string) Result {
	path := findFile(dir, "composer.json")
	if path == "" {
		return Result{File: "composer.json", Action: "not-found", Detail: "no composer.json found"}
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: "composer.json", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	newEntry := "\"" + outDir + "/\""
	if strings.Contains(content, "Veld\\\\Generated") || strings.Contains(content, "Veld\\Generated") {
		// Check if it already points to the correct path.
		if strings.Contains(content, "Veld\\\\Generated\\\\\":") && strings.Contains(content, newEntry) {
			return Result{File: "composer.json", Action: "skipped", Detail: "Veld\\Generated already points to " + outDir}
		}
		if strings.Contains(content, "Veld\\Generated\":") && strings.Contains(content, newEntry) {
			return Result{File: "composer.json", Action: "skipped", Detail: "Veld\\Generated already points to " + outDir}
		}
		// Update the path.
		re := regexp.MustCompile(`("Veld\\\\Generated\\\\"\s*:\s*)"[^"]*"`)
		content = re.ReplaceAllString(content, "${1}"+newEntry)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "composer.json", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "composer.json", Action: "patched", Detail: "updated Veld\\Generated path to " + outDir + "/"}
	}

	entry := "\"Veld\\\\Generated\\\\\": \"" + outDir + "/\""
	if strings.Contains(content, `"psr-4"`) {
		// Insert into existing psr-4 object
		content = strings.Replace(content, `"psr-4": {`, `"psr-4": {`+"\n            "+entry+",", 1)
	} else if strings.Contains(content, `"autoload"`) {
		content = strings.Replace(content, `"autoload": {`,
			`"autoload": {`+"\n        \"psr-4\": {\n            "+entry+"\n        },", 1)
	} else if strings.Contains(content, "}") {
		// Find the last } and insert autoload before it
		idx := strings.LastIndex(content, "}")
		content = content[:idx] + ",\n    \"autoload\": {\n        \"psr-4\": {\n            " + entry + "\n        }\n    }\n}"
	}

	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "composer.json", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "composer.json", Action: "patched", Detail: "added Veld\\Generated PSR-4 autoload"}
}

// patchPubspecYAML adds veld_client path dependency to pubspec.yaml.
// If veld_client already exists with a different path, it is updated.
func patchPubspecYAML(dir, outDir string) Result {
	path := findFile(dir, "pubspec.yaml")
	if path == "" {
		return Result{File: "pubspec.yaml", Action: "not-found", Detail: "no pubspec.yaml found"}
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: "pubspec.yaml", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	newPath := "./" + outDir + "/client"
	if strings.Contains(content, "veld_client") {
		if strings.Contains(content, newPath) {
			return Result{File: "pubspec.yaml", Action: "skipped", Detail: "veld_client already points to " + outDir}
		}
		// Update the existing path.
		re := regexp.MustCompile(`(veld_client:\s*\n\s*path:\s*)\S+`)
		content = re.ReplaceAllString(content, "${1}"+newPath)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "pubspec.yaml", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "pubspec.yaml", Action: "patched", Detail: "updated veld_client path to " + newPath}
	}

	dep := "  veld_client:\n    path: ./" + outDir + "/client"
	if strings.Contains(content, "dependencies:") {
		content = strings.Replace(content, "dependencies:", "dependencies:\n"+dep, 1)
	} else {
		content = strings.TrimRight(content, "\n") + "\n\ndependencies:\n" + dep + "\n"
	}

	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "pubspec.yaml", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "pubspec.yaml", Action: "patched", Detail: "added veld_client path dependency"}
}

// patchGradleSettings adds include(":moduleName") + projectDir to settings.gradle.kts
// (or settings.gradle if the .kts variant is not found).
// projectPath is the filesystem path passed to file(...), e.g. "generated" or "generated/client".
// If the entry already exists with a different path, it is updated.
func patchGradleSettings(dir, projectPath, moduleName string) Result {
	var path string
	for _, name := range []string{"settings.gradle.kts", "settings.gradle"} {
		if p := findFile(dir, name); p != "" {
			path = p
			break
		}
	}
	if path == "" {
		return Result{File: "settings.gradle.kts", Action: "not-found", Detail: "no settings.gradle.kts found"}
	}

	filename := filepath.Base(path)
	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: filename, Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	newProjectDir := `file("` + projectPath + `")`
	if strings.Contains(content, moduleName) {
		if strings.Contains(content, newProjectDir) {
			return Result{File: filename, Action: "skipped", Detail: moduleName + " already points to " + projectPath}
		}
		re := regexp.MustCompile(`(project\("` + regexp.QuoteMeta(":"+moduleName) + `"\)\.projectDir\s*=\s*)file\("[^"]*"\)`)
		if re.MatchString(content) {
			content = re.ReplaceAllString(content, "${1}"+newProjectDir)
			if err := os.WriteFile(path, []byte(content), 0644); err != nil {
				return Result{File: filename, Action: "not-found", Detail: err.Error()}
			}
			return Result{File: filename, Action: "patched", Detail: "updated :" + moduleName + " project path to " + projectPath}
		}
		return Result{File: filename, Action: "skipped", Detail: moduleName + " already configured"}
	}

	entry := "\ninclude(\":" + moduleName + "\")\nproject(\":" + moduleName + "\").projectDir = " + newProjectDir + "\n"
	content = strings.TrimRight(content, "\n") + "\n" + entry
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: filename, Action: "not-found", Detail: err.Error()}
	}
	return Result{File: filename, Action: "patched", Detail: "added :" + moduleName + " project include"}
}

// patchGradleBuildDep adds implementation(project(":moduleName")) to build.gradle.kts
// (or build.gradle). If the dependency already exists, it is skipped.
func patchGradleBuildDep(dir, moduleName string) Result {
	var path string
	for _, name := range []string{"build.gradle.kts", "build.gradle", "app/build.gradle.kts", "app/build.gradle"} {
		p := filepath.Join(dir, name)
		if _, err := os.Stat(p); err == nil {
			path = p
			break
		}
	}
	if path == "" {
		return Result{File: "build.gradle.kts", Action: "not-found", Detail: "no build.gradle.kts found"}
	}

	filename := filepath.Base(path)
	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: filename, Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	dep := `implementation(project(":` + moduleName + `"))`
	if strings.Contains(content, dep) {
		return Result{File: filename, Action: "skipped", Detail: moduleName + " dependency already present"}
	}

	if strings.Contains(content, "dependencies {") {
		content = strings.Replace(content, "dependencies {", "dependencies {\n    "+dep, 1)
	} else {
		content = strings.TrimRight(content, "\n") + "\n\ndependencies {\n    " + dep + "\n}\n"
	}

	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: filename, Action: "not-found", Detail: err.Error()}
	}
	return Result{File: filename, Action: "patched", Detail: `added implementation(project(":` + moduleName + `"))`}
}

// patchPyprojectToml adds pydantic>=2.0 to pyproject.toml.
// Handles both Poetry ([tool.poetry.dependencies]) and PEP 621 ([project] dependencies) styles.
func patchPyprojectToml(dir string) Result {
	path := findFile(dir, "pyproject.toml")
	if path == "" {
		return Result{File: "pyproject.toml", Action: "not-found", Detail: "no pyproject.toml found"}
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: "pyproject.toml", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	if strings.Contains(strings.ToLower(content), "pydantic") {
		return Result{File: "pyproject.toml", Action: "skipped", Detail: "pydantic already listed"}
	}

	// Poetry style
	if strings.Contains(content, "[tool.poetry.dependencies]") {
		content = strings.Replace(content, "[tool.poetry.dependencies]",
			"[tool.poetry.dependencies]\npydantic = \">=2.0\"", 1)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "pyproject.toml", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "pyproject.toml", Action: "patched", Detail: "added pydantic>=2.0 (poetry)"}
	}

	// PEP 621 style: dependencies = [...]
	re := regexp.MustCompile(`(dependencies\s*=\s*\[)`)
	if re.MatchString(content) {
		content = re.ReplaceAllString(content, "${1}\n    \"pydantic>=2.0\",")
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "pyproject.toml", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "pyproject.toml", Action: "patched", Detail: "added pydantic>=2.0 (PEP 621)"}
	}

	return Result{File: "pyproject.toml", Action: "manual", Detail: `add pydantic>=2.0 to your [project] dependencies or [tool.poetry.dependencies]`}
}

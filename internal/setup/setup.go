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
	BackendDir  string // directory containing backend project files (default: projectDir)
	FrontendDir string // directory containing frontend project files (default: projectDir)
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
	done := map[string]bool{} // dedup by patcher name

	// relOutFor returns outDir relative to the given base directory (slash-normalised).
	relOutFor := func(baseDir string) string {
		rel := outDir
		if filepath.IsAbs(outDir) {
			if r, err := filepath.Rel(baseDir, outDir); err == nil {
				rel = r
			}
		}
		return filepath.ToSlash(rel)
	}

	relOutBackend := relOutFor(backendDir)
	relOutFrontend := relOutFor(frontendDir)

	type patcher struct {
		name string
		fn   func(string, string) Result
	}

	var patchers []patcher

	switch backend {
	case "node":
		patchers = append(patchers, patcher{"tsconfig", func(dir, out string) Result { return patchTSConfig(dir, out) }})
	case "python":
		patchers = append(patchers, patcher{"requirements", func(dir, _ string) Result { return patchRequirementsTxt(dir) }})
		patchers = append(patchers, patcher{"conftest", func(dir, out string) Result { return patchConftest(dir, out) }})
	case "go":
		patchers = append(patchers, patcher{"gomod", func(dir, out string) Result { return patchGoMod(dir, out) }})
	case "rust":
		patchers = append(patchers, patcher{"cargo", func(dir, out string) Result { return patchCargoToml(dir, out) }})
	case "java":
		patchers = append(patchers, patcher{"pom", func(dir, out string) Result { return patchPomXML(dir, out) }})
	case "csharp":
		patchers = append(patchers, patcher{"csproj", func(dir, out string) Result { return patchCsproj(dir, out) }})
	case "php":
		patchers = append(patchers, patcher{"composer", func(dir, out string) Result { return patchComposerJSON(dir, out) }})
	}

	switch frontend {
	case "typescript", "react", "vue", "angular", "svelte":
		patchers = append(patchers, patcher{"tsconfig", func(dir, out string) Result { return patchTSConfig(dir, out) }})
	case "dart", "flutter":
		patchers = append(patchers, patcher{"pubspec", func(dir, out string) Result { return patchPubspecYAML(dir, out) }})
	case "kotlin":
		patchers = append(patchers, patcher{"gradle", func(dir, out string) Result { return patchGradleKts(dir, out) }})
	case "swift":
		patchers = append(patchers, patcher{"swift", func(_, _ string) Result {
			return Result{
				File:   "Xcode",
				Action: "manual",
				Detail: "add " + relOutFrontend + "/client/ as a local Swift package dependency",
			}
		}})
	}

	for _, p := range patchers {
		if done[p.name] {
			continue
		}
		done[p.name] = true

		// Backend patchers run against backendDir; frontend patchers against frontendDir.
		dir := backendDir
		relOut := relOutBackend
		switch p.name {
		case "tsconfig", "pubspec", "gradle", "swift":
			// These may be used by both backend (node) and frontend.
			// If a dedicated frontendDir was provided, prefer it; otherwise
			// fall through to the backend dir (covers node backend tsconfig).
			if o.FrontendDir != "" {
				dir = frontendDir
				relOut = relOutFrontend
			}
		}

		results = append(results, p.fn(dir, relOut))
	}

	return results
}

// ── helpers ──────────────────────────────────────────────────────────────────

// findFile looks for filename in dir, then one directory up.
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
	return ""
}

// findFileGlob looks for files matching a glob pattern in dir, then one up.
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

// patchConftest creates or patches conftest.py to add the generated directory
// to sys.path, so Python code can import from the generated folder directly:
//
//	from types import User
//	from interfaces.i_users_service import IUsersService
//
// If conftest.py already contains the path, it is updated if the outDir changed.
func patchConftest(dir, outDir string) Result {
	path := filepath.Join(dir, "conftest.py")

	marker := "# veld:generated-path"
	newLine := `sys.path.insert(0, os.path.join(os.path.dirname(__file__), "` + outDir + `"))  ` + marker

	// Check if conftest.py exists and already has our marker.
	data, err := os.ReadFile(path)
	if err == nil {
		content := string(data)
		if strings.Contains(content, marker) {
			if strings.Contains(content, `"`+outDir+`"`) {
				return Result{File: "conftest.py", Action: "skipped", Detail: "sys.path already points to " + outDir}
			}
			// Update the existing line.
			re := regexp.MustCompile(`(?m)^.*` + regexp.QuoteMeta(marker) + `.*$`)
			content = re.ReplaceAllString(content, newLine)
			if err := os.WriteFile(path, []byte(content), 0644); err != nil {
				return Result{File: "conftest.py", Action: "not-found", Detail: err.Error()}
			}
			return Result{File: "conftest.py", Action: "patched", Detail: "updated sys.path to " + outDir}
		}
		// Marker not found — append it.
		content = strings.TrimRight(content, "\n") + "\n\n" +
			"import os, sys  # noqa: E401\n" + newLine + "\n"
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "conftest.py", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "conftest.py", Action: "patched", Detail: "added sys.path for " + outDir}
	}

	// conftest.py doesn't exist — create it.
	content := "# AUTO-GENERATED BY VELD — safe to extend\nimport os, sys  # noqa: E401\n" + newLine + "\n"
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "conftest.py", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "conftest.py", Action: "patched", Detail: "created conftest.py with sys.path for " + outDir}
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

// patchPomXML adds <module>generated</module> to pom.xml.
// If a veld module entry already exists with a different path, it is updated.
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

	moduleTag := "<module>" + outDir + "</module>"
	if strings.Contains(content, moduleTag) {
		return Result{File: "pom.xml", Action: "skipped", Detail: "module already listed with correct path"}
	}

	// Check for an existing veld-generated module entry with a different path.
	re := regexp.MustCompile(`<module>[^<]*generated[^<]*</module>`)
	if re.MatchString(content) {
		content = re.ReplaceAllString(content, moduleTag)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			return Result{File: "pom.xml", Action: "not-found", Detail: err.Error()}
		}
		return Result{File: "pom.xml", Action: "patched", Detail: "updated module path to " + outDir}
	}

	if strings.Contains(content, "<modules>") {
		content = strings.Replace(content, "<modules>", "<modules>\n        "+moduleTag, 1)
	} else if strings.Contains(content, "</project>") {
		content = strings.Replace(content, "</project>",
			"    <modules>\n        "+moduleTag+"\n    </modules>\n</project>", 1)
	}

	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "pom.xml", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "pom.xml", Action: "patched", Detail: "added <module>" + outDir + "</module>"}
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

// patchGradleKts adds include(":veld-client") + projectDir to settings.gradle.kts.
// If the entry already exists with a different path, it is updated.
func patchGradleKts(dir, outDir string) Result {
	path := findFile(dir, "settings.gradle.kts")
	if path == "" {
		return Result{File: "settings.gradle.kts", Action: "not-found", Detail: "no settings.gradle.kts found"}
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return Result{File: "settings.gradle.kts", Action: "not-found", Detail: err.Error()}
	}
	content := string(data)

	newProjectDir := `file("` + outDir + `/client")`
	if strings.Contains(content, "veld-client") {
		if strings.Contains(content, newProjectDir) {
			return Result{File: "settings.gradle.kts", Action: "skipped", Detail: "veld-client already points to " + outDir}
		}
		// Update the existing projectDir path if the line exists.
		re := regexp.MustCompile(`(project\(":veld-client"\)\.projectDir\s*=\s*)file\("[^"]*"\)`)
		if re.MatchString(content) {
			content = re.ReplaceAllString(content, "${1}"+newProjectDir)
			if err := os.WriteFile(path, []byte(content), 0644); err != nil {
				return Result{File: "settings.gradle.kts", Action: "not-found", Detail: err.Error()}
			}
			return Result{File: "settings.gradle.kts", Action: "patched", Detail: "updated :veld-client project path to " + outDir}
		}
		// Include exists but no projectDir line — already configured.
		return Result{File: "settings.gradle.kts", Action: "skipped", Detail: "veld-client already configured"}
	}

	entry := "\ninclude(\":veld-client\")\nproject(\":veld-client\").projectDir = " + newProjectDir + "\n"
	content = strings.TrimRight(content, "\n") + "\n" + entry
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return Result{File: "settings.gradle.kts", Action: "not-found", Detail: err.Error()}
	}
	return Result{File: "settings.gradle.kts", Action: "patched", Detail: "added :veld-client project include"}
}

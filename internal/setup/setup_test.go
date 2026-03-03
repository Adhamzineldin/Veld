package setup

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// helper creates a temp dir with optional files and returns its path.
func tmpProject(t *testing.T, files map[string]string) string {
	t.Helper()
	dir := t.TempDir()
	for name, content := range files {
		p := filepath.Join(dir, name)
		if err := os.MkdirAll(filepath.Dir(p), 0755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte(content), 0644); err != nil {
			t.Fatal(err)
		}
	}
	return dir
}

func findResult(results []Result, file string) *Result {
	for _, r := range results {
		if r.File == file {
			return &r
		}
	}
	return nil
}

// ── TSConfig tests ───────────────────────────────────────────────────────────

func TestPatchTSConfig_Patched(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	r := patchTSConfig(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "tsconfig.json"))
	if !strings.Contains(string(data), `@veld/*`) {
		t.Fatal("tsconfig.json should contain @veld/* path alias")
	}
	if !strings.Contains(string(data), `./generated/*`) {
		t.Fatal("tsconfig.json should contain ./generated/* mapping")
	}
}

func TestPatchTSConfig_Skipped(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "paths": {
      "@veld/*": ["./generated/*"]
    }
  }
}`,
	})
	r := patchTSConfig(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s", r.Action)
	}
}

func TestPatchTSConfig_NotFound(t *testing.T) {
	dir := t.TempDir()
	r := patchTSConfig(dir, "generated")
	if r.Action != "not-found" {
		t.Fatalf("expected not-found, got %s", r.Action)
	}
}

func TestPatchTSConfig_ExistingPaths(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "paths": {
      "@app/*": ["./src/*"]
    }
  }
}`,
	})
	r := patchTSConfig(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s", r.Action)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "tsconfig.json"))
	content := string(data)
	if !strings.Contains(content, `@veld/*`) {
		t.Fatal("should add @veld/* to existing paths")
	}
	if !strings.Contains(content, `@app/*`) {
		t.Fatal("should preserve existing paths")
	}
}

// ── Vite config tests ────────────────────────────────────────────────────────

func TestPatchViteConfig_NoFile(t *testing.T) {
	dir := t.TempDir()
	r := patchViteConfig(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped when no vite config, got %s: %s", r.Action, r.Detail)
	}
}

func TestPatchViteConfig_DefineConfig(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"vite.config.ts": `import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
})
`,
	})
	r := patchViteConfig(dir, "../generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "vite.config.ts"))
	content := string(data)
	if !strings.Contains(content, "'@veld'") {
		t.Fatal("should contain @veld alias")
	}
	if !strings.Contains(content, "path.resolve") {
		t.Fatal("should contain path.resolve")
	}
	if !strings.Contains(content, "../generated") {
		t.Fatal("should contain the outDir path")
	}
	if !strings.Contains(content, "import path from 'path'") {
		t.Fatal("should add path import")
	}
}

func TestPatchViteConfig_ExistingAlias(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"vite.config.ts": `import { defineConfig } from 'vite'
import path from 'path'

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
    },
  },
})
`,
	})
	r := patchViteConfig(dir, "../generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "vite.config.ts"))
	content := string(data)
	if !strings.Contains(content, "'@veld'") {
		t.Fatal("should contain @veld alias")
	}
	if !strings.Contains(content, "'@'") {
		t.Fatal("should preserve existing @ alias")
	}
}

func TestPatchViteConfig_AlreadyConfigured(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"vite.config.ts": `import { defineConfig } from 'vite'
import path from 'path'

export default defineConfig({
  resolve: {
    alias: {
      '@veld': path.resolve(__dirname, '../generated'),
    },
  },
})
`,
	})
	r := patchViteConfig(dir, "../generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s: %s", r.Action, r.Detail)
	}
}

func TestPatchViteConfig_UpdatePath(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"vite.config.ts": `import { defineConfig } from 'vite'
import path from 'path'

export default defineConfig({
  resolve: {
    alias: {
      '@veld': path.resolve(__dirname, './old-generated'),
    },
  },
})
`,
	})
	r := patchViteConfig(dir, "../new-generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "vite.config.ts"))
	content := string(data)
	if !strings.Contains(content, "../new-generated") {
		t.Fatal("should update to the new outDir path")
	}
	if strings.Contains(content, "old-generated") {
		t.Fatal("should not contain the old path")
	}
}

func TestPatchViteConfig_JSFile(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"vite.config.js": `import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [],
})
`,
	})
	r := patchViteConfig(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "vite.config.js"))
	content := string(data)
	if !strings.Contains(content, "'@veld'") {
		t.Fatal("should contain @veld alias in .js config")
	}
}

func TestPatchViteConfig_ExportDefault(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"vite.config.ts": `import react from '@vitejs/plugin-react'

export default {
  plugins: [react()],
}
`,
	})
	r := patchViteConfig(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "vite.config.ts"))
	content := string(data)
	if !strings.Contains(content, "'@veld'") {
		t.Fatal("should contain @veld alias with plain export default")
	}
	if !strings.Contains(content, "resolve") {
		t.Fatal("should add resolve block")
	}
}

func TestPatchViteConfig_SkipsPathImportIfPresent(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"vite.config.ts": `import { defineConfig } from 'vite'
import * as path from 'path'

export default defineConfig({
  plugins: [],
})
`,
	})
	r := patchViteConfig(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "vite.config.ts"))
	content := string(data)
	// Should NOT add a second path import.
	count := strings.Count(content, "'path'")
	if count != 1 {
		t.Fatalf("should have exactly 1 path import, got %d", count)
	}
}

// ── Requirements.txt tests ───────────────────────────────────────────────────

func TestPatchRequirementsTxt_Patched(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"requirements.txt": "flask>=3.0\n",
	})
	r := patchRequirementsTxt(dir)
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s", r.Action)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "requirements.txt"))
	if !strings.Contains(string(data), "pydantic>=2.0") {
		t.Fatal("should contain pydantic>=2.0")
	}
}

func TestPatchRequirementsTxt_Skipped(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"requirements.txt": "pydantic>=2.0\nflask>=3.0\n",
	})
	r := patchRequirementsTxt(dir)
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s", r.Action)
	}
}

func TestPatchRequirementsTxt_NotFound(t *testing.T) {
	dir := t.TempDir()
	r := patchRequirementsTxt(dir)
	if r.Action != "not-found" {
		t.Fatalf("expected not-found, got %s", r.Action)
	}
}

// ── veld_path.py tests ───────────────────────────────────────────────────────

func TestPatchPythonPath_CreatesNew(t *testing.T) {
	dir := t.TempDir()
	r := patchPythonPath(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "veld_path.py"))
	content := string(data)
	// Internal dir "generated" → parent is "." (project root)
	if !strings.Contains(content, `"."`) {
		t.Fatalf("should add '.' (project root) to sys.path for internal dir, got:\n%s", content)
	}
	if !strings.Contains(content, "veld:generated-path") {
		t.Fatal("should contain marker comment")
	}
	if !strings.Contains(r.Detail, "import veld_path") {
		t.Fatalf("result detail should mention import veld_path, got: %s", r.Detail)
	}
}

func TestPatchPythonPath_ExternalPath(t *testing.T) {
	dir := t.TempDir()
	r := patchPythonPath(dir, "../generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "veld_path.py"))
	content := string(data)
	// "../generated" → parent is ".."
	if !strings.Contains(content, `".."`) {
		t.Fatalf("should add '..' to sys.path for external dir, got:\n%s", content)
	}
}

func TestPatchPythonPath_DeepExternalPath(t *testing.T) {
	dir := t.TempDir()
	r := patchPythonPath(dir, "../../shared/generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "veld_path.py"))
	content := string(data)
	// "../../shared/generated" → parent is "../../shared"
	if !strings.Contains(content, `"../../shared"`) {
		t.Fatalf("should add '../../shared' to sys.path, got:\n%s", content)
	}
}

func TestPatchPythonPath_Skipped(t *testing.T) {
	dir := t.TempDir()
	patchPythonPath(dir, "generated")
	r := patchPythonPath(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped on second run, got %s: %s", r.Action, r.Detail)
	}
}

func TestPatchPythonPath_UpdatesOnPathChange(t *testing.T) {
	dir := t.TempDir()
	patchPythonPath(dir, "generated")
	r := patchPythonPath(dir, "../new-output")
	if r.Action != "patched" {
		t.Fatalf("expected patched on path change, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "veld_path.py"))
	content := string(data)
	if !strings.Contains(content, `".."`) {
		t.Fatalf("should now point to '..', got:\n%s", content)
	}
	// Old path should be gone
	if strings.Contains(content, `"."`) && !strings.Contains(content, `".."`) {
		t.Fatal("should no longer contain old path")
	}
	// Only one marker
	if strings.Count(content, "veld:generated-path") != 1 {
		t.Fatal("should have exactly one marker")
	}
}

// ── go.mod tests ─────────────────────────────────────────────────────────────

func TestPatchGoMod_Patched(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"go.mod": "module myapp\n\ngo 1.21\n",
	})
	r := patchGoMod(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s", r.Action)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "go.mod"))
	if !strings.Contains(string(data), "replace veld/generated => ./generated") {
		t.Fatal("should contain replace directive")
	}
}

func TestPatchGoMod_Skipped(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"go.mod": "module myapp\n\ngo 1.21\n\nreplace veld/generated => ./generated\n",
	})
	r := patchGoMod(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s", r.Action)
	}
}

// ── Cargo.toml tests ─────────────────────────────────────────────────────────

func TestPatchCargoToml_Patched(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"Cargo.toml": "[package]\nname = \"myapp\"\n",
	})
	r := patchCargoToml(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s", r.Action)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "Cargo.toml"))
	if !strings.Contains(string(data), `"generated"`) {
		t.Fatal("should contain generated in workspace members")
	}
}

func TestPatchCargoToml_Skipped(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"Cargo.toml": "[workspace]\nmembers = [\"generated\"]\n",
	})
	r := patchCargoToml(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s", r.Action)
	}
}

// ── pom.xml tests ────────────────────────────────────────────────────────────

func TestPatchPomXML_Patched(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"pom.xml": "<project>\n</project>\n",
	})
	r := patchPomXML(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s", r.Action)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "pom.xml"))
	if !strings.Contains(string(data), "<module>generated</module>") {
		t.Fatal("should contain <module>generated</module>")
	}
}

func TestPatchPomXML_Skipped(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"pom.xml": "<project>\n    <modules>\n        <module>generated</module>\n    </modules>\n</project>\n",
	})
	r := patchPomXML(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s", r.Action)
	}
}

// ── .csproj tests ────────────────────────────────────────────────────────────

func TestPatchCsproj_Patched(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"MyApp.csproj": "<Project>\n</Project>\n",
	})
	r := patchCsproj(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "MyApp.csproj"))
	if !strings.Contains(string(data), "ProjectReference") {
		t.Fatal("should contain ProjectReference")
	}
}

func TestPatchCsproj_NotFound(t *testing.T) {
	dir := t.TempDir()
	r := patchCsproj(dir, "generated")
	if r.Action != "not-found" {
		t.Fatalf("expected not-found, got %s", r.Action)
	}
}

// ── composer.json tests ──────────────────────────────────────────────────────

func TestPatchComposerJSON_Patched(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"composer.json": `{
    "name": "my/app"
}`,
	})
	r := patchComposerJSON(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "composer.json"))
	if !strings.Contains(string(data), "Veld\\\\Generated") {
		t.Fatal("should contain PSR-4 autoload entry")
	}
}

func TestPatchComposerJSON_Skipped(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"composer.json": `{
    "autoload": {
        "psr-4": {
            "Veld\\Generated\\": "generated/"
        }
    }
}`,
	})
	r := patchComposerJSON(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s", r.Action)
	}
}

// ── pubspec.yaml tests ───────────────────────────────────────────────────────

func TestPatchPubspecYAML_Patched(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"pubspec.yaml": "name: myapp\n\ndependencies:\n  flutter:\n    sdk: flutter\n",
	})
	r := patchPubspecYAML(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s", r.Action)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "pubspec.yaml"))
	if !strings.Contains(string(data), "veld_client") {
		t.Fatal("should contain veld_client dependency")
	}
}

func TestPatchPubspecYAML_Skipped(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"pubspec.yaml": "name: myapp\n\ndependencies:\n  veld_client:\n    path: ./generated/client\n",
	})
	r := patchPubspecYAML(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s", r.Action)
	}
}

// ── settings.gradle.kts tests ────────────────────────────────────────────────

func TestPatchGradleKts_Patched(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"settings.gradle.kts": "rootProject.name = \"myapp\"\n",
	})
	r := patchGradleKts(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s", r.Action)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "settings.gradle.kts"))
	content := string(data)
	if !strings.Contains(content, `include(":veld-client")`) {
		t.Fatal("should contain include directive")
	}
	if !strings.Contains(content, `projectDir`) {
		t.Fatal("should contain projectDir setting")
	}
}

func TestPatchGradleKts_Skipped(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"settings.gradle.kts": "rootProject.name = \"myapp\"\ninclude(\":veld-client\")\n",
	})
	r := patchGradleKts(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s", r.Action)
	}
}

// ── Run() orchestrator tests ─────────────────────────────────────────────────

func TestRun_NodeTS_Dedup(t *testing.T) {
	// Both node backend and typescript frontend need tsconfig — should only patch once
	dir := tmpProject(t, map[string]string{
		"package.json": `{ "name": "test", "dependencies": {} }`,
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	results := Run(dir, "node", "typescript", "generated")
	count := 0
	for _, r := range results {
		if r.File == "tsconfig.json" {
			count++
		}
	}
	if count != 1 {
		t.Fatalf("expected exactly 1 tsconfig result (dedup), got %d", count)
	}
	tsResult := findResult(results, "tsconfig.json")
	if tsResult == nil || tsResult.Action != "patched" {
		t.Fatalf("expected tsconfig.json patched, got %v", tsResult)
	}
}

func TestRun_PythonDart(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"pubspec.yaml": "name: myapp\n\ndependencies:\n  flutter:\n    sdk: flutter\n",
	})
	results := Run(dir, "python", "dart", "generated")
	if len(results) != 2 {
		t.Fatalf("expected 2 results, got %d: %+v", len(results), results)
	}
	veldPath := findResult(results, "veld_path.py")
	pub := findResult(results, "pubspec.yaml")
	if veldPath == nil || veldPath.Action != "patched" {
		t.Fatal("expected veld_path.py patched")
	}
	if pub == nil || pub.Action != "patched" {
		t.Fatal("expected pubspec.yaml patched")
	}
}

func TestRun_Swift_Manual(t *testing.T) {
	dir := t.TempDir()
	results := Run(dir, "node", "swift", "generated")
	swift := findResult(results, "Xcode")
	if swift == nil || swift.Action != "manual" {
		t.Fatal("expected Xcode manual result for swift")
	}
}

func TestRun_IdempotentSecondRun(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"package.json": `{ "name": "test", "dependencies": {} }`,
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	r1 := Run(dir, "node", "typescript", "generated")
	ts1 := findResult(r1, "tsconfig.json")
	if ts1 == nil || ts1.Action != "patched" {
		t.Fatalf("first run: expected tsconfig.json patched, got %v", ts1)
	}
	r2 := Run(dir, "node", "typescript", "generated")
	ts2 := findResult(r2, "tsconfig.json")
	if ts2 == nil || ts2.Action != "skipped" {
		t.Fatalf("second run: expected tsconfig.json skipped, got %v", ts2)
	}
}

// ── Update-in-place tests ────────────────────────────────────────────────────

func TestPatchTSConfig_UpdatePath(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "paths": {
      "@veld/*": ["./old-generated/*"]
    }
  }
}`,
	})
	r := patchTSConfig(dir, "new-output")
	if r.Action != "patched" {
		t.Fatalf("expected patched (update), got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "tsconfig.json"))
	content := string(data)
	if !strings.Contains(content, "./new-output/*") {
		t.Fatal("tsconfig.json should now contain ./new-output/*")
	}
	if strings.Contains(content, "old-generated") {
		t.Fatal("tsconfig.json should no longer contain old-generated")
	}
}

func TestPatchGoMod_UpdatePath(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"go.mod": "module myapp\n\ngo 1.21\n\nreplace veld/generated => ./old-path\n",
	})
	r := patchGoMod(dir, "new-output")
	if r.Action != "patched" {
		t.Fatalf("expected patched (update), got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "go.mod"))
	content := string(data)
	if !strings.Contains(content, "./new-output") {
		t.Fatal("go.mod should contain ./new-output")
	}
	if strings.Contains(content, "old-path") {
		t.Fatal("go.mod should no longer contain old-path")
	}
}

func TestPatchPubspecYAML_UpdatePath(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"pubspec.yaml": "name: myapp\ndependencies:\n  veld_client:\n    path: ./old-generated/client\n",
	})
	r := patchPubspecYAML(dir, "new-output")
	if r.Action != "patched" {
		t.Fatalf("expected patched (update), got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "pubspec.yaml"))
	content := string(data)
	if !strings.Contains(content, "./new-output/client") {
		t.Fatal("pubspec.yaml should contain ./new-output/client")
	}
	if strings.Contains(content, "old-generated") {
		t.Fatal("pubspec.yaml should no longer contain old-generated")
	}
}

func TestPatchGradleKts_UpdatePath(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"settings.gradle.kts": "rootProject.name = \"myapp\"\ninclude(\":veld-client\")\nproject(\":veld-client\").projectDir = file(\"old-path/client\")\n",
	})
	r := patchGradleKts(dir, "new-output")
	if r.Action != "patched" {
		t.Fatalf("expected patched (update), got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "settings.gradle.kts"))
	content := string(data)
	if !strings.Contains(content, "new-output/client") {
		t.Fatal("settings.gradle.kts should contain new-output/client")
	}
	if strings.Contains(content, "old-path") {
		t.Fatal("settings.gradle.kts should no longer contain old-path")
	}
}

func TestRun_UpdatePath_FullCycle(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"package.json": `{ "name": "test", "dependencies": {} }`,
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	// First run: patch with "generated"
	r1 := Run(dir, "node", "typescript", "generated")
	ts1 := findResult(r1, "tsconfig.json")
	if ts1 == nil || ts1.Action != "patched" {
		t.Fatalf("first run: expected tsconfig.json patched, got %v", ts1)
	}
	// Second run: same path → skipped
	r2 := Run(dir, "node", "typescript", "generated")
	ts2 := findResult(r2, "tsconfig.json")
	if ts2 == nil || ts2.Action != "skipped" {
		t.Fatalf("second run: expected tsconfig.json skipped, got %v", ts2)
	}
	// Third run: changed path → should update, not skip
	r3 := Run(dir, "node", "typescript", "output/v2")
	ts3 := findResult(r3, "tsconfig.json")
	if ts3 == nil || ts3.Action != "patched" {
		t.Fatalf("third run (path change): expected tsconfig.json patched, got %v", ts3)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "tsconfig.json"))
	if !strings.Contains(string(data), "output/v2") {
		t.Fatal("tsconfig should now point to output/v2")
	}
}

// ── BackendDir / FrontendDir tests ───────────────────────────────────────────

func TestRun_WithBackendDir(t *testing.T) {
	backendDir := tmpProject(t, map[string]string{
		"go.mod": "module myapp\n\ngo 1.21\n",
	})
	projectDir := t.TempDir()
	results := Run(projectDir, "go", "none", "generated", Options{BackendDir: backendDir})
	r := findResult(results, "go.mod")
	if r == nil || r.Action != "patched" {
		t.Fatal("expected go.mod patched in backend dir")
	}
	data, _ := os.ReadFile(filepath.Join(backendDir, "go.mod"))
	if !strings.Contains(string(data), "veld/generated") {
		t.Fatal("go.mod in backend dir should contain veld/generated")
	}
}

func TestRun_WithFrontendDir(t *testing.T) {
	frontendDir := tmpProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	projectDir := t.TempDir()
	results := Run(projectDir, "python", "react", "generated", Options{FrontendDir: frontendDir})
	ts := findResult(results, "tsconfig.json")
	if ts == nil || ts.Action != "patched" {
		t.Fatal("expected tsconfig.json patched in frontend dir")
	}
	data, _ := os.ReadFile(filepath.Join(frontendDir, "tsconfig.json"))
	if !strings.Contains(string(data), "@veld/*") {
		t.Fatal("tsconfig.json in frontend dir should contain @veld/*")
	}
}

func TestRun_WithSeparateDirs(t *testing.T) {
	backendDir := tmpProject(t, map[string]string{
		"go.mod": "module myapp\n\ngo 1.21\n",
	})
	frontendDir := tmpProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	projectDir := t.TempDir()
	results := Run(projectDir, "go", "react", "generated", Options{
		BackendDir:  backendDir,
		FrontendDir: frontendDir,
	})
	gomod := findResult(results, "go.mod")
	if gomod == nil || gomod.Action != "patched" {
		t.Fatal("expected go.mod patched")
	}
	ts := findResult(results, "tsconfig.json")
	if ts == nil || ts.Action != "patched" {
		t.Fatal("expected tsconfig.json patched")
	}
}

func TestRun_NodeReact_SeparateDirs(t *testing.T) {
	// Core bug fix: node backend + react frontend in SEPARATE directories
	// should patch BOTH tsconfig.json files.
	backendDir := tmpProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	frontendDir := tmpProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	projectDir := t.TempDir()
	results := Run(projectDir, "node", "react", "generated", Options{
		BackendDir:  backendDir,
		FrontendDir: frontendDir,
	})
	// Should have 2 tsconfig results — one for backend, one for frontend
	tsCount := 0
	for _, r := range results {
		if r.File == "tsconfig.json" {
			tsCount++
			if r.Action != "patched" {
				t.Fatalf("expected patched for tsconfig.json, got %s: %s", r.Action, r.Detail)
			}
		}
	}
	if tsCount != 2 {
		t.Fatalf("expected 2 tsconfig results for separate dirs, got %d", tsCount)
	}
	// Verify both files were actually patched
	backendData, _ := os.ReadFile(filepath.Join(backendDir, "tsconfig.json"))
	if !strings.Contains(string(backendData), "@veld/*") {
		t.Fatal("backend tsconfig.json should contain @veld/*")
	}
	frontendData, _ := os.ReadFile(filepath.Join(frontendDir, "tsconfig.json"))
	if !strings.Contains(string(frontendData), "@veld/*") {
		t.Fatal("frontend tsconfig.json should contain @veld/*")
	}
}

// ── patchNodePackageJSON tests ───────────────────────────────────────────────

func TestPatchNodePackageJSON_AddDep(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"package.json": `{
  "name": "my-app",
  "dependencies": {
    "express": "^4.18.0"
  }
}`,
	})
	r := patchNodePackageJSON(dir, "./generated", "@veld/generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "package.json"))
	content := string(data)
	if !strings.Contains(content, `"@veld/generated": "file:./generated"`) {
		t.Fatal("package.json should contain @veld/generated file: dependency")
	}
	if !strings.Contains(content, `"express"`) {
		t.Fatal("existing dependencies should be preserved")
	}
}

func TestPatchNodePackageJSON_NoDepsBlock(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"package.json": `{
  "name": "my-app",
  "private": true
}`,
	})
	r := patchNodePackageJSON(dir, "./generated", "@veld/generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "package.json"))
	content := string(data)
	if !strings.Contains(content, `"dependencies"`) {
		t.Fatal("should have created dependencies block")
	}
	if !strings.Contains(content, `"@veld/generated": "file:./generated"`) {
		t.Fatal("package.json should contain @veld/generated file: dependency")
	}
}

func TestPatchNodePackageJSON_Idempotent(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"package.json": `{
  "name": "my-app",
  "dependencies": {
    "@veld/generated": "file:./generated"
  }
}`,
	})
	r := patchNodePackageJSON(dir, "./generated", "@veld/generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped (already set), got %s: %s", r.Action, r.Detail)
	}
}

func TestPatchNodePackageJSON_UpdatePath(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"package.json": `{
  "name": "my-app",
  "dependencies": {
    "@veld/generated": "file:./old-generated"
  }
}`,
	})
	r := patchNodePackageJSON(dir, "./new-output", "@veld/generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched (update), got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "package.json"))
	content := string(data)
	if !strings.Contains(content, "file:./new-output") {
		t.Fatal("should contain new path")
	}
	if strings.Contains(content, "old-generated") {
		t.Fatal("should not contain old path")
	}
}

func TestPatchNodePackageJSON_NotFound(t *testing.T) {
	dir := t.TempDir() // no package.json
	r := patchNodePackageJSON(dir, "./generated", "@veld/generated")
	if r.Action != "not-found" {
		t.Fatalf("expected not-found, got %s", r.Action)
	}
}

func TestPatchNodePackageJSON_MultiplePackages(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"package.json": `{
  "name": "my-app",
  "dependencies": {
    "express": "^4.18.0"
  }
}`,
	})
	// Add root package
	r1 := patchNodePackageJSON(dir, "./generated", "@veld/generated")
	if r1.Action != "patched" {
		t.Fatalf("expected patched for root, got %s", r1.Action)
	}
	// Add client sub-package
	r2 := patchNodePackageJSON(dir, "./generated/client", "@veld/client")
	if r2.Action != "patched" {
		t.Fatalf("expected patched for client, got %s", r2.Action)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "package.json"))
	content := string(data)
	if !strings.Contains(content, `"@veld/generated": "file:./generated"`) {
		t.Fatal("should contain @veld/generated")
	}
	if !strings.Contains(content, `"@veld/client": "file:./generated/client"`) {
		t.Fatal("should contain @veld/client")
	}
}

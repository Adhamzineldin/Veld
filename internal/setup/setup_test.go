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

// ── conftest.py tests ────────────────────────────────────────────────────────

func TestPatchConftest_Created(t *testing.T) {
	dir := t.TempDir()
	r := patchConftest(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "conftest.py"))
	content := string(data)
	if !strings.Contains(content, "generated") {
		t.Fatal("conftest.py should contain generated path")
	}
	if !strings.Contains(content, "sys.path") {
		t.Fatal("conftest.py should contain sys.path insert")
	}
}

func TestPatchConftest_Skipped(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"conftest.py": "import os, sys  # noqa: E401\nsys.path.insert(0, os.path.join(os.path.dirname(__file__), \"generated\"))  # veld:generated-path\n",
	})
	r := patchConftest(dir, "generated")
	if r.Action != "skipped" {
		t.Fatalf("expected skipped, got %s: %s", r.Action, r.Detail)
	}
}

func TestPatchConftest_UpdatePath(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"conftest.py": "import os, sys  # noqa: E401\nsys.path.insert(0, os.path.join(os.path.dirname(__file__), \"old-output\"))  # veld:generated-path\n",
	})
	r := patchConftest(dir, "new-output")
	if r.Action != "patched" {
		t.Fatalf("expected patched (update), got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "conftest.py"))
	content := string(data)
	if !strings.Contains(content, "new-output") {
		t.Fatal("conftest.py should now contain new-output")
	}
	if strings.Contains(content, "old-output") {
		t.Fatal("conftest.py should no longer contain old-output")
	}
}

func TestPatchConftest_AppendToExisting(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"conftest.py": "# existing conftest\nimport pytest\n",
	})
	r := patchConftest(dir, "generated")
	if r.Action != "patched" {
		t.Fatalf("expected patched, got %s: %s", r.Action, r.Detail)
	}
	data, _ := os.ReadFile(filepath.Join(dir, "conftest.py"))
	content := string(data)
	if !strings.Contains(content, "import pytest") {
		t.Fatal("should preserve existing content")
	}
	if !strings.Contains(content, "sys.path") {
		t.Fatal("should add sys.path insert")
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
	if results[0].Action != "patched" {
		t.Fatalf("expected patched, got %s", results[0].Action)
	}
}

func TestRun_PythonDart(t *testing.T) {
	dir := tmpProject(t, map[string]string{
		"requirements.txt": "flask>=3.0\n",
		"pubspec.yaml":     "name: myapp\n\ndependencies:\n  flutter:\n    sdk: flutter\n",
	})
	results := Run(dir, "python", "dart", "generated")
	if len(results) != 3 {
		t.Fatalf("expected 3 results, got %d: %+v", len(results), results)
	}
	req := findResult(results, "requirements.txt")
	pub := findResult(results, "pubspec.yaml")
	conf := findResult(results, "conftest.py")
	if req == nil || req.Action != "patched" {
		t.Fatal("expected requirements.txt patched")
	}
	if conf == nil || conf.Action != "patched" {
		t.Fatal("expected conftest.py patched")
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
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	r1 := Run(dir, "node", "typescript", "generated")
	if r1[0].Action != "patched" {
		t.Fatalf("first run: expected patched, got %s", r1[0].Action)
	}
	r2 := Run(dir, "node", "typescript", "generated")
	if r2[0].Action != "skipped" {
		t.Fatalf("second run: expected skipped, got %s", r2[0].Action)
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
		"tsconfig.json": `{
  "compilerOptions": {
    "target": "ES2020"
  }
}`,
	})
	// First run: patch with "generated"
	r1 := Run(dir, "node", "typescript", "generated")
	if r1[0].Action != "patched" {
		t.Fatalf("first run: expected patched, got %s", r1[0].Action)
	}
	// Second run: same path → skipped
	r2 := Run(dir, "node", "typescript", "generated")
	if r2[0].Action != "skipped" {
		t.Fatalf("second run: expected skipped, got %s", r2[0].Action)
	}
	// Third run: changed path → should update, not skip
	r3 := Run(dir, "node", "typescript", "output/v2")
	if r3[0].Action != "patched" {
		t.Fatalf("third run (path change): expected patched, got %s: %s", r3[0].Action, r3[0].Detail)
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

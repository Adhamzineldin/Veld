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
	if len(results) != 2 {
		t.Fatalf("expected 2 results, got %d", len(results))
	}
	req := findResult(results, "requirements.txt")
	pub := findResult(results, "pubspec.yaml")
	if req == nil || req.Action != "patched" {
		t.Fatal("expected requirements.txt patched")
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

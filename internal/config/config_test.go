package config

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestFrontendAlias(t *testing.T) {
	tests := []struct {
		input, expected string
	}{
		{"react", "react"},
		{"react-hooks", "react"},
		{"hooks", "react"},
		{"ts", "typescript"},
		{"typescript", "typescript"},
		{"flutter", "dart"},
		{"none", "none"},
		{"python", "python"},
	}
	for _, tc := range tests {
		got := frontendAlias(tc.input)
		if got != tc.expected {
			t.Errorf("frontendAlias(%q) = %q, want %q", tc.input, got, tc.expected)
		}
	}
}

func TestBuildResolvedDefaults(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cfgPath, []byte(`{"input": "app.veld"}`), 0644); err != nil {
		t.Fatal(err)
	}
	// Write a dummy app.veld so the input path is valid
	if err := os.WriteFile(filepath.Join(dir, "veld", "app.veld"), []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	// Change to the temp dir so FindConfig picks up the file
	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if rc.Backend != "node-ts" {
		t.Errorf("expected default backend 'node-ts', got %q", rc.Backend)
	}
	if rc.Frontend != "typescript" {
		t.Errorf("expected default frontend 'typescript', got %q", rc.Frontend)
	}
}

func TestBuildResolvedWithAllFields(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	cfg := `{
		"input": "app.veld",
		"backend": "python",
		"frontend": "none",
		"out": "../output",
		"baseUrl": "/api/v1"
	}`
	if err := os.WriteFile(cfgPath, []byte(cfg), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "veld", "app.veld"), []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if rc.Backend != "python" {
		t.Errorf("expected backend 'python', got %q", rc.Backend)
	}
	if rc.Frontend != "none" {
		t.Errorf("expected frontend 'none', got %q", rc.Frontend)
	}
	if rc.BaseUrl != "/api/v1" {
		t.Errorf("expected baseUrl '/api/v1', got %q", rc.BaseUrl)
	}
}

func TestBuildResolvedFlagOverrides(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cfgPath, []byte(`{"input":"app.veld","backend":"node"}`), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "veld", "app.veld"), []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{
		Backend:    "python",
		BackendSet: true,
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if rc.Backend != "python" {
		t.Errorf("expected backend override 'python', got %q", rc.Backend)
	}
}

func TestBuildResolvedReactAlias(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cfgPath, []byte(`{"input":"app.veld","frontend":"react"}`), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "veld", "app.veld"), []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if rc.Frontend != "react" {
		t.Errorf("expected 'react' to resolve to 'react', got %q", rc.Frontend)
	}
}

func TestBuildResolvedNoInput(t *testing.T) {
	dir := t.TempDir()
	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	_, err := BuildResolved(FlagOverrides{})
	if err == nil {
		t.Fatal("expected error when no input file specified")
	}
}

func TestResolveInputFromArgs(t *testing.T) {
	path, err := ResolveInput([]string{"my/file.veld"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if path != "my/file.veld" {
		t.Errorf("expected 'my/file.veld', got %q", path)
	}
}

func TestBuildResolvedAliasesMerge(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	// Custom aliases: override "models" and add a new "auth" alias
	cfg := `{
		"input": "app.veld",
		"aliases": {
			"models": "custom/models",
			"auth": "services/auth"
		}
	}`
	if err := os.WriteFile(cfgPath, []byte(cfg), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "veld", "app.veld"), []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Custom alias should override default
	if rc.Aliases["models"] != "custom/models" {
		t.Errorf("expected aliases[models] = 'custom/models', got %q", rc.Aliases["models"])
	}
	// New alias should be added
	if rc.Aliases["auth"] != "services/auth" {
		t.Errorf("expected aliases[auth] = 'services/auth', got %q", rc.Aliases["auth"])
	}
	// Default aliases should still exist
	if rc.Aliases["modules"] != "modules" {
		t.Errorf("expected aliases[modules] = 'modules', got %q", rc.Aliases["modules"])
	}
	if rc.Aliases["shared"] != "shared" {
		t.Errorf("expected aliases[shared] = 'shared', got %q", rc.Aliases["shared"])
	}
}

func TestOutPathValidation(t *testing.T) {
	// Test that bare ".." is rejected as an out path
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cfgPath, []byte(`{"input": "app.veld", "out": ".."}`), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "veld", "app.veld"), []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	_, err := BuildResolved(FlagOverrides{})
	if err == nil {
		t.Fatal("expected error for out path '..'")
	}
	if !strings.Contains(err.Error(), "must end with a folder name") {
		t.Errorf("unexpected error message: %v", err)
	}
}

func TestOutPathWithFolderAllowed(t *testing.T) {
	// "../generated" should be allowed
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cfgPath, []byte(`{"input": "app.veld", "out": "../generated"}`), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "veld", "app.veld"), []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	_, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("../generated should be allowed, got: %v", err)
	}
}

func TestBackendDirectoryAlias(t *testing.T) {
	// "backendDirectory" should work as an alias for "backendDir"
	dir := t.TempDir()
	cfgDir := filepath.Join(dir, "veld")
	os.MkdirAll(cfgDir, 0755)
	os.MkdirAll(filepath.Join(dir, "backend"), 0755)
	os.MkdirAll(filepath.Join(dir, "frontend"), 0755)

	cfg := `{
		"input": "app.veld",
		"backend": "node",
		"frontend": "react",
		"out": "../generated",
		"backendDirectory": "../backend",
		"frontendDirectory": "../frontend"
	}`
	os.WriteFile(filepath.Join(cfgDir, "veld.config.json"), []byte(cfg), 0644)
	os.WriteFile(filepath.Join(cfgDir, "app.veld"), []byte(""), 0644)

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// BackendDir should resolve to dir/backend
	expectedBackend := filepath.Clean(filepath.Join(dir, "backend"))
	if rc.BackendDir != expectedBackend {
		t.Errorf("BackendDir = %q, want %q", rc.BackendDir, expectedBackend)
	}

	// FrontendDir should resolve to dir/frontend
	expectedFrontend := filepath.Clean(filepath.Join(dir, "frontend"))
	if rc.FrontendDir != expectedFrontend {
		t.Errorf("FrontendDir = %q, want %q", rc.FrontendDir, expectedFrontend)
	}
}

func TestBackendDirShortForm(t *testing.T) {
	// "backendDir" (short form) should also work
	dir := t.TempDir()
	cfgDir := filepath.Join(dir, "veld")
	os.MkdirAll(cfgDir, 0755)
	os.MkdirAll(filepath.Join(dir, "server"), 0755)

	cfg := `{
		"input": "app.veld",
		"backendDir": "../server"
	}`
	os.WriteFile(filepath.Join(cfgDir, "veld.config.json"), []byte(cfg), 0644)
	os.WriteFile(filepath.Join(cfgDir, "app.veld"), []byte(""), 0644)

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := filepath.Clean(filepath.Join(dir, "server"))
	if rc.BackendDir != expected {
		t.Errorf("BackendDir = %q, want %q", rc.BackendDir, expected)
	}
}

func TestBackendDirPriorityOverDirectory(t *testing.T) {
	// When both are set, backendDir takes priority over backendDirectory
	dir := t.TempDir()
	cfgDir := filepath.Join(dir, "veld")
	os.MkdirAll(cfgDir, 0755)

	cfg := `{
		"input": "app.veld",
		"backendDir": "../server",
		"backendDirectory": "../backend"
	}`
	os.WriteFile(filepath.Join(cfgDir, "veld.config.json"), []byte(cfg), 0644)
	os.WriteFile(filepath.Join(cfgDir, "app.veld"), []byte(""), 0644)

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// backendDir should win over backendDirectory
	expected := filepath.Clean(filepath.Join(dir, "server"))
	if rc.BackendDir != expected {
		t.Errorf("BackendDir = %q, want %q (backendDir should take priority)", rc.BackendDir, expected)
	}
}

func TestSplitOutputDirs(t *testing.T) {
	dir := t.TempDir()
	cfgDir := filepath.Join(dir, "veld")
	os.MkdirAll(cfgDir, 0755)

	cfg := `{
		"input": "app.veld",
		"backend": "node",
		"frontend": "react",
		"out": "../generated",
		"backendOut": "../backend/src/generated",
		"frontendOut": "../frontend/src/generated"
	}`
	os.WriteFile(filepath.Join(cfgDir, "veld.config.json"), []byte(cfg), 0644)
	os.WriteFile(filepath.Join(cfgDir, "app.veld"), []byte(""), 0644)

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Out should still be set (fallback)
	expectedOut := filepath.Clean(filepath.Join(dir, "generated"))
	if rc.Out != expectedOut {
		t.Errorf("Out = %q, want %q", rc.Out, expectedOut)
	}

	// BackendOut should resolve to backend/src/generated
	expectedBE := filepath.Clean(filepath.Join(dir, "backend", "src", "generated"))
	if rc.BackendOut != expectedBE {
		t.Errorf("BackendOut = %q, want %q", rc.BackendOut, expectedBE)
	}

	// FrontendOut should resolve to frontend/src/generated
	expectedFE := filepath.Clean(filepath.Join(dir, "frontend", "src", "generated"))
	if rc.FrontendOut != expectedFE {
		t.Errorf("FrontendOut = %q, want %q", rc.FrontendOut, expectedFE)
	}

	// SplitOutput should be true
	if !rc.SplitOutput() {
		t.Error("SplitOutput() should be true when backendOut != frontendOut")
	}

	// OutputDirs should return 2 dirs
	dirs := rc.OutputDirs()
	if len(dirs) != 2 {
		t.Errorf("OutputDirs() returned %d dirs, want 2", len(dirs))
	}
}

func TestSplitOutputFallsBackToOut(t *testing.T) {
	dir := t.TempDir()
	cfgDir := filepath.Join(dir, "veld")
	os.MkdirAll(cfgDir, 0755)

	// No backendOut/frontendOut — should fall back to "out"
	cfg := `{
		"input": "app.veld",
		"out": "../generated"
	}`
	os.WriteFile(filepath.Join(cfgDir, "veld.config.json"), []byte(cfg), 0644)
	os.WriteFile(filepath.Join(cfgDir, "app.veld"), []byte(""), 0644)

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if rc.BackendOut != rc.Out {
		t.Errorf("BackendOut = %q, should equal Out %q when not set", rc.BackendOut, rc.Out)
	}
	if rc.FrontendOut != rc.Out {
		t.Errorf("FrontendOut = %q, should equal Out %q when not set", rc.FrontendOut, rc.Out)
	}
	if rc.SplitOutput() {
		t.Error("SplitOutput() should be false when backendOut/frontendOut not set")
	}

	dirs := rc.OutputDirs()
	if len(dirs) != 1 {
		t.Errorf("OutputDirs() returned %d dirs, want 1", len(dirs))
	}
}

func TestSplitOutputFlagOverrides(t *testing.T) {
	dir := t.TempDir()
	cfgDir := filepath.Join(dir, "veld")
	os.MkdirAll(cfgDir, 0755)

	cfg := `{
		"input": "app.veld",
		"out": "../generated"
	}`
	os.WriteFile(filepath.Join(cfgDir, "veld.config.json"), []byte(cfg), 0644)
	os.WriteFile(filepath.Join(cfgDir, "app.veld"), []byte(""), 0644)

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{
		BackendOut:     "../be-out",
		BackendOutSet:  true,
		FrontendOut:    "../fe-out",
		FrontendOutSet: true,
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expectedBE := filepath.Clean(filepath.Join(cfgDir, "../be-out"))
	if rc.BackendOut != expectedBE {
		t.Errorf("BackendOut = %q, want %q", rc.BackendOut, expectedBE)
	}
	expectedFE := filepath.Clean(filepath.Join(cfgDir, "../fe-out"))
	if rc.FrontendOut != expectedFE {
		t.Errorf("FrontendOut = %q, want %q", rc.FrontendOut, expectedFE)
	}
}

func TestSplitOutputPathValidation(t *testing.T) {
	dir := t.TempDir()
	cfgDir := filepath.Join(dir, "veld")
	os.MkdirAll(cfgDir, 0755)

	cfg := `{
		"input": "app.veld",
		"backendOut": ".."
	}`
	os.WriteFile(filepath.Join(cfgDir, "veld.config.json"), []byte(cfg), 0644)
	os.WriteFile(filepath.Join(cfgDir, "app.veld"), []byte(""), 0644)

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	_, err := BuildResolved(FlagOverrides{})
	if err == nil {
		t.Fatal("expected error for backendOut path '..'")
	}
	if !strings.Contains(err.Error(), "backendOut") {
		t.Errorf("error should mention 'backendOut', got: %v", err)
	}
}

func TestNormalizeNestedBackend(t *testing.T) {
	cfg := RawConfig{
		Input: "app.veld",
		BackendCfg: &BackendConfig{
			Target:    "python",
			Framework: "flask",
			Out:       "../backend/generated",
		},
		FrontendCfg: &FrontendConfig{
			Target: "react",
			Out:    "../frontend/generated",
		},
		Hooks: &HooksConfig{
			PostGenerate: "npm run format",
		},
	}
	cfg.normalize()

	if cfg.Backend != "python" {
		t.Errorf("Backend = %q, want %q", cfg.Backend, "python")
	}
	if cfg.BackendFramework != "flask" {
		t.Errorf("BackendFramework = %q, want %q", cfg.BackendFramework, "flask")
	}
	if cfg.BackendOut != "../backend/generated" {
		t.Errorf("BackendOut = %q, want %q", cfg.BackendOut, "../backend/generated")
	}
	if cfg.Frontend != "react" {
		t.Errorf("Frontend = %q, want %q", cfg.Frontend, "react")
	}
	if cfg.FrontendOut != "../frontend/generated" {
		t.Errorf("FrontendOut = %q, want %q", cfg.FrontendOut, "../frontend/generated")
	}
	if cfg.PostGenerate != "npm run format" {
		t.Errorf("PostGenerate = %q, want %q", cfg.PostGenerate, "npm run format")
	}
}

func TestNormalizeFlatStillWorks(t *testing.T) {
	cfg := RawConfig{
		Input:            "app.veld",
		Backend:          "node",
		Frontend:         "typescript",
		BackendFramework: "express",
		PostGenerate:     "echo done",
	}
	cfg.normalize()

	if cfg.Backend != "node" {
		t.Errorf("Backend = %q, want %q", cfg.Backend, "node")
	}
	if cfg.BackendFramework != "express" {
		t.Errorf("BackendFramework = %q, want %q", cfg.BackendFramework, "express")
	}
	if cfg.PostGenerate != "echo done" {
		t.Errorf("PostGenerate = %q, want %q", cfg.PostGenerate, "echo done")
	}
}

func TestNormalizeNestedDoesNotOverrideFlat(t *testing.T) {
	// If both flat and nested are set, flat wins (user explicitly set it).
	cfg := RawConfig{
		Input:   "app.veld",
		Backend: "go",
		BackendCfg: &BackendConfig{
			Target: "python",
		},
	}
	cfg.normalize()

	// Flat "go" was already set, nested "python" should NOT override.
	if cfg.Backend != "go" {
		t.Errorf("Backend = %q, want %q (flat should not be overridden)", cfg.Backend, "go")
	}
}

func TestNormalizeWorkspaceEntry(t *testing.T) {
	cfg := RawConfig{
		Workspace: []WorkspaceEntry{
			{
				Name: "iam",
				BackendCfg: &BackendConfig{
					Target: "node",
					Out:    "../backend/iam/generated",
				},
			},
			{
				Name: "frontend",
				FrontendCfg: &FrontendConfig{
					Target: "react",
					Out:    "../frontend/generated",
				},
			},
		},
	}
	cfg.normalize()

	if cfg.Workspace[0].Backend != "node" {
		t.Errorf("ws[0].Backend = %q, want %q", cfg.Workspace[0].Backend, "node")
	}
	if cfg.Workspace[0].Out != "../backend/iam/generated" {
		t.Errorf("ws[0].Out = %q, want %q", cfg.Workspace[0].Out, "../backend/iam/generated")
	}
	if cfg.Workspace[1].Frontend != "react" {
		t.Errorf("ws[1].Frontend = %q, want %q", cfg.Workspace[1].Frontend, "react")
	}
}

func TestNormalizeValidateFromNested(t *testing.T) {
	v := true
	cfg := RawConfig{
		Input: "app.veld",
		BackendCfg: &BackendConfig{
			Target:   "node",
			Validate: &v,
		},
	}
	cfg.normalize()

	if !cfg.Validate {
		t.Error("Validate should be true from nested config")
	}
}

func TestEffectiveBackendDirNested(t *testing.T) {
	cfg := RawConfig{
		BackendCfg: &BackendConfig{Dir: "../backend"},
		BackendDir: "../old-backend",
	}
	if got := cfg.effectiveBackendDir(); got != "../backend" {
		t.Errorf("effectiveBackendDir() = %q, want %q (nested should win)", got, "../backend")
	}
}

func TestEffectiveFrontendDirNested(t *testing.T) {
	cfg := RawConfig{
		FrontendCfg: &FrontendConfig{Dir: "../frontend"},
		FrontendDir: "../old-frontend",
	}
	if got := cfg.effectiveFrontendDir(); got != "../frontend" {
		t.Errorf("effectiveFrontendDir() = %q, want %q (nested should win)", got, "../frontend")
	}
}

func TestEffectivePostGenerateHooks(t *testing.T) {
	cfg := RawConfig{
		Hooks:        &HooksConfig{PostGenerate: "new-cmd"},
		PostGenerate: "old-cmd",
	}
	if got := cfg.effectivePostGenerate(); got != "new-cmd" {
		t.Errorf("effectivePostGenerate() = %q, want %q (hooks should win)", got, "new-cmd")
	}
}

func TestSchemaFieldIgnored(t *testing.T) {
	dir := t.TempDir()
	cfgDir := filepath.Join(dir, "veld")
	os.MkdirAll(cfgDir, 0755)

	cfg := `{
		"$schema": "https://veld.dev/schemas/veld.config.schema.json",
		"input": "app.veld",
		"backend": "node"
	}`
	os.WriteFile(filepath.Join(cfgDir, "veld.config.json"), []byte(cfg), 0644)
	os.WriteFile(filepath.Join(cfgDir, "app.veld"), []byte(""), 0644)

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if rc.Backend != "node-ts" {
		t.Errorf("Backend = %q, want %q", rc.Backend, "node-ts")
	}
}

func TestNestedConfigEndToEnd(t *testing.T) {
	dir := t.TempDir()
	cfgDir := filepath.Join(dir, "veld")
	os.MkdirAll(cfgDir, 0755)

	cfg := `{
		"$schema": "https://veld.dev/schemas/veld.config.schema.json",
		"input": "app.veld",
		"description": "My API",
		"backendConfig": {
			"target": "python",
			"framework": "flask",
			"out": "../backend/generated",
			"dir": "../backend"
		},
		"frontendConfig": {
			"target": "vue",
			"out": "../frontend/generated"
		},
		"hooks": {
			"postGenerate": "echo done"
		},
		"baseUrl": "/api"
	}`
	os.WriteFile(filepath.Join(cfgDir, "veld.config.json"), []byte(cfg), 0644)
	os.WriteFile(filepath.Join(cfgDir, "app.veld"), []byte(""), 0644)

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if rc.Backend != "python" {
		t.Errorf("Backend = %q, want %q", rc.Backend, "python")
	}
	if rc.BackendFramework != "flask" {
		t.Errorf("BackendFramework = %q, want %q", rc.BackendFramework, "flask")
	}
	if rc.Frontend != "vue" {
		t.Errorf("Frontend = %q, want %q", rc.Frontend, "vue")
	}
	if rc.PostGenerate != "echo done" {
		t.Errorf("PostGenerate = %q, want %q", rc.PostGenerate, "echo done")
	}
	if rc.BaseUrl != "/api" {
		t.Errorf("BaseUrl = %q, want %q", rc.BaseUrl, "/api")
	}
}

func TestNormalizeWildcardConsumes(t *testing.T) {
	cfg := RawConfig{
		Workspace: []WorkspaceEntry{
			{Name: "iam"},
			{Name: "accounts"},
			{Name: "transactions"},
			{Name: "frontend", Consumes: []string{"*"}},
		},
	}
	cfg.normalize()

	// "*" should be expanded to all other service names
	if len(cfg.Workspace[3].Consumes) != 3 {
		t.Fatalf("expected 3 consumed services, got %d: %v", len(cfg.Workspace[3].Consumes), cfg.Workspace[3].Consumes)
	}
	expected := map[string]bool{"iam": true, "accounts": true, "transactions": true}
	for _, c := range cfg.Workspace[3].Consumes {
		if !expected[c] {
			t.Errorf("unexpected consumed service %q", c)
		}
	}
	// "frontend" should NOT be in its own consumes list
	for _, c := range cfg.Workspace[3].Consumes {
		if c == "frontend" {
			t.Error("frontend should not consume itself")
		}
	}
}

func TestNormalizeWildcardConsumesNotExpanded(t *testing.T) {
	// Normal consumes without "*" should not be changed
	cfg := RawConfig{
		Workspace: []WorkspaceEntry{
			{Name: "iam"},
			{Name: "accounts", Consumes: []string{"iam"}},
		},
	}
	cfg.normalize()

	if len(cfg.Workspace[1].Consumes) != 1 || cfg.Workspace[1].Consumes[0] != "iam" {
		t.Errorf("expected [iam], got %v", cfg.Workspace[1].Consumes)
	}
}

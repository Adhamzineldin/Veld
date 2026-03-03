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
	if rc.Backend != "node" {
		t.Errorf("expected default backend 'node', got %q", rc.Backend)
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

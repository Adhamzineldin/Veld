package config

import (
	"os"
	"path/filepath"
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

func TestValidationDefaultTrue(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	// No "validation" key → should default to true
	if err := os.WriteFile(cfgPath, []byte(`{"input":"app.veld"}`), 0644); err != nil {
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
	if !rc.Validation {
		t.Error("expected Validation to default to true")
	}
}

func TestValidationExplicitFalse(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cfgPath, []byte(`{"input":"app.veld","validation":false}`), 0644); err != nil {
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
	if rc.Validation {
		t.Error("expected Validation to be false when set in config")
	}
}

func TestValidationFlagOverridesConfig(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "veld", "veld.config.json")
	if err := os.MkdirAll(filepath.Dir(cfgPath), 0755); err != nil {
		t.Fatal(err)
	}
	// Config says validation: true, flag says --no-validation
	if err := os.WriteFile(cfgPath, []byte(`{"input":"app.veld","validation":true}`), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "veld", "app.veld"), []byte(""), 0644); err != nil {
		t.Fatal(err)
	}

	orig, _ := os.Getwd()
	defer os.Chdir(orig)
	os.Chdir(dir)

	rc, err := BuildResolved(FlagOverrides{
		NoValidation:    true,
		NoValidationSet: true,
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if rc.Validation {
		t.Error("expected --no-validation flag to override config validation=true")
	}
}

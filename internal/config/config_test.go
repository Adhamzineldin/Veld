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
		{"react", "typescript"},
		{"typescript", "typescript"},
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
	if rc.Frontend != "typescript" {
		t.Errorf("expected 'react' to alias to 'typescript', got %q", rc.Frontend)
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

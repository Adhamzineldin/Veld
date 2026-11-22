package loader

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseSingleFile(t *testing.T) {
	dir := t.TempDir()
	veldFile := filepath.Join(dir, "app.veld")
	content := `model User {
  id: string
  name: string
}

module Auth {
  action Login {
    method: POST
    path: /auth/login
    input: User
    output: User
  }
}`
	if err := os.WriteFile(veldFile, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	a, files, err := Parse(veldFile)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(a.Models) != 1 {
		t.Errorf("expected 1 model, got %d", len(a.Models))
	}
	if len(a.Modules) != 1 {
		t.Errorf("expected 1 module, got %d", len(a.Modules))
	}
	if len(files) != 1 {
		t.Errorf("expected 1 file, got %d", len(files))
	}
}

func TestParseWithImports(t *testing.T) {
	dir := t.TempDir()

	// Create models/user.veld
	modelsDir := filepath.Join(dir, "models")
	os.MkdirAll(modelsDir, 0755)
	os.WriteFile(filepath.Join(modelsDir, "user.veld"), []byte(`model User {
  id: string
  name: string
}`), 0644)

	// Create modules/auth.veld
	modulesDir := filepath.Join(dir, "modules")
	os.MkdirAll(modulesDir, 0755)
	os.WriteFile(filepath.Join(modulesDir, "auth.veld"), []byte(`module Auth {
  action Login {
    method: POST
    path: /auth/login
    input: User
    output: User
  }
}`), 0644)

	// Create app.veld with imports
	os.WriteFile(filepath.Join(dir, "app.veld"), []byte(`import "models/user.veld"
import "modules/auth.veld"
`), 0644)

	a, files, err := Parse(filepath.Join(dir, "app.veld"))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(a.Models) != 1 {
		t.Errorf("expected 1 model, got %d", len(a.Models))
	}
	if len(a.Modules) != 1 {
		t.Errorf("expected 1 module, got %d", len(a.Modules))
	}
	if len(files) != 3 {
		t.Errorf("expected 3 files loaded, got %d: %v", len(files), files)
	}
}

func TestParseCircularImport(t *testing.T) {
	dir := t.TempDir()

	// a.veld imports b.veld, b.veld imports a.veld
	os.WriteFile(filepath.Join(dir, "a.veld"), []byte(`import "b.veld"
model A { id: string }`), 0644)
	os.WriteFile(filepath.Join(dir, "b.veld"), []byte(`import "a.veld"
model B { id: string }`), 0644)

	a, _, err := Parse(filepath.Join(dir, "a.veld"))
	if err != nil {
		t.Fatalf("circular import should not error, got: %v", err)
	}
	if len(a.Models) != 2 {
		t.Errorf("expected 2 models (A and B), got %d", len(a.Models))
	}
}

func TestParseSourceFileTracking(t *testing.T) {
	dir := t.TempDir()
	modelsDir := filepath.Join(dir, "models")
	os.MkdirAll(modelsDir, 0755)
	os.WriteFile(filepath.Join(modelsDir, "user.veld"), []byte(`model User { id: string }`), 0644)
	os.WriteFile(filepath.Join(dir, "app.veld"), []byte(`import "models/user.veld"
model Config { key: string }`), 0644)

	a, _, err := Parse(filepath.Join(dir, "app.veld"))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Find each model and verify its SourceFile
	for _, m := range a.Models {
		if m.SourceFile == "" {
			t.Errorf("model %q has empty SourceFile", m.Name)
		}
		if m.Name == "User" && !filepath.IsAbs(m.SourceFile) {
			t.Errorf("expected absolute SourceFile for User, got %q", m.SourceFile)
		}
	}
}

func TestParseMissingFile(t *testing.T) {
	_, _, err := Parse("/nonexistent/file.veld")
	if err == nil {
		t.Fatal("expected error for missing file")
	}
}

func TestParseMissingImport(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "app.veld"), []byte(`import "missing.veld"`), 0644)

	_, _, err := Parse(filepath.Join(dir, "app.veld"))
	if err == nil {
		t.Fatal("expected error for missing import")
	}
}

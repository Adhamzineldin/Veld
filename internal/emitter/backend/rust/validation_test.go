package rustbackend_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/emitter"
	rustbackend "github.com/Adhamzineldin/Veld/internal/emitter/backend/rust"
)

func TestRustValidation(t *testing.T) {
	e := rustbackend.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, "src", "validation.rs")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("expected validation.rs to exist")
	}
	data, _ := os.ReadFile(path)
	content := string(data)
	for _, needle := range []string{"ValidationError", "validate_struct", "validator::Validate"} {
		if !strings.Contains(content, needle) {
			t.Errorf("validation.rs missing %q", needle)
		}
	}
}

func TestRustValidationSkipped(t *testing.T) {
	e := rustbackend.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: false}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, "src", "validation.rs")
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Error("expected validation.rs to NOT exist when validation is disabled")
	}
}

func TestRustErrorTypes(t *testing.T) {
	e := rustbackend.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, "src", "errors.rs")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("expected errors.rs to exist")
	}
	data, _ := os.ReadFile(path)
	content := string(data)
	for _, needle := range []string{"AppError", "ApiErrorResponse", "IntoResponse", "BadRequest", "NotFound"} {
		if !strings.Contains(content, needle) {
			t.Errorf("errors.rs missing %q", needle)
		}
	}
}

func TestRustCargoValidatorDep(t *testing.T) {
	e := rustbackend.New()

	t.Run("enabled", func(t *testing.T) {
		outDir := t.TempDir()
		if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
			t.Fatalf("Emit: %v", err)
		}
		data, _ := os.ReadFile(filepath.Join(outDir, "Cargo.toml"))
		if !strings.Contains(string(data), "validator") {
			t.Error("Cargo.toml should include validator dep when enabled")
		}
	})

	t.Run("disabled", func(t *testing.T) {
		outDir := t.TempDir()
		if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: false}); err != nil {
			t.Fatalf("Emit: %v", err)
		}
		data, _ := os.ReadFile(filepath.Join(outDir, "Cargo.toml"))
		if strings.Contains(string(data), "validator") {
			t.Error("Cargo.toml should NOT include validator dep when disabled")
		}
	})
}

func TestRustLibRsValidationModule(t *testing.T) {
	e := rustbackend.New()

	t.Run("enabled", func(t *testing.T) {
		outDir := t.TempDir()
		if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
			t.Fatalf("Emit: %v", err)
		}
		data, _ := os.ReadFile(filepath.Join(outDir, "src", "lib.rs"))
		if !strings.Contains(string(data), "pub mod validation;") {
			t.Error("lib.rs should include validation module when enabled")
		}
	})

	t.Run("disabled", func(t *testing.T) {
		outDir := t.TempDir()
		if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: false}); err != nil {
			t.Fatalf("Emit: %v", err)
		}
		data, _ := os.ReadFile(filepath.Join(outDir, "src", "lib.rs"))
		if strings.Contains(string(data), "pub mod validation;") {
			t.Error("lib.rs should NOT include validation module when disabled")
		}
	})
}

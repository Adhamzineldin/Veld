package gobackend_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/emitter"
	gobackend "github.com/Adhamzineldin/Veld/internal/emitter/backend/go"
)

func TestGoValidationGenerated(t *testing.T) {
	e := gobackend.New()
	outDir := t.TempDir()
	a := minimalAST()

	opts := emitter.EmitOptions{Validation: true}
	if err := e.Emit(a, outDir, opts); err != nil {
		t.Fatalf("Emit() error: %v", err)
	}

	valPath := filepath.Join(outDir, "internal", "models", "validation.go")
	if _, err := os.Stat(valPath); os.IsNotExist(err) {
		t.Fatal("expected validation.go to exist when validation is enabled")
	}

	data, err := os.ReadFile(valPath)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	content := string(data)

	checks := []string{
		"package models",
		"go-playground/validator",
		"ValidateStruct",
		"ValidationError",
		"FormatValidationErrors",
	}
	for _, needle := range checks {
		if !strings.Contains(content, needle) {
			t.Errorf("validation.go missing %q", needle)
		}
	}
}

func TestGoValidationSkippedWhenDisabled(t *testing.T) {
	e := gobackend.New()
	outDir := t.TempDir()
	a := minimalAST()

	opts := emitter.EmitOptions{Validation: false}
	if err := e.Emit(a, outDir, opts); err != nil {
		t.Fatalf("Emit() error: %v", err)
	}

	valPath := filepath.Join(outDir, "internal", "models", "validation.go")
	if _, err := os.Stat(valPath); !os.IsNotExist(err) {
		t.Error("expected validation.go to NOT exist when validation is disabled")
	}
}

func TestGoValidateTagsPresent(t *testing.T) {
	e := gobackend.New()
	outDir := t.TempDir()
	a := minimalAST()

	opts := emitter.EmitOptions{Validation: true}
	if err := e.Emit(a, outDir, opts); err != nil {
		t.Fatalf("Emit() error: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "internal", "models", "types.go"))
	content := string(data)

	if !strings.Contains(content, `validate:"`) {
		t.Error("types.go should contain validate tags when validation is enabled")
	}
}

func TestGoValidateTagsAbsent(t *testing.T) {
	e := gobackend.New()
	outDir := t.TempDir()
	a := minimalAST()

	opts := emitter.EmitOptions{Validation: false}
	if err := e.Emit(a, outDir, opts); err != nil {
		t.Fatalf("Emit() error: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "internal", "models", "types.go"))
	content := string(data)

	if strings.Contains(content, `validate:"`) {
		t.Error("types.go should NOT contain validate tags when validation is disabled")
	}
}

func TestGoModValidatorDep(t *testing.T) {
	e := gobackend.New()

	t.Run("enabled", func(t *testing.T) {
		outDir := t.TempDir()
		if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
			t.Fatalf("Emit: %v", err)
		}
		data, _ := os.ReadFile(filepath.Join(outDir, "go.mod"))
		if !strings.Contains(string(data), "go-playground/validator") {
			t.Error("go.mod should include validator dependency when validation is enabled")
		}
	})

	t.Run("disabled", func(t *testing.T) {
		outDir := t.TempDir()
		if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: false}); err != nil {
			t.Fatalf("Emit: %v", err)
		}
		data, _ := os.ReadFile(filepath.Join(outDir, "go.mod"))
		if strings.Contains(string(data), "validator") {
			t.Error("go.mod should NOT include validator dependency when validation is disabled")
		}
	})
}

func TestGoEmailFieldGetsEmailValidation(t *testing.T) {
	e := gobackend.New()
	outDir := t.TempDir()
	a := minimalAST() // has "email" field

	if err := e.Emit(a, outDir, emitter.EmitOptions{Validation: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "internal", "models", "types.go"))
	content := string(data)

	if !strings.Contains(content, "email") {
		t.Error("types.go should contain email validation for email fields")
	}
}

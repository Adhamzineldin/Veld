package java_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/backend/java"
)

func TestJavaValidationHelper(t *testing.T) {
	e := java.New()
	outDir := t.TempDir()

	opts := emitter.EmitOptions{Validation: true}
	if err := e.Emit(minimalAST(), outDir, opts); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, "models", "ValidationHelper.java")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("expected ValidationHelper.java to exist")
	}

	data, _ := os.ReadFile(path)
	content := string(data)
	for _, needle := range []string{"ValidationHelper", "jakarta.validation", "validate", "ValidationError"} {
		if !strings.Contains(content, needle) {
			t.Errorf("ValidationHelper.java missing %q", needle)
		}
	}
}

func TestJavaValidationSkipped(t *testing.T) {
	e := java.New()
	outDir := t.TempDir()

	opts := emitter.EmitOptions{Validation: false}
	if err := e.Emit(minimalAST(), outDir, opts); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, "models", "ValidationHelper.java")
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Error("expected ValidationHelper.java to NOT exist when validation is disabled")
	}
}

func TestJavaErrorHandler(t *testing.T) {
	e := java.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	// GlobalExceptionHandler
	ehPath := filepath.Join(outDir, "controllers", "GlobalExceptionHandler.java")
	if _, err := os.Stat(ehPath); os.IsNotExist(err) {
		t.Fatal("expected GlobalExceptionHandler.java to exist")
	}
	data, _ := os.ReadFile(ehPath)
	content := string(data)
	for _, needle := range []string{"@ControllerAdvice", "@ExceptionHandler", "handleBadRequest", "handleAll"} {
		if !strings.Contains(content, needle) {
			t.Errorf("GlobalExceptionHandler.java missing %q", needle)
		}
	}

	// ApiErrorResponse
	arPath := filepath.Join(outDir, "models", "ApiErrorResponse.java")
	if _, err := os.Stat(arPath); os.IsNotExist(err) {
		t.Fatal("expected ApiErrorResponse.java to exist")
	}
	data2, _ := os.ReadFile(arPath)
	if !strings.Contains(string(data2), "ApiErrorResponse") {
		t.Error("ApiErrorResponse.java missing class definition")
	}
}

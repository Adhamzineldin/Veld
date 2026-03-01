package csharp_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/backend/csharp"
)

func TestCSharpValidationHelper(t *testing.T) {
	e := csharp.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, "Models", "ValidationHelper.cs")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("expected ValidationHelper.cs to exist")
	}
	data, _ := os.ReadFile(path)
	content := string(data)
	for _, needle := range []string{"ValidationHelper", "DataAnnotations", "Validate", "ValidateOrThrow"} {
		if !strings.Contains(content, needle) {
			t.Errorf("ValidationHelper.cs missing %q", needle)
		}
	}
}

func TestCSharpValidationSkipped(t *testing.T) {
	e := csharp.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: false}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, "Models", "ValidationHelper.cs")
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Error("expected ValidationHelper.cs to NOT exist when validation is disabled")
	}
}

func TestCSharpErrorHandling(t *testing.T) {
	e := csharp.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	mwPath := filepath.Join(outDir, "Middleware", "ErrorHandlingMiddleware.cs")
	if _, err := os.Stat(mwPath); os.IsNotExist(err) {
		t.Fatal("expected ErrorHandlingMiddleware.cs to exist")
	}
	data, _ := os.ReadFile(mwPath)
	content := string(data)
	for _, needle := range []string{"ErrorHandlingMiddleware", "InvokeAsync", "ApiErrorResponse"} {
		if !strings.Contains(content, needle) {
			t.Errorf("ErrorHandlingMiddleware.cs missing %q", needle)
		}
	}

	arPath := filepath.Join(outDir, "Models", "ApiErrorResponse.cs")
	if _, err := os.Stat(arPath); os.IsNotExist(err) {
		t.Fatal("expected ApiErrorResponse.cs to exist")
	}
}

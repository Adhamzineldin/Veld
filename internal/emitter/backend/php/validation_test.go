package php_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/backend/php"
)

func TestPhpValidationRules(t *testing.T) {
	e := php.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	// Base FormRequest
	basePath := filepath.Join(outDir, "app", "Http", "Requests", "VeldFormRequest.php")
	if _, err := os.Stat(basePath); os.IsNotExist(err) {
		t.Fatal("expected VeldFormRequest.php to exist")
	}
	data, _ := os.ReadFile(basePath)
	if !strings.Contains(string(data), "VeldFormRequest") {
		t.Error("VeldFormRequest.php missing class definition")
	}

	// Per-input model form request (LoginInput is used as action input)
	frPath := filepath.Join(outDir, "app", "Http", "Requests", "LoginInputRequest.php")
	if _, err := os.Stat(frPath); os.IsNotExist(err) {
		t.Fatal("expected LoginInputRequest.php to exist")
	}
	data2, _ := os.ReadFile(frPath)
	content := string(data2)
	for _, needle := range []string{"rules()", "required", "email", "string"} {
		if !strings.Contains(content, needle) {
			t.Errorf("LoginInputRequest.php missing %q", needle)
		}
	}
}

func TestPhpValidationSkipped(t *testing.T) {
	e := php.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: false}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	reqDir := filepath.Join(outDir, "app", "Http", "Requests")
	if _, err := os.Stat(reqDir); !os.IsNotExist(err) {
		// If directory exists, it should be empty or not contain form requests
		entries, _ := os.ReadDir(reqDir)
		if len(entries) > 0 {
			t.Error("expected no form request files when validation is disabled")
		}
	}
}

func TestPhpErrorHandler(t *testing.T) {
	e := php.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{Validation: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, "app", "Exceptions", "VeldExceptionHandler.php")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("expected VeldExceptionHandler.php to exist")
	}
	data, _ := os.ReadFile(path)
	content := string(data)
	for _, needle := range []string{"VeldExceptionHandler", "ValidationException", "render"} {
		if !strings.Contains(content, needle) {
			t.Errorf("VeldExceptionHandler.php missing %q", needle)
		}
	}
}

package openapi_test

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/generators/openapi"
)

func minimalAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{Name: "LoginInput", Fields: []ast.Field{
				{Name: "email", Type: "string"},
				{Name: "password", Type: "string"},
			}},
			{Name: "User", Fields: []ast.Field{
				{Name: "id", Type: "uuid"},
				{Name: "email", Type: "string"},
				{Name: "name", Type: "string", Optional: true},
			}},
		},
		Enums: []ast.Enum{
			{Name: "Role", Values: []string{"admin", "user"}},
		},
		Modules: []ast.Module{
			{Name: "Auth", Prefix: "/api", Actions: []ast.Action{
				{Name: "Login", Method: "POST", Path: "/auth/login", Input: "LoginInput", Output: "User"},
				{Name: "Me", Method: "GET", Path: "/auth/me", Output: "User"},
				{Name: "ListUsers", Method: "GET", Path: "/users", Output: "User", OutputArray: true},
			}},
		},
	}
}

func TestOpenAPIEmitCreatesFile(t *testing.T) {
	e := openapi.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	path := filepath.Join(outDir, "openapi.json")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatal("expected openapi.json to exist")
	}
}

func TestOpenAPIDryRun(t *testing.T) {
	e := openapi.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}

func TestOpenAPIValidJSON(t *testing.T) {
	e := openapi.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "openapi.json"))
	var spec map[string]interface{}
	if err := json.Unmarshal(data, &spec); err != nil {
		t.Fatalf("openapi.json is not valid JSON: %v", err)
	}

	if spec["openapi"] != "3.0.3" {
		t.Errorf("expected openapi version 3.0.3, got %v", spec["openapi"])
	}
}

func TestOpenAPIContainsPaths(t *testing.T) {
	e := openapi.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "openapi.json"))
	content := string(data)

	for _, needle := range []string{
		"/api/auth/login",
		"/api/auth/me",
		"/api/users",
		"LoginInput",
		"User",
		"Role",
		"ErrorResponse",
		"#/components/schemas/",
	} {
		if !strings.Contains(content, needle) {
			t.Errorf("openapi.json missing %q", needle)
		}
	}
}

func TestOpenAPISchemaRefs(t *testing.T) {
	spec := openapi.BuildSpec(minimalAST())
	components := spec["components"].(map[string]interface{})
	schemas := components["schemas"].(map[string]interface{})

	if _, ok := schemas["User"]; !ok {
		t.Error("schemas should contain User")
	}
	if _, ok := schemas["LoginInput"]; !ok {
		t.Error("schemas should contain LoginInput")
	}
	if _, ok := schemas["Role"]; !ok {
		t.Error("schemas should contain Role enum")
	}
	if _, ok := schemas["ErrorResponse"]; !ok {
		t.Error("schemas should contain ErrorResponse")
	}
}

func TestOpenAPIArrayOutput(t *testing.T) {
	e := openapi.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "openapi.json"))
	content := string(data)

	// ListUsers returns User[], should have "type": "array"
	if !strings.Contains(content, `"type": "array"`) {
		t.Error("expected array type in response for ListUsers")
	}
}

func TestOpenAPISummary(t *testing.T) {
	e := openapi.New()
	lines := e.Summary(nil)
	if len(lines) == 0 {
		t.Fatal("Summary should return at least one line")
	}
	if !strings.Contains(lines[0].Files, "openapi.json") {
		t.Errorf("Summary should mention openapi.json, got %q", lines[0].Files)
	}
}

package lint

import (
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func TestLintClean(t *testing.T) {
	a := ast.AST{
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}},
		},
		Modules: []ast.Module{{
			Name:        "Users",
			Description: "User management",
			Actions: []ast.Action{{
				Name:        "Get",
				Description: "Get a user",
				Method:      "GET",
				Path:        "/",
				Output:      "User",
				Middleware:  []string{},
			}},
		}},
	}
	issues := Lint(a)
	for _, iss := range issues {
		if iss.IsError() {
			t.Errorf("unexpected error: %s — %s", iss.Path, iss.Message)
		}
	}
}

func TestCheckUnusedModels(t *testing.T) {
	a := ast.AST{
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}},
			{Name: "Orphan", Fields: []ast.Field{{Name: "x", Type: "string"}}},
		},
		Modules: []ast.Module{{
			Name: "Users",
			Actions: []ast.Action{{
				Name: "Get", Method: "GET", Path: "/", Output: "User", Middleware: []string{},
			}},
		}},
	}
	issues := checkUnusedModels(a)
	if len(issues) != 1 {
		t.Fatalf("expected 1, got %d", len(issues))
	}
	if issues[0].Rule != "unused-model" {
		t.Errorf("rule = %q", issues[0].Rule)
	}
}

func TestCheckEmptyModules(t *testing.T) {
	a := ast.AST{Modules: []ast.Module{{Name: "Empty"}}}
	issues := checkEmptyModules(a)
	if len(issues) != 1 {
		t.Fatalf("expected 1, got %d", len(issues))
	}
	if issues[0].Rule != "empty-module" {
		t.Errorf("rule = %q", issues[0].Rule)
	}
}

func TestCheckEmptyModels(t *testing.T) {
	a := ast.AST{Models: []ast.Model{{Name: "Empty"}}}
	issues := checkEmptyModels(a)
	if len(issues) != 1 {
		t.Fatalf("expected 1, got %d", len(issues))
	}
	if issues[0].Rule != "empty-model" {
		t.Errorf("rule = %q", issues[0].Rule)
	}
}

func TestCheckEmptyModelsWithExtends(t *testing.T) {
	a := ast.AST{Models: []ast.Model{{Name: "Child", Extends: "Parent"}}}
	issues := checkEmptyModels(a)
	if len(issues) != 0 {
		t.Error("model with extends and no fields should not be flagged")
	}
}

func TestCheckDuplicateRoutes(t *testing.T) {
	a := ast.AST{
		Modules: []ast.Module{
			{Name: "A", Prefix: "/api", Actions: []ast.Action{
				{Name: "Get", Method: "GET", Path: "/users", Middleware: []string{}},
			}},
			{Name: "B", Prefix: "/api", Actions: []ast.Action{
				{Name: "List", Method: "GET", Path: "/users", Middleware: []string{}},
			}},
		},
	}
	issues := checkDuplicateRoutes(a)
	if len(issues) != 1 {
		t.Fatalf("expected 1, got %d", len(issues))
	}
	if issues[0].Rule != "duplicate-route" {
		t.Errorf("rule = %q", issues[0].Rule)
	}
}

func TestCheckDuplicateActionNames(t *testing.T) {
	a := ast.AST{
		Modules: []ast.Module{{
			Name: "Users",
			Actions: []ast.Action{
				{Name: "Get", Method: "GET", Path: "/", Middleware: []string{}},
				{Name: "Get", Method: "POST", Path: "/other", Middleware: []string{}},
			},
		}},
	}
	issues := checkDuplicateActionNames(a)
	if len(issues) != 1 {
		t.Fatalf("expected 1, got %d", len(issues))
	}
}

func TestCheckMissingDescriptions(t *testing.T) {
	a := ast.AST{
		Modules: []ast.Module{{
			Name: "Users",
			Actions: []ast.Action{
				{Name: "Get", Method: "GET", Path: "/", Middleware: []string{}},
			},
		}},
	}
	issues := checkMissingDescriptions(a)
	if len(issues) < 2 {
		t.Errorf("expected at least 2 (module + action), got %d", len(issues))
	}
}

func TestCheckDeprecatedActions(t *testing.T) {
	a := ast.AST{
		Modules: []ast.Module{{
			Name: "Users",
			Actions: []ast.Action{
				{Name: "Old", Method: "GET", Path: "/", Deprecated: "use New", Middleware: []string{}},
			},
		}},
		Models: []ast.Model{{
			Name: "User",
			Fields: []ast.Field{
				{Name: "old", Type: "string", Deprecated: "use new"},
			},
		}},
	}
	issues := checkDeprecatedActions(a)
	if len(issues) != 2 {
		t.Errorf("expected 2 (action + field), got %d", len(issues))
	}
}

func TestHasErrors(t *testing.T) {
	if HasErrors(nil) {
		t.Error("nil should not have errors")
	}
	if HasErrors([]Issue{{Severity: Warning}}) {
		t.Error("warnings only should not have errors")
	}
	if !HasErrors([]Issue{{Severity: Error}}) {
		t.Error("should detect errors")
	}
}

func TestSortIssues(t *testing.T) {
	issues := []Issue{
		{Severity: Warning, Path: "b"},
		{Severity: Error, Path: "z"},
		{Severity: Warning, Path: "a"},
		{Severity: Error, Path: "a"},
	}
	sortIssues(issues)
	if issues[0].Severity != Error {
		t.Error("errors should come first")
	}
}

package emitter

import (
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func TestIsPrimitive(t *testing.T) {
	primitives := []string{"string", "int", "float", "bool", "date", "datetime", "uuid"}
	for _, p := range primitives {
		if !IsPrimitive(p) {
			t.Errorf("expected %q to be primitive", p)
		}
	}
	nonPrimitives := []string{"User", "Role", "MyModel", ""}
	for _, np := range nonPrimitives {
		if IsPrimitive(np) {
			t.Errorf("expected %q to NOT be primitive", np)
		}
	}
}

func TestCollectTransitiveModels(t *testing.T) {
	a := ast.AST{
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{
				{Name: "id", Type: "string"},
				{Name: "address", Type: "Address"},
			}},
			{Name: "Address", Fields: []ast.Field{
				{Name: "city", Type: "string"},
				{Name: "country", Type: "Country"},
			}},
			{Name: "Country", Fields: []ast.Field{
				{Name: "name", Type: "string"},
			}},
			{Name: "Unrelated", Fields: []ast.Field{
				{Name: "x", Type: "string"},
			}},
		},
		Modules: []ast.Module{
			{Name: "Users", Actions: []ast.Action{
				{Name: "Get", Output: "User", Middleware: []string{}},
			}},
		},
	}

	used := CollectTransitiveModels(a, a.Modules[0])
	if !used["User"] {
		t.Error("expected User to be used")
	}
	if !used["Address"] {
		t.Error("expected Address to be transitively used")
	}
	if !used["Country"] {
		t.Error("expected Country to be transitively used")
	}
	if used["Unrelated"] {
		t.Error("expected Unrelated to NOT be used")
	}
}

func TestCollectUsedEnums(t *testing.T) {
	a := ast.AST{
		Enums: []ast.Enum{
			{Name: "Role", Values: []string{"admin", "user"}},
			{Name: "Unused", Values: []string{"a", "b"}},
		},
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{
				{Name: "role", Type: "Role"},
			}},
		},
		Modules: []ast.Module{
			{Name: "Users", Actions: []ast.Action{
				{Name: "Get", Output: "User", Middleware: []string{}},
			}},
		},
	}

	used := CollectUsedEnums(a, a.Modules[0])
	if !used["Role"] {
		t.Error("expected Role to be used")
	}
	if used["Unused"] {
		t.Error("expected Unused to NOT be used")
	}
}

func TestCollectUsedTypes(t *testing.T) {
	a := ast.AST{
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}},
			{Name: "LoginInput", Fields: []ast.Field{{Name: "email", Type: "string"}}},
		},
		Modules: []ast.Module{
			{Name: "Auth", Actions: []ast.Action{
				{Name: "Login", Input: "LoginInput", Output: "User", Middleware: []string{}},
			}},
		},
	}

	types := CollectUsedTypes(a, a.Modules[0])
	if len(types) < 2 {
		t.Fatalf("expected at least 2 types, got %d: %v", len(types), types)
	}
	foundUser, foundLogin := false, false
	for _, tp := range types {
		if tp == "User" {
			foundUser = true
		}
		if tp == "LoginInput" {
			foundLogin = true
		}
	}
	if !foundUser || !foundLogin {
		t.Errorf("expected User and LoginInput in types, got %v", types)
	}
}

func TestCollectModuleMiddleware(t *testing.T) {
	mod := ast.Module{
		Name: "Auth",
		Actions: []ast.Action{
			{Name: "Login", Middleware: []string{"RateLimit", "Logger"}},
			{Name: "Me", Middleware: []string{"AuthGuard", "RateLimit"}},
		},
	}
	mw := CollectModuleMiddleware(mod)
	if len(mw) != 3 {
		t.Fatalf("expected 3 unique middleware, got %d: %v", len(mw), mw)
	}
	// RateLimit should appear only once
	count := 0
	for _, m := range mw {
		if m == "RateLimit" {
			count++
		}
	}
	if count != 1 {
		t.Errorf("RateLimit should appear once, appeared %d times", count)
	}
}

func TestToSnakeCase(t *testing.T) {
	tests := []struct {
		input, expected string
	}{
		{"camelCase", "camel_case"},
		{"PascalCase", "pascal_case"},
		{"GetById", "get_by_id"},
		{"simple", "simple"},
		{"HTMLParser", "h_t_m_l_parser"},
		{"", ""},
	}
	for _, tc := range tests {
		got := ToSnakeCase(tc.input)
		if got != tc.expected {
			t.Errorf("ToSnakeCase(%q) = %q, want %q", tc.input, got, tc.expected)
		}
	}
}

func TestRegistryBackend(t *testing.T) {
	// Backends should already be registered via init() imports in tests
	backends := ListBackends()
	if len(backends) == 0 {
		t.Skip("no backends registered (this test runs in the emitter package without imports)")
	}
}

func TestRegistryGetUnknown(t *testing.T) {
	_, err := GetBackend("nonexistent_backend_xyz")
	if err == nil {
		t.Error("expected error for unknown backend")
	}
	_, err = GetFrontend("nonexistent_frontend_xyz")
	if err == nil {
		t.Error("expected error for unknown frontend")
	}
}

func TestRegistryNoneFrontend(t *testing.T) {
	fe, err := GetFrontend("none")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if fe != nil {
		t.Error("expected nil for 'none' frontend")
	}
}

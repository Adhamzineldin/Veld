package emitter

import (
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func TestIsPrimitive(t *testing.T) {
	primitives := []string{"string", "int", "long", "float", "decimal", "bool", "date", "datetime", "time", "uuid", "bytes", "any", "json"}
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

func TestMergeASTs(t *testing.T) {
	base := ast.AST{
		ASTVersion: "1.0.0",
		Models:     []ast.Model{{Name: "SharedModel"}},
		Enums:      []ast.Enum{{Name: "SharedEnum", Values: []string{"a", "b"}}},
		Modules:    []ast.Module{{Name: "SharedModule"}},
	}

	consumed := []ConsumedServiceInfo{
		{
			Name: "iam",
			AST: ast.AST{
				Models:  []ast.Model{{Name: "User"}, {Name: "SharedModel"}}, // SharedModel is a dupe
				Enums:   []ast.Enum{{Name: "Role", Values: []string{"admin", "user"}}},
				Modules: []ast.Module{{Name: "IAM", Prefix: "/api/iam"}},
			},
			BaseUrl: "http://iam:3001",
		},
		{
			Name: "accounts",
			AST: ast.AST{
				Models:  []ast.Model{{Name: "Account"}, {Name: "User"}}, // User is a dupe from iam
				Modules: []ast.Module{{Name: "Accounts", Prefix: "/api/accounts"}},
			},
			BaseUrl: "http://accounts:3002",
		},
	}

	merged := MergeASTs(base, consumed)

	// Check models are deduplicated.
	if len(merged.Models) != 3 {
		t.Errorf("expected 3 models (SharedModel, User, Account), got %d", len(merged.Models))
	}
	modelNames := make(map[string]bool)
	for _, m := range merged.Models {
		modelNames[m.Name] = true
	}
	for _, expected := range []string{"SharedModel", "User", "Account"} {
		if !modelNames[expected] {
			t.Errorf("missing model %s", expected)
		}
	}

	// Check enums are deduplicated.
	if len(merged.Enums) != 2 {
		t.Errorf("expected 2 enums (SharedEnum, Role), got %d", len(merged.Enums))
	}

	// Check modules are deduplicated.
	if len(merged.Modules) != 3 {
		t.Errorf("expected 3 modules (SharedModule, IAM, Accounts), got %d", len(merged.Modules))
	}

	// Check order: base first, then consumed in order.
	if merged.Models[0].Name != "SharedModel" {
		t.Errorf("expected first model to be SharedModel (from base), got %s", merged.Models[0].Name)
	}
}

func TestMergeASTsEmpty(t *testing.T) {
	base := ast.AST{ASTVersion: "1.0.0"}
	merged := MergeASTs(base, nil)
	if len(merged.Models) != 0 || len(merged.Modules) != 0 {
		t.Error("expected empty merged AST when no consumed services")
	}
}

func TestApplyTopLevelPrefix(t *testing.T) {
	a := ast.AST{
		Prefix: "/api/v1",
		Modules: []ast.Module{
			{Name: "IAM", Prefix: "/iam"},
			{Name: "Bare"},
		},
	}
	got := ApplyTopLevelPrefix(a)
	if got.Prefix != "" {
		t.Errorf("expected top-level prefix to be cleared, got %q", got.Prefix)
	}
	if got.Modules[0].Prefix != "/api/v1/iam" {
		t.Errorf("expected /api/v1/iam, got %q", got.Modules[0].Prefix)
	}
	if got.Modules[1].Prefix != "/api/v1" {
		t.Errorf("expected /api/v1, got %q", got.Modules[1].Prefix)
	}
	// Idempotent: running it again must not double-prefix.
	again := ApplyTopLevelPrefix(got)
	if again.Modules[0].Prefix != "/api/v1/iam" {
		t.Errorf("idempotency broken: %q", again.Modules[0].Prefix)
	}
	// Caller's AST must not be mutated (defensive copy).
	if a.Modules[0].Prefix != "/iam" {
		t.Errorf("input AST was mutated: %q", a.Modules[0].Prefix)
	}
}

func TestApplyTopLevelPrefixEmpty(t *testing.T) {
	a := ast.AST{Modules: []ast.Module{{Name: "X", Prefix: "/x"}}}
	got := ApplyTopLevelPrefix(a)
	if got.Modules[0].Prefix != "/x" {
		t.Errorf("no top-level prefix should leave module untouched, got %q", got.Modules[0].Prefix)
	}
}

func TestSuccessStatusForAction(t *testing.T) {
	tests := []struct {
		name     string
		act      ast.Action
		expected int
	}{
		{"POST defaults to 201", ast.Action{Method: "POST", Output: "User"}, 201},
		{"GET defaults to 200", ast.Action{Method: "GET", Output: "User"}, 200},
		{"PUT defaults to 200", ast.Action{Method: "PUT", Output: "User"}, 200},
		{"DELETE no output defaults to 204", ast.Action{Method: "DELETE", Output: ""}, 204},
		{"DELETE with output defaults to 200", ast.Action{Method: "DELETE", Output: "User"}, 200},
		{"custom 202 overrides POST default", ast.Action{Method: "POST", Output: "Job", SuccessStatus: 202}, 202},
		{"custom 200 overrides POST default", ast.Action{Method: "POST", Output: "User", SuccessStatus: 200}, 200},
		{"custom 205 on GET", ast.Action{Method: "GET", Output: "Report", SuccessStatus: 205}, 205},
		{"custom 204 on DELETE", ast.Action{Method: "DELETE", Output: "", SuccessStatus: 204}, 204},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := SuccessStatusForAction(tt.act)
			if got != tt.expected {
				t.Errorf("SuccessStatusForAction() = %d, want %d", got, tt.expected)
			}
		})
	}
}

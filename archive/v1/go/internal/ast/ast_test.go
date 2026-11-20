package ast

import (
	"encoding/json"
	"testing"
)

func TestASTJSONRoundtrip(t *testing.T) {
	original := AST{
		ASTVersion: "1.0.0",
		Prefix:     "/api",
		Models: []Model{
			{
				Name:        "User",
				Description: "A user",
				Extends:     "",
				Fields: []Field{
					{Name: "id", Type: "uuid"},
					{Name: "email", Type: "string"},
					{Name: "tags", Type: "string", IsArray: true},
					{Name: "meta", Type: "Map", IsMap: true, MapValueType: "string"},
					{Name: "bio", Type: "string", Optional: true},
					{Name: "role", Type: "Role", Default: "user"},
					{Name: "old", Type: "string", Deprecated: "use newField"},
					{Name: "ex", Type: "string", Example: "hello"},
					{Name: "uniq", Type: "string", Unique: true},
					{Name: "idx", Type: "string", Index: true},
					{Name: "ref", Type: "Profile", Relation: "Profile"},
				},
			},
		},
		Enums: []Enum{
			{Name: "Role", Values: []string{"admin", "user"}, Description: "User role"},
		},
		Modules: []Module{
			{
				Name:        "Users",
				Description: "User management",
				Prefix:      "/users",
				Actions: []Action{
					{
						Name:        "GetUser",
						Method:      "GET",
						Path:        "/:id",
						Output:      "User",
						OutputArray: false,
						Middleware:  []string{"Auth"},
					},
					{
						Name:        "ListUsers",
						Method:      "GET",
						Path:        "/",
						Output:      "User",
						OutputArray: true,
						Query:       "ListQuery",
					},
				},
			},
		},
	}

	data, err := json.MarshalIndent(original, "", "  ")
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var decoded AST
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if decoded.ASTVersion != "1.0.0" {
		t.Errorf("ASTVersion = %q, want 1.0.0", decoded.ASTVersion)
	}
	if decoded.Prefix != "/api" {
		t.Errorf("Prefix = %q, want /api", decoded.Prefix)
	}
	if len(decoded.Models) != 1 {
		t.Fatalf("Models len = %d, want 1", len(decoded.Models))
	}
	if len(decoded.Models[0].Fields) != 11 {
		t.Errorf("Fields len = %d, want 11", len(decoded.Models[0].Fields))
	}
	if len(decoded.Enums) != 1 {
		t.Errorf("Enums len = %d, want 1", len(decoded.Enums))
	}
	if len(decoded.Modules) != 1 {
		t.Fatalf("Modules len = %d, want 1", len(decoded.Modules))
	}
	if len(decoded.Modules[0].Actions) != 2 {
		t.Errorf("Actions len = %d, want 2", len(decoded.Modules[0].Actions))
	}
}

func TestFieldAnnotationFields(t *testing.T) {
	f := Field{
		Name:     "email",
		Type:     "string",
		Unique:   true,
		Index:    true,
		Relation: "Profile",
		Example:  "test@example.com",
	}

	data, _ := json.Marshal(f)
	var decoded Field
	json.Unmarshal(data, &decoded)

	if !decoded.Unique {
		t.Error("Unique should be true")
	}
	if !decoded.Index {
		t.Error("Index should be true")
	}
	if decoded.Relation != "Profile" {
		t.Errorf("Relation = %q, want Profile", decoded.Relation)
	}
	if decoded.Example != "test@example.com" {
		t.Errorf("Example = %q, want test@example.com", decoded.Example)
	}
}

func TestZeroValueOmitted(t *testing.T) {
	f := Field{Name: "id", Type: "string"}
	data, _ := json.Marshal(f)
	s := string(data)

	// These zero-value fields should be omitted
	for _, key := range []string{"optional", "isArray", "isMap", "mapValueType", "default", "deprecated", "example", "unique", "index", "relation"} {
		if contains(s, `"`+key+`"`) {
			t.Errorf("zero-value field %q should be omitted from JSON", key)
		}
	}
}

func contains(s, sub string) bool {
	return len(s) >= len(sub) && searchString(s, sub)
}

func searchString(s, sub string) bool {
	for i := 0; i <= len(s)-len(sub); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}

func TestActionDefaults(t *testing.T) {
	a := Action{
		Name:       "Test",
		Method:     "GET",
		Path:       "/test",
		Middleware: []string{},
	}
	if a.OutputArray {
		t.Error("OutputArray should default to false")
	}
	if a.Stream != "" {
		t.Error("Stream should default to empty")
	}
	if len(a.Errors) != 0 {
		t.Error("Errors should default to empty")
	}
}

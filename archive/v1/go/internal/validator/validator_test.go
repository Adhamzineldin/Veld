package validator

import (
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func TestValidateEmpty(t *testing.T) {
	errs := Validate(ast.AST{ASTVersion: "1.0.0"})
	if len(errs) != 0 {
		t.Errorf("expected no errors for empty AST, got %d", len(errs))
	}
}

func TestValidateValid(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}},
			{Name: "LoginInput", Fields: []ast.Field{{Name: "email", Type: "string"}}},
		},
		Modules: []ast.Module{
			{Name: "Auth", Actions: []ast.Action{
				{Name: "Login", Method: "POST", Path: "/login", Input: "LoginInput", Output: "User", Middleware: []string{}},
			}},
		},
	}
	errs := Validate(a)
	if len(errs) != 0 {
		for _, e := range errs {
			t.Errorf("unexpected error: %v", e)
		}
	}
}

func TestValidateDuplicateModel(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}},
			{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}},
		},
	}
	errs := Validate(a)
	if len(errs) != 1 {
		t.Fatalf("expected 1 error, got %d: %v", len(errs), errs)
	}
	if !strings.Contains(errs[0].Error(), "duplicate model name") {
		t.Errorf("expected 'duplicate model name' error, got: %v", errs[0])
	}
}

func TestValidateDuplicateEnum(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Enums: []ast.Enum{
			{Name: "Role", Values: []string{"admin"}},
			{Name: "Role", Values: []string{"user"}},
		},
	}
	errs := Validate(a)
	if len(errs) != 1 {
		t.Fatalf("expected 1 error, got %d: %v", len(errs), errs)
	}
	if !strings.Contains(errs[0].Error(), "duplicate enum name") {
		t.Errorf("expected 'duplicate enum name', got: %v", errs[0])
	}
}

func TestValidateEmptyEnum(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Enums:      []ast.Enum{{Name: "Empty", Values: []string{}}},
	}
	errs := Validate(a)
	if len(errs) != 1 || !strings.Contains(errs[0].Error(), "has no values") {
		t.Errorf("expected 'has no values' error, got: %v", errs)
	}
}

func TestValidateNameCollision(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Enums:      []ast.Enum{{Name: "Role", Values: []string{"admin"}}},
		Models:     []ast.Model{{Name: "Role", Fields: []ast.Field{{Name: "id", Type: "string"}}}},
	}
	errs := Validate(a)
	found := false
	for _, e := range errs {
		if strings.Contains(e.Error(), "name collision") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'name collision' error, got: %v", errs)
	}
}

func TestValidateUndefinedFieldType(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{{Name: "data", Type: "NonExistent"}}},
		},
	}
	errs := Validate(a)
	if len(errs) != 1 || !strings.Contains(errs[0].Error(), "undefined type") {
		t.Errorf("expected 'undefined type' error, got: %v", errs)
	}
}

func TestValidateUndefinedActionInput(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models:     []ast.Model{{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}}},
		Modules: []ast.Module{{Name: "Auth", Actions: []ast.Action{
			{Name: "Login", Method: "POST", Path: "/login", Input: "BadType", Output: "User", Middleware: []string{}},
		}}},
	}
	errs := Validate(a)
	found := false
	for _, e := range errs {
		if strings.Contains(e.Error(), "undefined input type") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'undefined input type' error, got: %v", errs)
	}
}

func TestValidateUndefinedActionOutput(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models:     []ast.Model{{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}}},
		Modules: []ast.Module{{Name: "Auth", Actions: []ast.Action{
			{Name: "Login", Method: "POST", Path: "/login", Output: "BadOutput", Middleware: []string{}},
		}}},
	}
	errs := Validate(a)
	found := false
	for _, e := range errs {
		if strings.Contains(e.Error(), "undefined output type") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'undefined output type' error, got: %v", errs)
	}
}

func TestValidateUndefinedQuery(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models:     []ast.Model{{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}}},
		Modules: []ast.Module{{Name: "Users", Actions: []ast.Action{
			{Name: "List", Method: "GET", Path: "/users", Output: "User", Query: "BadQuery", Middleware: []string{}},
		}}},
	}
	errs := Validate(a)
	found := false
	for _, e := range errs {
		if strings.Contains(e.Error(), "undefined query type") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'undefined query type' error, got: %v", errs)
	}
}

func TestValidateDuplicateAction(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models:     []ast.Model{{Name: "User", Fields: []ast.Field{{Name: "id", Type: "string"}}}},
		Modules: []ast.Module{{Name: "Auth", Actions: []ast.Action{
			{Name: "Login", Method: "POST", Path: "/login", Output: "User", Middleware: []string{}},
			{Name: "Login", Method: "GET", Path: "/login2", Output: "User", Middleware: []string{}},
		}}},
	}
	errs := Validate(a)
	found := false
	for _, e := range errs {
		if strings.Contains(e.Error(), "duplicate action name") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'duplicate action name' error, got: %v", errs)
	}
}

func TestValidateDuplicateField(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{{Name: "User", Fields: []ast.Field{
			{Name: "id", Type: "string"},
			{Name: "id", Type: "int"},
		}}},
	}
	errs := Validate(a)
	found := false
	for _, e := range errs {
		if strings.Contains(e.Error(), "duplicate field name") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected 'duplicate field name' error, got: %v", errs)
	}
}

func TestValidateBadDefaultType(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{{Name: "User", Fields: []ast.Field{
			{Name: "age", Type: "int", Default: `"hello"`},
		}}},
	}
	errs := Validate(a)
	if len(errs) == 0 {
		t.Fatal("expected error for string default on int field")
	}
	if !strings.Contains(errs[0].Error(), "@default for int must be a number") {
		t.Errorf("unexpected error: %v", errs[0])
	}
}

func TestValidateDefaultBoolOnIntField(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{{Name: "Config", Fields: []ast.Field{
			{Name: "count", Type: "int", Default: "true"},
		}}},
	}
	errs := Validate(a)
	if len(errs) == 0 {
		t.Fatal("expected error for bool default on int field")
	}
	if !strings.Contains(errs[0].Error(), "@default for int must be a number") {
		t.Errorf("unexpected error: %v", errs[0])
	}
}

func TestValidateDefaultBoolOnFloatField(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{{Name: "Config", Fields: []ast.Field{
			{Name: "rate", Type: "float", Default: "false"},
		}}},
	}
	errs := Validate(a)
	if len(errs) == 0 {
		t.Fatal("expected error for bool default on float field")
	}
	if !strings.Contains(errs[0].Error(), "@default for float must be a number") {
		t.Errorf("unexpected error: %v", errs[0])
	}
}

func TestValidateDefaultNumberOnBoolField(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{{Name: "Config", Fields: []ast.Field{
			{Name: "enabled", Type: "bool", Default: "42"},
		}}},
	}
	errs := Validate(a)
	if len(errs) == 0 {
		t.Fatal("expected error for number default on bool field")
	}
	if !strings.Contains(errs[0].Error(), "@default for bool must be true or false") {
		t.Errorf("unexpected error: %v", errs[0])
	}
}

func TestValidateDefaultStringOnBoolField(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{{Name: "Config", Fields: []ast.Field{
			{Name: "enabled", Type: "bool", Default: `"yes"`},
		}}},
	}
	errs := Validate(a)
	if len(errs) == 0 {
		t.Fatal("expected error for string default on bool field")
	}
	if !strings.Contains(errs[0].Error(), "@default for bool must be true or false") {
		t.Errorf("unexpected error: %v", errs[0])
	}
}

func TestValidateDefaultFloatOnIntField(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{{Name: "Config", Fields: []ast.Field{
			{Name: "count", Type: "int", Default: "3.14"},
		}}},
	}
	errs := Validate(a)
	if len(errs) == 0 {
		t.Fatal("expected error for float default on int field")
	}
	if !strings.Contains(errs[0].Error(), "@default for int must be a whole number") {
		t.Errorf("unexpected error: %v", errs[0])
	}
}

func TestValidateDefaultNumberOnStringField(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{{Name: "Config", Fields: []ast.Field{
			{Name: "name", Type: "string", Default: "42"},
		}}},
	}
	errs := Validate(a)
	if len(errs) == 0 {
		t.Fatal("expected error for unquoted number on string field")
	}
	if !strings.Contains(errs[0].Error(), "@default for string must be a quoted string") {
		t.Errorf("unexpected error: %v", errs[0])
	}
}

func TestValidateDefaultValidCases(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Enums:      []ast.Enum{{Name: "Role", Values: []string{"admin", "user"}}},
		Models: []ast.Model{{Name: "Config", Fields: []ast.Field{
			{Name: "name", Type: "string", Default: `"hello"`},
			{Name: "count", Type: "int", Default: "42"},
			{Name: "rate", Type: "float", Default: "3.14"},
			{Name: "enabled", Type: "bool", Default: "true"},
			{Name: "role", Type: "Role", Default: "admin"},
			{Name: "createdAt", Type: "date", Default: `"2024-01-01"`},
			{Name: "uid", Type: "uuid", Default: `"550e8400-e29b-41d4-a716-446655440000"`},
		}}},
	}
	errs := Validate(a)
	for _, e := range errs {
		// Filter out only @default errors
		if strings.Contains(e.Error(), "@default") {
			t.Errorf("unexpected @default error: %v", e)
		}
	}
}

func TestValidateFileLineContext(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{
			{Name: "User", SourceFile: "/project/veld/models/user.veld", Line: 5, Fields: []ast.Field{{Name: "id", Type: "string"}}},
			{Name: "User", SourceFile: "/project/veld/models/user.veld", Line: 10, Fields: []ast.Field{{Name: "id", Type: "string"}}},
		},
	}
	errs := Validate(a)
	if len(errs) != 1 {
		t.Fatalf("expected 1 error, got %d", len(errs))
	}
	errStr := errs[0].Error()
	if !strings.Contains(errStr, "user.veld:10") {
		t.Errorf("expected 'user.veld:10' in error, got: %s", errStr)
	}
}

func TestValidateVoidOutputAllowed(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Modules: []ast.Module{{Name: "Auth", Actions: []ast.Action{
			{Name: "Logout", Method: "POST", Path: "/logout", Output: "", Middleware: []string{}},
		}}},
	}
	errs := Validate(a)
	if len(errs) != 0 {
		t.Errorf("expected no errors for void output, got: %v", errs)
	}
}

func TestValidatePrimitiveOutput(t *testing.T) {
	a := ast.AST{
		ASTVersion: "1.0.0",
		Modules: []ast.Module{{Name: "Health", Actions: []ast.Action{
			{Name: "Check", Method: "GET", Path: "/health", Output: "string", Middleware: []string{}},
		}}},
	}
	errs := Validate(a)
	if len(errs) != 0 {
		t.Errorf("expected no errors for primitive output, got: %v", errs)
	}
}

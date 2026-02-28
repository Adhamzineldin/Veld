package parser

import (
	"testing"

	"github.com/veld-dev/veld/internal/lexer"
)

func parse(src string) error {
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		return err
	}
	_, err = New(tokens).Parse()
	return err
}

func mustParse(t *testing.T, src string) {
	t.Helper()
	if err := parse(src); err != nil {
		t.Fatalf("unexpected parse error: %v", err)
	}
}

func TestParseEmptyFile(t *testing.T) {
	mustParse(t, "")
}

func TestParseSimpleModel(t *testing.T) {
	src := `model User {
  name: string
  age: int
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(a.Models) != 1 {
		t.Fatalf("expected 1 model, got %d", len(a.Models))
	}
	m := a.Models[0]
	if m.Name != "User" {
		t.Errorf("expected model name 'User', got %q", m.Name)
	}
	if len(m.Fields) != 2 {
		t.Fatalf("expected 2 fields, got %d", len(m.Fields))
	}
	if m.Fields[0].Name != "name" || m.Fields[0].Type != "string" {
		t.Errorf("field 0: expected name:string, got %s:%s", m.Fields[0].Name, m.Fields[0].Type)
	}
	if m.Fields[1].Name != "age" || m.Fields[1].Type != "int" {
		t.Errorf("field 1: expected age:int, got %s:%s", m.Fields[1].Name, m.Fields[1].Type)
	}
	if m.Line != 1 {
		t.Errorf("expected model line 1, got %d", m.Line)
	}
}

func TestParseModelWithDescription(t *testing.T) {
	src := `model User {
  description: "A platform user"
  name: string
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if a.Models[0].Description != "A platform user" {
		t.Errorf("expected description, got %q", a.Models[0].Description)
	}
}

func TestParseOptionalField(t *testing.T) {
	src := `model User {
  bio?: string
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !a.Models[0].Fields[0].Optional {
		t.Error("expected bio to be optional")
	}
}

func TestParseArrayField(t *testing.T) {
	src := `model User {
  tags: string[]
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	f := a.Models[0].Fields[0]
	if !f.IsArray {
		t.Error("expected tags to be an array")
	}
	if f.Type != "string" {
		t.Errorf("expected type 'string', got %q", f.Type)
	}
}

func TestParseDefaultAnnotation(t *testing.T) {
	src := `model User {
  verified: bool @default(false)
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	f := a.Models[0].Fields[0]
	if f.Default != "false" {
		t.Errorf("expected default 'false', got %q", f.Default)
	}
}

func TestParseDefaultStringAnnotation(t *testing.T) {
	src := `model Config {
  name: string @default("hello")
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	f := a.Models[0].Fields[0]
	if f.Default != `"hello"` {
		t.Errorf("expected default '\"hello\"', got %q", f.Default)
	}
}

func TestParseEnum(t *testing.T) {
	src := `enum Role {
  admin
  user
  guest
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(a.Enums) != 1 {
		t.Fatalf("expected 1 enum, got %d", len(a.Enums))
	}
	e := a.Enums[0]
	if e.Name != "Role" {
		t.Errorf("expected enum name 'Role', got %q", e.Name)
	}
	if len(e.Values) != 3 {
		t.Fatalf("expected 3 values, got %d", len(e.Values))
	}
	if e.Line != 1 {
		t.Errorf("expected enum line 1, got %d", e.Line)
	}
}

func TestParseModule(t *testing.T) {
	src := `model User { id: string }
module Auth {
  action Login {
    method: POST
    path: /auth/login
    input: User
    output: User
  }
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(a.Modules) != 1 {
		t.Fatalf("expected 1 module, got %d", len(a.Modules))
	}
	mod := a.Modules[0]
	if mod.Name != "Auth" {
		t.Errorf("expected module name 'Auth', got %q", mod.Name)
	}
	if len(mod.Actions) != 1 {
		t.Fatalf("expected 1 action, got %d", len(mod.Actions))
	}
	act := mod.Actions[0]
	if act.Name != "Login" {
		t.Errorf("expected action name 'Login', got %q", act.Name)
	}
	if act.Method != "POST" {
		t.Errorf("expected method POST, got %q", act.Method)
	}
	if act.Path != "/auth/login" {
		t.Errorf("expected path /auth/login, got %q", act.Path)
	}
	if act.Input != "User" {
		t.Errorf("expected input User, got %q", act.Input)
	}
}

func TestParseModuleWithPrefix(t *testing.T) {
	src := `model User { id: string }
module Users {
  prefix: /api
  action List {
    method: GET
    path: /users
    output: User
  }
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if a.Modules[0].Prefix != "/api" {
		t.Errorf("expected prefix /api, got %q", a.Modules[0].Prefix)
	}
}

func TestParseActionWithMiddleware(t *testing.T) {
	src := `model User { id: string }
module Auth {
  action Me {
    method: GET
    path: /auth/me
    output: User
    middleware: AuthGuard
    middleware: RateLimit
  }
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	mw := a.Modules[0].Actions[0].Middleware
	if len(mw) != 2 {
		t.Fatalf("expected 2 middleware, got %d", len(mw))
	}
	if mw[0] != "AuthGuard" || mw[1] != "RateLimit" {
		t.Errorf("expected [AuthGuard, RateLimit], got %v", mw)
	}
}

func TestParseOutputArray(t *testing.T) {
	src := `model User { id: string }
module Users {
  action List {
    method: GET
    path: /users
    output: User[]
  }
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	act := a.Modules[0].Actions[0]
	if !act.OutputArray {
		t.Error("expected OutputArray to be true")
	}
	if act.Output != "User" {
		t.Errorf("expected output 'User', got %q", act.Output)
	}
}

func TestParseActionWithQuery(t *testing.T) {
	src := `model Filters { search: string }
model User { id: string }
module Users {
  action List {
    method: GET
    path: /users
    query: Filters
    output: User[]
  }
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if a.Modules[0].Actions[0].Query != "Filters" {
		t.Errorf("expected query 'Filters', got %q", a.Modules[0].Actions[0].Query)
	}
}

func TestParseImport(t *testing.T) {
	src := `import "models/auth.veld"
import "modules/auth.veld"`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(a.Imports) != 2 {
		t.Fatalf("expected 2 imports, got %d", len(a.Imports))
	}
	if a.Imports[0] != "models/auth.veld" {
		t.Errorf("expected 'models/auth.veld', got %q", a.Imports[0])
	}
}

func TestParseActionNoOutput(t *testing.T) {
	// Actions without output are allowed (void)
	src := `module Auth {
  action Logout {
    method: POST
    path: /auth/logout
  }
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if a.Modules[0].Actions[0].Output != "" {
		t.Errorf("expected empty output for void action, got %q", a.Modules[0].Actions[0].Output)
	}
}

func TestParseLineTracking(t *testing.T) {
	src := `model User {
  name: string
}

module Auth {
  action Login {
    method: POST
    path: /login
    output: User
  }
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if a.Models[0].Line != 1 {
		t.Errorf("expected model at line 1, got %d", a.Models[0].Line)
	}
	if a.Modules[0].Line != 5 {
		t.Errorf("expected module at line 5, got %d", a.Modules[0].Line)
	}
	if a.Modules[0].Actions[0].Line != 6 {
		t.Errorf("expected action at line 6, got %d", a.Modules[0].Actions[0].Line)
	}
}

// ── Error cases ──────────────────────────────────────────────────────────────

func TestParseMissingBrace(t *testing.T) {
	src := `model User {
  name: string`
	if err := parse(src); err == nil {
		t.Fatal("expected error for missing closing brace")
	}
}

func TestParseMissingMethod(t *testing.T) {
	src := `module Auth {
  action Login {
    path: /login
  }
}`
	if err := parse(src); err == nil {
		t.Fatal("expected error for missing method")
	}
}

func TestParseMissingPath(t *testing.T) {
	src := `module Auth {
  action Login {
    method: POST
  }
}`
	if err := parse(src); err == nil {
		t.Fatal("expected error for missing path")
	}
}

func TestParseUnexpectedToken(t *testing.T) {
	src := `model 123`
	if err := parse(src); err == nil {
		t.Fatal("expected error for unexpected token")
	}
}

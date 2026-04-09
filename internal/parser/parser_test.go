package parser

import (
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/lexer"
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

func TestParseActionWithMiddlewareBracketList(t *testing.T) {
	src := `model User { id: string }
module Auth {
  action Register {
    method: POST
    path: /auth/register
    input: User
    output: User
    middleware: [validate, hashPassword, sendEmail]
  }
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	mw := a.Modules[0].Actions[0].Middleware
	if len(mw) != 3 {
		t.Fatalf("expected 3 middleware, got %d", len(mw))
	}
	if mw[0] != "validate" || mw[1] != "hashPassword" || mw[2] != "sendEmail" {
		t.Errorf("expected [validate, hashPassword, sendEmail], got %v", mw)
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
	if len(a.Modules[0].Actions[0].QueryFields) != 0 {
		t.Errorf("expected no QueryFields for named query, got %d", len(a.Modules[0].Actions[0].QueryFields))
	}
}

func TestParseActionWithInlineQuery(t *testing.T) {
	src := `module Equipment {
  action GetContractors {
    method: GET
    path: /contractors
    query: { areaCode: string }
    output: string[]
  }
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	act := a.Modules[0].Actions[0]
	if act.Query != "GetContractorsQuery" {
		t.Errorf("expected synthetic query name 'GetContractorsQuery', got %q", act.Query)
	}
	if len(act.QueryFields) != 1 {
		t.Fatalf("expected 1 QueryField, got %d", len(act.QueryFields))
	}
	if act.QueryFields[0].Name != "areaCode" || act.QueryFields[0].Type != "string" {
		t.Errorf("unexpected field: %+v", act.QueryFields[0])
	}
	// Synthetic model should be added to AST.Models
	found := false
	for _, m := range a.Models {
		if m.Name == "GetContractorsQuery" {
			found = true
			if len(m.Fields) != 1 || m.Fields[0].Name != "areaCode" {
				t.Errorf("synthetic model fields mismatch: %+v", m.Fields)
			}
		}
	}
	if !found {
		t.Error("synthetic model 'GetContractorsQuery' not found in AST.Models")
	}
}

func TestParseActionWithInlineQueryMultipleFields(t *testing.T) {
	src := `module Equipment {
  action GetRate {
    method: GET
    path: /rate
    query: { equipmentId: string, contractor: string, area: string }
    output: string
  }
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	act := a.Modules[0].Actions[0]
	if act.Query != "GetRateQuery" {
		t.Errorf("expected 'GetRateQuery', got %q", act.Query)
	}
	if len(act.QueryFields) != 3 {
		t.Fatalf("expected 3 QueryFields, got %d", len(act.QueryFields))
	}
}

func TestParseActionWithInlineOutput(t *testing.T) {
	src := `module Equipment {
  action GetDailyRate {
    method: GET
    path: /rate
    output: { price: float, currency: string }
  }
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	act := a.Modules[0].Actions[0]
	if act.Output != "GetDailyRateOutput" {
		t.Errorf("expected synthetic output name 'GetDailyRateOutput', got %q", act.Output)
	}
	if act.OutputArray {
		t.Error("expected OutputArray to be false")
	}
	if len(act.OutputFields) != 2 {
		t.Fatalf("expected 2 OutputFields, got %d", len(act.OutputFields))
	}
	if act.OutputFields[0].Name != "price" || act.OutputFields[0].Type != "float" {
		t.Errorf("unexpected field 0: %+v", act.OutputFields[0])
	}
	if act.OutputFields[1].Name != "currency" || act.OutputFields[1].Type != "string" {
		t.Errorf("unexpected field 1: %+v", act.OutputFields[1])
	}
	// Synthetic model should exist in AST.Models
	found := false
	for _, m := range a.Models {
		if m.Name == "GetDailyRateOutput" {
			found = true
			if len(m.Fields) != 2 {
				t.Errorf("synthetic model fields count mismatch: %d", len(m.Fields))
			}
		}
	}
	if !found {
		t.Error("synthetic model 'GetDailyRateOutput' not found in AST.Models")
	}
}

func TestParseActionWithInlineOutputArray(t *testing.T) {
	src := `module Equipment {
  action ListRates {
    method: GET
    path: /rates
    output: { price: float, name: string }[]
  }
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	act := a.Modules[0].Actions[0]
	if act.Output != "ListRatesOutput" {
		t.Errorf("expected 'ListRatesOutput', got %q", act.Output)
	}
	if !act.OutputArray {
		t.Error("expected OutputArray to be true for inline output with []")
	}
	if len(act.OutputFields) != 2 {
		t.Fatalf("expected 2 OutputFields, got %d", len(act.OutputFields))
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

// ── Import syntax tests ──────────────────────────────────────────────────────

func TestParseImportVariants(t *testing.T) {
	tests := []struct {
		name   string
		src    string
		expect string
	}{
		{"import @alias/name", `import @models/user`, "@models/user.veld"},
		{"import @alias/*", `import @models/*`, "@models/*"},
		{"import @alias (bare)", `import @models`, "@models/*"},
		{"import alias/name", `import models/user`, "@models/user.veld"},
		{"import alias/*", `import models/*`, "@models/*"},
		{"import /path/*", `import /models/*`, "@models/*"},
		{"import /path/name", `import /models/user`, "@models/user.veld"},
		{"import /path (bare)", `import /models`, "@models/*"},
		{"from @alias import *", `from @models import *`, "@models/*"},
		{"from @alias import name", `from @models import user`, "@models/user.veld"},
		{"from /path import *", `from /models import *`, "@models/*"},
		{"from /path import name", `from /models import user`, "@models/user.veld"},
		{"from alias import *", `from models import *`, "@models/*"},
		{"from alias import name", `from models import user`, "@models/user.veld"},
		{"import quoted (legacy)", `import "models/user.veld"`, "models/user.veld"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tokens, err := lexer.New(tt.src).Tokenize()
			if err != nil {
				t.Fatalf("lex: %v", err)
			}
			a, err := New(tokens).Parse()
			if err != nil {
				t.Fatalf("parse: %v", err)
			}
			if len(a.Imports) != 1 {
				t.Fatalf("expected 1 import, got %d", len(a.Imports))
			}
			if a.Imports[0] != tt.expect {
				t.Errorf("expected %q, got %q", tt.expect, a.Imports[0])
			}
		})
	}
}

// ── Errors directive tests ───────────────────────────────────────────────────

func TestParseActionErrors(t *testing.T) {
	src := `module Users {
  action GetUser {
    method: GET
    path: /:id
    output: string
    errors: [NotFound, Unauthorized]
  }
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if len(a.Modules) != 1 || len(a.Modules[0].Actions) != 1 {
		t.Fatal("expected 1 module with 1 action")
	}
	act := a.Modules[0].Actions[0]
	if len(act.Errors) != 2 {
		t.Fatalf("expected 2 errors, got %d", len(act.Errors))
	}
	if act.Errors[0] != "NotFound" || act.Errors[1] != "Unauthorized" {
		t.Errorf("got errors %v", act.Errors)
	}
}

func TestParseActionNoErrors(t *testing.T) {
	src := `model LoginInput { email: string }
model Token { value: string }
module Auth {
  action Login {
    method: POST
    path: /login
    input: LoginInput
    output: Token
  }
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	act := a.Modules[0].Actions[0]
	if len(act.Errors) != 0 {
		t.Errorf("expected no errors, got %v", act.Errors)
	}
}

// ── Top-level prefix test ────────────────────────────────────────────────────

func TestParseTopLevelPrefix(t *testing.T) {
	src := `prefix: /api/v1
module Auth {
  action Login {
    method: POST
    path: /login
    output: string
  }
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if a.Prefix != "/api/v1" {
		t.Errorf("expected prefix /api/v1, got %q", a.Prefix)
	}
}

func TestParseExampleAnnotation(t *testing.T) {
	src := `model User {
  email: string @example("user@example.com")
  age: int @example(25)
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if len(a.Models) != 1 {
		t.Fatalf("expected 1 model, got %d", len(a.Models))
	}
	if a.Models[0].Fields[0].Example != "user@example.com" {
		t.Errorf("expected example 'user@example.com', got %q", a.Models[0].Fields[0].Example)
	}
	if a.Models[0].Fields[1].Example != "25" {
		t.Errorf("expected example '25', got %q", a.Models[0].Fields[1].Example)
	}
}

func TestParseUniqueAnnotation(t *testing.T) {
	src := `model User {
  email: string @unique
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if !a.Models[0].Fields[0].Unique {
		t.Error("email should be unique")
	}
}

func TestParseIndexAnnotation(t *testing.T) {
	src := `model User {
  email: string @index
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if !a.Models[0].Fields[0].Index {
		t.Error("email should be indexed")
	}
}

func TestParseRelationAnnotation(t *testing.T) {
	src := `model Post {
  author: User @relation(User)
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if a.Models[0].Fields[0].Relation != "User" {
		t.Errorf("expected relation 'User', got %q", a.Models[0].Fields[0].Relation)
	}
}

func TestParseMultipleAnnotations(t *testing.T) {
	src := `model User {
  email: string @unique @index @example("test@test.com")
}`
	tokens, err := lexer.New(src).Tokenize()
	if err != nil {
		t.Fatalf("lex: %v", err)
	}
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	f := a.Models[0].Fields[0]
	if !f.Unique {
		t.Error("should be unique")
	}
	if !f.Index {
		t.Error("should be indexed")
	}
	if f.Example != "test@test.com" {
		t.Errorf("expected example 'test@test.com', got %q", f.Example)
	}
}

// ── Default value (= syntax) tests ───────────────────────────────────────────

func TestParseDefaultEqualsInt(t *testing.T) {
	src := `model Pagination {
  page: int = 0
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	f := a.Models[0].Fields[0]
	if f.Default != "0" {
		t.Errorf("expected default '0', got %q", f.Default)
	}
}

func TestParseDefaultEqualsString(t *testing.T) {
	src := `model Config {
  name: string = "hello"
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

func TestParseDefaultEqualsBool(t *testing.T) {
	src := `model Settings {
  verified: bool = false
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

func TestParseDefaultEqualsIdent(t *testing.T) {
	src := `enum Role { admin user guest }
model User {
  role: Role = user
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	f := a.Models[0].Fields[0]
	if f.Default != "user" {
		t.Errorf("expected default 'user', got %q", f.Default)
	}
}

func TestParseDefaultEqualsNegativeNumber(t *testing.T) {
	src := `model Account {
  balance: float = -1.5
}`
	tokens, _ := lexer.New(src).Tokenize()
	a, err := New(tokens).Parse()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	f := a.Models[0].Fields[0]
	if f.Default != "-1.5" {
		t.Errorf("expected default '-1.5', got %q", f.Default)
	}
}

func TestParseDefaultEqualsWithDeprecated(t *testing.T) {
	src := `model User {
  name: string = "hello" @deprecated "use fullName"
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
	if f.Deprecated != "use fullName" {
		t.Errorf("expected deprecated 'use fullName', got %q", f.Deprecated)
	}
}

func TestParseDefaultEqualsConflictWithAnnotation(t *testing.T) {
	src := `model Config {
  name: string = "hello" @default("world")
}`
	tokens, _ := lexer.New(src).Tokenize()
	_, err := New(tokens).Parse()
	if err == nil {
		t.Fatal("expected error for combining = value with @default()")
	}
	if !strings.Contains(err.Error(), "already has a default value") {
		t.Errorf("expected 'already has a default value' error, got: %v", err)
	}
}

func TestParseBlockComment(t *testing.T) {
	src := `/* Block comment */
model User {
  /* field comment */
  name: string
}`
	mustParse(t, src)
}

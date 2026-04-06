package lexer

import (
	"testing"
)

func TestTokenizeEmpty(t *testing.T) {
	tokens, err := New("").Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(tokens) != 1 || tokens[0].Type != TEOF {
		t.Fatalf("expected single EOF token, got %d tokens", len(tokens))
	}
}

func TestTokenizeComment(t *testing.T) {
	tokens, err := New("// this is a comment\n").Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(tokens) != 1 || tokens[0].Type != TEOF {
		t.Fatalf("expected only EOF after comment, got %d tokens", len(tokens))
	}
}

func TestTokenizeModel(t *testing.T) {
	src := `model User {
  name: string
  age: int
}`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := []struct {
		typ TokenType
		val string
	}{
		{TModel, "model"},
		{TIdent, "User"},
		{TLBrace, "{"},
		{TIdent, "name"},
		{TColon, ":"},
		{TTypeString, "string"},
		{TIdent, "age"},
		{TColon, ":"},
		{TTypeInt, "int"},
		{TRBrace, "}"},
		{TEOF, ""},
	}

	if len(tokens) != len(expected) {
		t.Fatalf("expected %d tokens, got %d", len(expected), len(tokens))
	}
	for i, e := range expected {
		if tokens[i].Type != e.typ {
			t.Errorf("token[%d]: expected type %s, got %s (value=%q)", i, e.typ, tokens[i].Type, tokens[i].Value)
		}
		if e.val != "" && tokens[i].Value != e.val {
			t.Errorf("token[%d]: expected value %q, got %q", i, e.val, tokens[i].Value)
		}
	}
}

func TestTokenizeAllTypes(t *testing.T) {
	src := "string int float decimal bool date datetime uuid"
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expectedTypes := []TokenType{
		TTypeString, TTypeInt, TTypeFloat, TTypeDecimal, TTypeBool,
		TTypeDate, TTypeDatetime, TTypeUUID, TEOF,
	}
	if len(tokens) != len(expectedTypes) {
		t.Fatalf("expected %d tokens, got %d", len(expectedTypes), len(tokens))
	}
	for i, et := range expectedTypes {
		if tokens[i].Type != et {
			t.Errorf("token[%d]: expected %s, got %s", i, et, tokens[i].Type)
		}
	}
}

func TestTokenizeHTTPMethods(t *testing.T) {
	src := "GET POST PUT DELETE PATCH"
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expectedTypes := []TokenType{TGET, TPOST, TPUT, TDELETE, TPATCH, TEOF}
	if len(tokens) != len(expectedTypes) {
		t.Fatalf("expected %d tokens, got %d", len(expectedTypes), len(tokens))
	}
	for i, et := range expectedTypes {
		if tokens[i].Type != et {
			t.Errorf("token[%d]: expected %s, got %s", i, et, tokens[i].Type)
		}
	}
}

func TestTokenizeKeywords(t *testing.T) {
	src := "model module action input output middleware import enum description query prefix method path"
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// input, output, middleware, description, query, prefix, method, path
	// are now contextual keywords — they emit TIdent so they can be used as field names.
	expectedTypes := []TokenType{
		TModel, TModule, TAction, TIdent, TIdent, TIdent,
		TImport, TEnum, TIdent, TIdent, TIdent, TIdent, TIdent, TEOF,
	}
	expectedValues := []string{
		"model", "module", "action", "input", "output", "middleware",
		"import", "enum", "description", "query", "prefix", "method", "path", "",
	}
	if len(tokens) != len(expectedTypes) {
		t.Fatalf("expected %d tokens, got %d", len(expectedTypes), len(tokens))
	}
	for i, et := range expectedTypes {
		if tokens[i].Type != et {
			t.Errorf("token[%d]: expected %s, got %s (value=%q)", i, et, tokens[i].Type, tokens[i].Value)
		}
		if expectedValues[i] != "" && tokens[i].Value != expectedValues[i] {
			t.Errorf("token[%d]: expected value %q, got %q", i, expectedValues[i], tokens[i].Value)
		}
	}
}

func TestTokenizePunctuation(t *testing.T) {
	src := `{ } : [ ] ? @ ( )`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expectedTypes := []TokenType{
		TLBrace, TRBrace, TColon, TLBracket, TRBracket,
		TQuestion, TAt, TLParen, TRParen, TEOF,
	}
	if len(tokens) != len(expectedTypes) {
		t.Fatalf("expected %d tokens, got %d", len(expectedTypes), len(tokens))
	}
	for i, et := range expectedTypes {
		if tokens[i].Type != et {
			t.Errorf("token[%d]: expected %s, got %s", i, et, tokens[i].Type)
		}
	}
}

func TestTokenizePath(t *testing.T) {
	src := `/auth/login /users/:id`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if tokens[0].Type != TPath || tokens[0].Value != "/auth/login" {
		t.Errorf("expected path /auth/login, got %s %q", tokens[0].Type, tokens[0].Value)
	}
	if tokens[1].Type != TPath || tokens[1].Value != "/users/:id" {
		t.Errorf("expected path /users/:id, got %s %q", tokens[1].Type, tokens[1].Value)
	}
}

func TestTokenizeStringLiteral(t *testing.T) {
	src := `import "models/auth.veld"`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if tokens[0].Type != TImport {
		t.Errorf("expected import, got %s", tokens[0].Type)
	}
	if tokens[1].Type != TString || tokens[1].Value != "models/auth.veld" {
		t.Errorf("expected string 'models/auth.veld', got %s %q", tokens[1].Type, tokens[1].Value)
	}
}

func TestTokenizeNumber(t *testing.T) {
	src := `@default(42)`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expectedTypes := []TokenType{TAt, TIdent, TLParen, TNumber, TRParen, TEOF}
	if len(tokens) != len(expectedTypes) {
		t.Fatalf("expected %d tokens, got %d", len(expectedTypes), len(tokens))
	}
	if tokens[3].Value != "42" {
		t.Errorf("expected number '42', got %q", tokens[3].Value)
	}
}

func TestTokenizeLineTracking(t *testing.T) {
	src := "model\n\nUser"
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if tokens[0].Line != 1 {
		t.Errorf("expected line 1 for 'model', got %d", tokens[0].Line)
	}
	if tokens[1].Line != 3 {
		t.Errorf("expected line 3 for 'User', got %d", tokens[1].Line)
	}
}

func TestTokenizeOptionalField(t *testing.T) {
	src := `bio?: string`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := []TokenType{TIdent, TQuestion, TColon, TTypeString, TEOF}
	if len(tokens) != len(expected) {
		t.Fatalf("expected %d tokens, got %d", len(expected), len(tokens))
	}
	for i, et := range expected {
		if tokens[i].Type != et {
			t.Errorf("token[%d]: expected %s, got %s", i, et, tokens[i].Type)
		}
	}
}

func TestTokenizeArraySuffix(t *testing.T) {
	src := `tags: string[]`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := []TokenType{TIdent, TColon, TTypeString, TLBracket, TRBracket, TEOF}
	if len(tokens) != len(expected) {
		t.Fatalf("expected %d tokens, got %d", len(expected), len(tokens))
	}
}

func TestTokenizeUnexpectedChar(t *testing.T) {
	_, err := New("model User { ~ }").Tokenize()
	if err == nil {
		t.Fatal("expected error for unexpected character ~")
	}
}

func TestTokenizeDefaultAnnotation(t *testing.T) {
	src := `role: Role @default(user)`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := []TokenType{TIdent, TColon, TIdent, TAt, TIdent, TLParen, TIdent, TRParen, TEOF}
	if len(tokens) != len(expected) {
		t.Fatalf("expected %d tokens, got %d", len(expected), len(tokens))
	}
	// Verify @default and (user) values
	if tokens[4].Value != "default" {
		t.Errorf("expected 'default', got %q", tokens[4].Value)
	}
	if tokens[6].Value != "user" {
		t.Errorf("expected 'user', got %q", tokens[6].Value)
	}
}

func TestTokenizeBlockComment(t *testing.T) {
	src := `/* this is a block comment */
model User {}`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Should skip the block comment and parse model User {}
	if tokens[0].Type != TModel {
		t.Errorf("expected TModel after block comment, got %s (%q)", tokens[0].Type, tokens[0].Value)
	}
}

func TestTokenizeBlockCommentMultiLine(t *testing.T) {
	src := `/*
  This spans
  multiple lines
*/
model User {}`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tokens[0].Type != TModel {
		t.Errorf("expected TModel, got %s", tokens[0].Type)
	}
	// Line tracking: model should be on line 5
	if tokens[0].Line != 5 {
		t.Errorf("expected line 5, got %d", tokens[0].Line)
	}
}

func TestTokenizeBlockCommentInline(t *testing.T) {
	src := `model /* inline */ User {}`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tokens[0].Type != TModel || tokens[1].Type != TIdent {
		t.Errorf("block comment should be skipped inline")
	}
	if tokens[1].Value != "User" {
		t.Errorf("expected User, got %q", tokens[1].Value)
	}
}

func TestErrorRecovery(t *testing.T) {
	src := "model User { name: string; age: int }"
	lex := New(src)
	tokens, err := lex.Tokenize()
	// The semicolons are unexpected characters
	if err == nil {
		t.Fatal("expected error for unexpected character ';'")
	}
	// But tokens should still be returned (recovery)
	if tokens == nil {
		t.Fatal("tokens should not be nil even with errors")
	}
	// Should have collected errors
	if len(lex.Errors()) == 0 {
		t.Error("Errors() should return collected errors")
	}
}

func TestTokenizeAnnotationKeywords(t *testing.T) {
	// @example, @unique, @index, @relation should all lex as @+ident
	src := `name: string @example("test") @unique @index @relation(User)`
	tokens, err := New(src).Tokenize()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Count @ tokens
	atCount := 0
	for _, tok := range tokens {
		if tok.Type == TAt {
			atCount++
		}
	}
	if atCount != 4 {
		t.Errorf("expected 4 @ tokens, got %d", atCount)
	}
}

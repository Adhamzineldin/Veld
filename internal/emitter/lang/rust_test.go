package lang

import "testing"

func TestRustAdapterMetadata(t *testing.T) {
	a := &RustAdapter{}
	meta := a.Metadata()
	if meta.Name != "rust" {
		t.Errorf("expected name 'rust', got %q", meta.Name)
	}
	if meta.Framework != "axum" {
		t.Errorf("expected framework 'axum', got %q", meta.Framework)
	}
}

func TestRustAdapterMapTypeBuiltins(t *testing.T) {
	a := &RustAdapter{}
	tests := map[string]string{
		"string":   "String",
		"int":      "i64",
		"float":    "f64",
		"bool":     "bool",
		"date":     "String",
		"datetime": "String",
		"uuid":     "String",
		"bytes":    "Vec<u8>",
	}
	for veld, expected := range tests {
		rustType, _, err := a.MapType(veld)
		if err != nil {
			t.Errorf("MapType(%q) error: %v", veld, err)
		}
		if rustType != expected {
			t.Errorf("MapType(%q) = %q, want %q", veld, rustType, expected)
		}
	}
}

func TestRustAdapterMapTypeList(t *testing.T) {
	a := &RustAdapter{}
	rustType, _, err := a.MapType("List<string>")
	if err != nil {
		t.Fatalf("MapType(List<string>) error: %v", err)
	}
	if rustType != "Vec<String>" {
		t.Errorf("MapType(List<string>) = %q, want %q", rustType, "Vec<String>")
	}
}

func TestRustAdapterMapTypeMap(t *testing.T) {
	a := &RustAdapter{}
	rustType, _, err := a.MapType("Map<string, int>")
	if err != nil {
		t.Fatalf("MapType(Map<string, int>) error: %v", err)
	}
	if rustType != "HashMap<String, i64>" {
		t.Errorf("MapType(Map<string, int>) = %q, want %q", rustType, "HashMap<String, i64>")
	}
}

func TestRustAdapterMapTypeCustom(t *testing.T) {
	a := &RustAdapter{}
	rustType, _, err := a.MapType("User")
	if err != nil {
		t.Fatalf("MapType(User) error: %v", err)
	}
	if rustType != "User" {
		t.Errorf("MapType(User) = %q, want %q", rustType, "User")
	}
}

func TestRustAdapterNamingConventions(t *testing.T) {
	a := &RustAdapter{}
	tests := []struct {
		input string
		ctx   NamingContext
		want  string
	}{
		{"userId", NamingContextPrivate, "user_id"},
		{"userId", NamingContextExported, "UserId"},
		{"userId", NamingContextConstant, "USER_ID"},
		{"loginInput", NamingContextExported, "LoginInput"},
		{"get_by_id", NamingContextExported, "GetById"},
	}
	for _, tc := range tests {
		got := a.NamingConvention(tc.input, tc.ctx)
		if got != tc.want {
			t.Errorf("NamingConvention(%q, %d) = %q, want %q", tc.input, tc.ctx, got, tc.want)
		}
	}
}

func TestRustAdapterNullableType(t *testing.T) {
	a := &RustAdapter{}
	if got := a.NullableType("String"); got != "Option<String>" {
		t.Errorf("NullableType(String) = %q, want Option<String>", got)
	}
	if got := a.NullableType("Option<String>"); got != "Option<String>" {
		t.Errorf("NullableType already-optional should be idempotent, got %q", got)
	}
}

func TestRustSnakeCase(t *testing.T) {
	tests := map[string]string{
		"userId":    "user_id",
		"UserID":    "user_id",
		"loginUser": "login_user",
		"id":        "id",
	}
	for input, want := range tests {
		got := rustSnakeCase(input)
		if got != want {
			t.Errorf("rustSnakeCase(%q) = %q, want %q", input, got, want)
		}
	}
}

func TestRustPascalCase(t *testing.T) {
	tests := map[string]string{
		"user_id":    "UserId",
		"login_user": "LoginUser",
		"id":         "Id",
	}
	for input, want := range tests {
		got := rustPascalCase(input)
		if got != want {
			t.Errorf("rustPascalCase(%q) = %q, want %q", input, got, want)
		}
	}
}

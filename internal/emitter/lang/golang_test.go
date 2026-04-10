package lang

import (
	"testing"
)

func TestGoAdapterMetadata(t *testing.T) {
	adapter := &GoAdapter{}
	meta := adapter.Metadata()

	if meta.Name != "go" {
		t.Errorf("expected name 'go', got %q", meta.Name)
	}
	if meta.Framework != "chi" {
		t.Errorf("expected framework 'chi', got %q", meta.Framework)
	}
}

func TestGoAdapterMapTypeBuiltins(t *testing.T) {
	adapter := &GoAdapter{}

	tests := map[string]string{
		"string":   "string",
		"int":      "int",
		"float":    "float64",
		"bool":     "bool",
		"date":     "time.Time",
		"datetime": "time.Time",
		"uuid":     "string",
		"bytes":    "[]byte",
	}

	for veldType, expected := range tests {
		goType, imports, err := adapter.MapType(veldType)
		if err != nil {
			t.Errorf("MapType(%q) returned error: %v", veldType, err)
		}
		if goType != expected {
			t.Errorf("MapType(%q) = %q, want %q", veldType, goType, expected)
		}
		if imports != nil && len(imports) > 0 {
			t.Errorf("MapType(%q) should have no imports, got %v", veldType, imports)
		}
	}
}

func TestGoAdapterMapTypeList(t *testing.T) {
	adapter := &GoAdapter{}

	goType, _, err := adapter.MapType("List<string>")
	if err != nil {
		t.Errorf("MapType(List<string>) failed: %v", err)
	}
	if goType != "[]string" {
		t.Errorf("MapType(List<string>) = %q, want %q", goType, "[]string")
	}
}

func TestGoAdapterMapTypeNestedList(t *testing.T) {
	adapter := &GoAdapter{}

	goType, _, err := adapter.MapType("List<List<int>>")
	if err != nil {
		t.Errorf("MapType(List<List<int>>) failed: %v", err)
	}
	if goType != "[][]int" {
		t.Errorf("MapType(List<List<int>>) = %q, want %q", goType, "[][]int")
	}
}

func TestGoAdapterMapTypeMap(t *testing.T) {
	adapter := &GoAdapter{}

	goType, _, err := adapter.MapType("Map<string, int>")
	if err != nil {
		t.Errorf("MapType(Map<string, int>) failed: %v", err)
	}
	if goType != "map[string]int" {
		t.Errorf("MapType(Map<string, int>) = %q, want %q", goType, "map[string]int")
	}
}

func TestGoAdapterMapTypeCustom(t *testing.T) {
	adapter := &GoAdapter{}

	// Custom types pass through unchanged
	goType, _, err := adapter.MapType("User")
	if err != nil {
		t.Errorf("MapType(User) failed: %v", err)
	}
	if goType != "User" {
		t.Errorf("MapType(User) = %q, want %q", goType, "User")
	}
}

func TestGoAdapterNamingContextExported(t *testing.T) {
	adapter := &GoAdapter{}

	tests := map[string]string{
		"user_id":   "UserId",
		"userId":    "UserId",
		"id":        "Id",
		"http_code": "HttpCode",
	}

	for input, expected := range tests {
		result := adapter.NamingConvention(input, NamingContextExported)
		if result != expected {
			t.Errorf("NamingConvention(%q, Exported) = %q, want %q", input, result, expected)
		}
	}
}

func TestGoAdapterNamingContextPrivate(t *testing.T) {
	adapter := &GoAdapter{}

	tests := map[string]string{
		"user_id":   "userId",
		"UserId":    "userId",
		"id":        "id",
		"http_code": "httpCode",
	}

	for input, expected := range tests {
		result := adapter.NamingConvention(input, NamingContextPrivate)
		if result != expected {
			t.Errorf("NamingConvention(%q, Private) = %q, want %q", input, result, expected)
		}
	}
}

func TestGoAdapterNamingContextConstant(t *testing.T) {
	adapter := &GoAdapter{}

	tests := map[string]string{
		"user_id":   "USER_ID",
		"UserId":    "USER_ID",
		"id":        "ID",
		"http_code": "HTTP_CODE",
	}

	for input, expected := range tests {
		result := adapter.NamingConvention(input, NamingContextConstant)
		if result != expected {
			t.Errorf("NamingConvention(%q, Constant) = %q, want %q", input, result, expected)
		}
	}
}

func TestGoAdapterStructFieldTag(t *testing.T) {
	adapter := &GoAdapter{}

	tag := adapter.StructFieldTag("userId", "string")
	expected := "`json:\"userId\"`"
	if tag != expected {
		t.Errorf("StructFieldTag = %q, want %q", tag, expected)
	}
}

func TestGoAdapterImportStatement(t *testing.T) {
	adapter := &GoAdapter{}

	imp := adapter.ImportStatement("github.com/go-chi/chi/v5", "")
	if imp != "import \"github.com/go-chi/chi/v5\"" {
		t.Errorf("ImportStatement without alias = %q", imp)
	}

	imp = adapter.ImportStatement("github.com/go-chi/chi/v5", "chi")
	if imp != "import chi \"github.com/go-chi/chi/v5\"" {
		t.Errorf("ImportStatement with alias = %q", imp)
	}
}

func TestGoAdapterCommentSyntax(t *testing.T) {
	adapter := &GoAdapter{}

	style := adapter.CommentSyntax()
	if style.Single != "//" {
		t.Errorf("expected single comment //, got %q", style.Single)
	}
	if style.Multi != "/*" {
		t.Errorf("expected multi comment /*, got %q", style.Multi)
	}
}

func TestGoAdapterFileExtension(t *testing.T) {
	adapter := &GoAdapter{}

	ext := adapter.FileExtension()
	if ext != ".go" {
		t.Errorf("expected extension .go, got %q", ext)
	}
}

func TestGoAdapterNullableType(t *testing.T) {
	adapter := &GoAdapter{}

	nullable := adapter.NullableType("string")
	if nullable != "*string" {
		t.Errorf("NullableType(string) = %q, want *string", nullable)
	}

	nullable = adapter.NullableType("User")
	if nullable != "*User" {
		t.Errorf("NullableType(User) = %q, want *User", nullable)
	}

	// Already nullable
	nullable = adapter.NullableType("*string")
	if nullable != "*string" {
		t.Errorf("NullableType(*string) = %q, want *string", nullable)
	}
}

func TestToSnakeCase(t *testing.T) {
	tests := map[string]string{
		"userId":     "user_id",
		"UserID":     "user_id",
		"id":         "id",
		"user_id":    "user_id",
		"getHTTPURL": "get_httpurl", // consecutive caps flatten to lowercase
		"HTTPCode":   "http_code",   // improved algo: detects H->ttp transition
		"getUserID":  "get_user_id",
	}

	for input, expected := range tests {
		result := toSnakeCase(input)
		if result != expected {
			t.Errorf("toSnakeCase(%q) = %q, want %q", input, result, expected)
		}
	}
}

func TestToCamelCase(t *testing.T) {
	tests := map[string]string{
		"user_id":   "userId",
		"http_code": "httpCode",
		"id":        "id",
		"userId":    "userId",
		"user__id":  "userId",
		"_user_id":  "userId",
	}

	for input, expected := range tests {
		result := toCamelCase(input)
		if result != expected {
			t.Errorf("toCamelCase(%q) = %q, want %q", input, result, expected)
		}
	}
}

func TestToPascalCase(t *testing.T) {
	tests := map[string]string{
		"user_id":   "UserId",
		"http_code": "HttpCode",
		"id":        "Id",
		"userId":    "UserId",
		"user__id":  "UserId",
	}

	for input, expected := range tests {
		result := toPascalCase(input)
		if result != expected {
			t.Errorf("toPascalCase(%q) = %q, want %q", input, result, expected)
		}
	}
}

func TestToShoutySnakeCase(t *testing.T) {
	tests := map[string]string{
		"userId":   "USER_ID",
		"httpCode": "HTTP_CODE",
		"id":       "ID",
		"user_id":  "USER_ID",
	}

	for input, expected := range tests {
		result := toShoutySnakeCase(input)
		if result != expected {
			t.Errorf("toShoutySnakeCase(%q) = %q, want %q", input, result, expected)
		}
	}
}

func TestTypeNeedsPointer(t *testing.T) {
	tests := map[string]bool{
		"string":    false, // no pointer
		"int64":     false,
		"float64":   false,
		"bool":      false,
		"time.Time": false,
		"User":      true, // custom types need pointer
		"[]string":  false,
	}

	for typeName, expected := range tests {
		result := TypeNeedsPointer(typeName)
		if result != expected {
			t.Errorf("TypeNeedsPointer(%q) = %v, want %v", typeName, result, expected)
		}
	}
}

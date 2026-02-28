package lang_test

import (
	"testing"

	"github.com/veld-dev/veld/internal/emitter/lang"
)

func TestPhpAdapterMapType_Builtins(t *testing.T) {
	a := &lang.PhpAdapter{}
	cases := []struct{ veld, want string }{
		{"string", "string"},
		{"int", "int"},
		{"float", "float"},
		{"bool", "bool"},
		{"date", "string"},
		{"datetime", "string"},
		{"uuid", "string"},
	}
	for _, c := range cases {
		got, _, err := a.MapType(c.veld)
		if err != nil {
			t.Errorf("MapType(%q) error: %v", c.veld, err)
		}
		if got != c.want {
			t.Errorf("MapType(%q) = %q, want %q", c.veld, got, c.want)
		}
	}
}

func TestPhpAdapterMapType_List(t *testing.T) {
	a := &lang.PhpAdapter{}
	got, docs, err := a.MapType("List<string>")
	if err != nil {
		t.Fatalf("MapType(List<string>) error: %v", err)
	}
	if got != "array" {
		t.Errorf("got %q, want array", got)
	}
	if len(docs) == 0 || docs[0] != "@var string[]" {
		t.Errorf("expected PHPDoc @var string[], got %v", docs)
	}
}

func TestPhpAdapterMapType_Map(t *testing.T) {
	a := &lang.PhpAdapter{}
	got, docs, err := a.MapType("Map<string, int>")
	if err != nil {
		t.Fatalf("MapType(Map<string,int>) error: %v", err)
	}
	if got != "array" {
		t.Errorf("got %q, want array", got)
	}
	if len(docs) == 0 || docs[0] != "@var array<string, int>" {
		t.Errorf("expected PHPDoc @var array<string, int>, got %v", docs)
	}
}

func TestPhpAdapterNamingConvention(t *testing.T) {
	a := &lang.PhpAdapter{}
	cases := []struct {
		name    string
		context lang.NamingContext
		want    string
	}{
		{"authService", lang.NamingContextExported, "AuthService"},
		{"userName", lang.NamingContextPrivate, "user_name"},
		{"maxItems", lang.NamingContextConstant, "MAX_ITEMS"},
		{"App\\Http", lang.NamingContextPackage, "App\\Http"}, // namespaces stay as-is via PascalCase
	}
	for _, c := range cases {
		got := a.NamingConvention(c.name, c.context)
		if got != c.want {
			t.Errorf("NamingConvention(%q, %v) = %q, want %q", c.name, c.context, got, c.want)
		}
	}
}

func TestPhpAdapterNullableType(t *testing.T) {
	a := &lang.PhpAdapter{}
	if got := a.NullableType("string"); got != "?string" {
		t.Errorf("NullableType(string) = %q, want ?string", got)
	}
	if got := a.NullableType("?int"); got != "?int" {
		t.Errorf("NullableType(?int) should be idempotent, got %q", got)
	}
}

func TestPhpAdapterFileExtension(t *testing.T) {
	a := &lang.PhpAdapter{}
	if got := a.FileExtension(); got != ".php" {
		t.Errorf("FileExtension() = %q, want .php", got)
	}
}

func TestPhpAdapterMetadata(t *testing.T) {
	a := &lang.PhpAdapter{}
	m := a.Metadata()
	if m.Name != "php" {
		t.Errorf("Metadata.Name = %q, want php", m.Name)
	}
	if m.Framework != "laravel" {
		t.Errorf("Metadata.Framework = %q, want laravel", m.Framework)
	}
}

func TestPhpAdapterImportStatement(t *testing.T) {
	a := &lang.PhpAdapter{}
	if got := a.ImportStatement("App\\Models\\User", ""); got != "use App\\Models\\User;" {
		t.Errorf("ImportStatement = %q", got)
	}
	if got := a.ImportStatement("App\\Models\\User", "UserModel"); got != "use App\\Models\\User as UserModel;" {
		t.Errorf("ImportStatement with alias = %q", got)
	}
}

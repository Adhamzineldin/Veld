package lang_test

import (
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/emitter/lang"
)

func TestCSharpAdapterMapType_Builtins(t *testing.T) {
	a := &lang.CSharpAdapter{}
	cases := []struct{ veld, want string }{
		{"string", "string"},
		{"int", "long"},
		{"float", "double"},
		{"bool", "bool"},
		{"date", "DateTime"},
		{"datetime", "DateTime"},
		{"uuid", "Guid"},
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

func TestCSharpAdapterMapType_List(t *testing.T) {
	a := &lang.CSharpAdapter{}
	got, imports, err := a.MapType("List<string>")
	if err != nil {
		t.Fatalf("MapType(List<string>) error: %v", err)
	}
	if got != "List<string>" {
		t.Errorf("got %q, want %q", got, "List<string>")
	}
	found := false
	for _, imp := range imports {
		if strings.Contains(imp, "Generic") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected System.Collections.Generic import, got %v", imports)
	}
}

func TestCSharpAdapterMapType_Dictionary(t *testing.T) {
	a := &lang.CSharpAdapter{}
	got, _, err := a.MapType("Map<string, bool>")
	if err != nil {
		t.Fatalf("MapType(Map<string,bool>) error: %v", err)
	}
	if got != "Dictionary<string, bool>" {
		t.Errorf("got %q, want %q", got, "Dictionary<string, bool>")
	}
}

func TestCSharpAdapterNamingConvention(t *testing.T) {
	a := &lang.CSharpAdapter{}
	cases := []struct {
		name    string
		context lang.NamingContext
		want    string
	}{
		{"loginUser", lang.NamingContextExported, "LoginUser"},
		{"myField", lang.NamingContextPrivate, "_myField"},
		{"loginUser", lang.NamingContextConstant, "LoginUser"}, // C# uses PascalCase for constants
	}
	for _, c := range cases {
		got := a.NamingConvention(c.name, c.context)
		if got != c.want {
			t.Errorf("NamingConvention(%q, %v) = %q, want %q", c.name, c.context, got, c.want)
		}
	}
}

func TestCSharpAdapterNullableType(t *testing.T) {
	a := &lang.CSharpAdapter{}
	if got := a.NullableType("long"); got != "long?" {
		t.Errorf("NullableType(long) = %q, want long?", got)
	}
	if got := a.NullableType("string"); got != "string?" {
		t.Errorf("NullableType(string) = %q, want string?", got)
	}
}

func TestCSharpAdapterFileExtension(t *testing.T) {
	a := &lang.CSharpAdapter{}
	if got := a.FileExtension(); got != ".cs" {
		t.Errorf("FileExtension() = %q, want .cs", got)
	}
}

func TestCSharpAdapterMetadata(t *testing.T) {
	a := &lang.CSharpAdapter{}
	m := a.Metadata()
	if m.Name != "csharp" {
		t.Errorf("Metadata.Name = %q, want csharp", m.Name)
	}
	if m.Framework != "aspnet-core" {
		t.Errorf("Metadata.Framework = %q, want aspnet-core", m.Framework)
	}
}

func TestCSharpAdapterImportStatement(t *testing.T) {
	a := &lang.CSharpAdapter{}
	if got := a.ImportStatement("System.Collections.Generic", ""); got != "using System.Collections.Generic;" {
		t.Errorf("ImportStatement = %q", got)
	}
	if got := a.ImportStatement("System.Text.Json", "Json"); got != "using Json = System.Text.Json;" {
		t.Errorf("ImportStatement with alias = %q", got)
	}
}

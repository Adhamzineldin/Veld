package lang_test

import (
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/emitter/lang"
)

func TestJavaAdapterMapType_Builtins(t *testing.T) {
	a := &lang.JavaAdapter{}
	cases := []struct{ veld, want string }{
		{"string", "String"},
		{"int", "Long"},
		{"float", "Double"},
		{"bool", "Boolean"},
		{"date", "String"},
		{"datetime", "String"},
		{"uuid", "UUID"}, // java.util.UUID — imported automatically by the emitter
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

func TestJavaAdapterMapType_List(t *testing.T) {
	a := &lang.JavaAdapter{}
	got, imports, err := a.MapType("List<string>")
	if err != nil {
		t.Fatalf("MapType(List<string>) error: %v", err)
	}
	if got != "List<String>" {
		t.Errorf("got %q, want %q", got, "List<String>")
	}
	found := false
	for _, imp := range imports {
		if strings.Contains(imp, "java.util.List") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected java.util.List import, got %v", imports)
	}
}

func TestJavaAdapterMapType_Map(t *testing.T) {
	a := &lang.JavaAdapter{}
	got, imports, err := a.MapType("Map<string, int>")
	if err != nil {
		t.Fatalf("MapType(Map<string,int>) error: %v", err)
	}
	if got != "Map<String, Long>" {
		t.Errorf("got %q, want %q", got, "Map<String, Long>")
	}
	found := false
	for _, imp := range imports {
		if strings.Contains(imp, "java.util.Map") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected java.util.Map import, got %v", imports)
	}
}

func TestJavaAdapterMapType_CustomType(t *testing.T) {
	a := &lang.JavaAdapter{}
	got, _, err := a.MapType("User")
	if err != nil {
		t.Fatalf("MapType(User) error: %v", err)
	}
	if got != "User" {
		t.Errorf("got %q, want %q", got, "User")
	}
}

func TestJavaAdapterNamingConvention(t *testing.T) {
	a := &lang.JavaAdapter{}
	cases := []struct {
		name    string
		context lang.NamingContext
		want    string
	}{
		{"getUserById", lang.NamingContextExported, "GetUserById"},
		{"getUserById", lang.NamingContextPrivate, "getUserById"},
		{"maxRetries", lang.NamingContextConstant, "MAX_RETRIES"},
		{"AuthService", lang.NamingContextPackage, "authservice"},
	}
	for _, c := range cases {
		got := a.NamingConvention(c.name, c.context)
		if got != c.want {
			t.Errorf("NamingConvention(%q, %v) = %q, want %q", c.name, c.context, got, c.want)
		}
	}
}

func TestJavaAdapterNullableType(t *testing.T) {
	a := &lang.JavaAdapter{}
	if got := a.NullableType("long"); got != "Long" {
		t.Errorf("NullableType(long) = %q, want Long", got)
	}
	if got := a.NullableType("String"); got != "String" {
		t.Errorf("NullableType(String) = %q, want String (unchanged)", got)
	}
}

func TestJavaAdapterMetadata(t *testing.T) {
	a := &lang.JavaAdapter{}
	m := a.Metadata()
	if m.Name != "java" {
		t.Errorf("Metadata.Name = %q, want java", m.Name)
	}
	if m.Framework != "spring-boot" {
		t.Errorf("Metadata.Framework = %q, want spring-boot", m.Framework)
	}
}

func TestJavaAdapterFileExtension(t *testing.T) {
	a := &lang.JavaAdapter{}
	if got := a.FileExtension(); got != ".java" {
		t.Errorf("FileExtension() = %q, want .java", got)
	}
}

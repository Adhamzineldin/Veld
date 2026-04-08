package lang

import (
	"fmt"
	"strings"
	"unicode"
)

// CSharpAdapter implements LanguageAdapter for C# (ASP.NET Core, .NET 8+) code generation.
// Generated code uses C# records for models, interfaces for service contracts, and
// ASP.NET Core controllers for route handlers.
type CSharpAdapter struct{}

// Metadata returns C# language metadata.
func (a *CSharpAdapter) Metadata() LanguageMetadata {
	return LanguageMetadata{
		Name:              "csharp",
		Version:           "12 (.NET 8+)",
		Runtime:           "CLR",
		Framework:         "aspnet-core",
		Features:          []string{"records", "nullable-refs", "async/await"},
		ExportPath:        "Generated",
		ImportPaths:       []string{"Microsoft.AspNetCore", "System.Collections.Generic"},
		TypeMapperVersion: "1.0",
	}
}

// MapType converts Veld types to C# types.
func (a *CSharpAdapter) MapType(veldType string) (string, []string, error) {
	veldType = strings.TrimSpace(veldType)

	builtins := map[string]string{
		"string":   "string",
		"int":      "long",
		"float":    "double",
		"decimal":  "decimal",
		"bool":     "bool",
		"date":     "DateTime",
		"datetime": "DateTime",
		"uuid":     "Guid",
		"bytes":    "byte[]",
		"any":      "object",
		"json":     "object",
	}

	if csType, ok := builtins[veldType]; ok {
		return csType, nil, nil
	}

	// List<T> → List<T>
	if strings.HasPrefix(veldType, "List<") && strings.HasSuffix(veldType, ">") {
		inner := veldType[5 : len(veldType)-1]
		innerType, innerImports, err := a.MapType(inner)
		if err != nil {
			return "", nil, err
		}
		imports := append([]string{"System.Collections.Generic"}, innerImports...)
		return "List<" + innerType + ">", imports, nil
	}

	// Map<string, V> → Dictionary<string, V>
	if strings.HasPrefix(veldType, "Map<") && strings.HasSuffix(veldType, ">") {
		content := veldType[4 : len(veldType)-1]
		parts := strings.SplitN(content, ",", 2)
		if len(parts) != 2 {
			return "", nil, fmt.Errorf("invalid Map type: %s", veldType)
		}
		keyType := strings.TrimSpace(parts[0])
		if keyType != "string" {
			return "", nil, fmt.Errorf("Map key type must be 'string', got: %s", keyType)
		}
		valCSType, valImports, err := a.MapType(strings.TrimSpace(parts[1]))
		if err != nil {
			return "", nil, err
		}
		imports := append([]string{"System.Collections.Generic"}, valImports...)
		return "Dictionary<string, " + valCSType + ">", imports, nil
	}

	// Custom types pass through as-is.
	return veldType, nil, nil
}

// NamingConvention converts names to C# conventions.
// C# uses PascalCase for virtually everything public (methods, properties, classes).
func (a *CSharpAdapter) NamingConvention(name string, context NamingContext) string {
	switch context {
	case NamingContextPrivate:
		return "_" + csharpCamelCase(name) // _camelCase for private fields
	case NamingContextDatabase:
		return csharpSnakeCase(name)
	default:
		return csharpPascalCase(name) // PascalCase for everything else
	}
}

// StructFieldTag for C#: properties use [JsonPropertyName] from System.Text.Json.
func (a *CSharpAdapter) StructFieldTag(fieldName string, _ string) string {
	pascal := csharpPascalCase(fieldName)
	if pascal != fieldName {
		return fmt.Sprintf(`[JsonPropertyName("%s")]`, fieldName)
	}
	return ""
}

// ImportStatement generates a C# using directive.
func (a *CSharpAdapter) ImportStatement(module string, alias string) string {
	if alias != "" {
		return fmt.Sprintf("using %s = %s;", alias, module)
	}
	return fmt.Sprintf("using %s;", module)
}

// CommentSyntax returns C# comment style.
func (a *CSharpAdapter) CommentSyntax() CommentStyle {
	return CommentStyle{Single: "//", Multi: "/*", MultiEnd: "*/"}
}

// FileExtension returns .cs.
func (a *CSharpAdapter) FileExtension() string { return ".cs" }

// NullableType appends ? to value types; reference types use nullable annotation.
func (a *CSharpAdapter) NullableType(baseType string) string {
	switch baseType {
	case "long", "double", "bool", "int", "float", "decimal":
		return baseType + "?" // nullable value type
	}
	return baseType + "?" // nullable reference type annotation
}

// ── case helpers ──────────────────────────────────────────────────────────────

func csharpSnakeCase(s string) string {
	var result strings.Builder
	var prev rune
	for i, r := range s {
		if unicode.IsUpper(r) && i > 0 && !unicode.IsUpper(prev) && prev != '_' {
			result.WriteRune('_')
		}
		result.WriteRune(unicode.ToLower(r))
		prev = r
	}
	out := result.String()
	for strings.Contains(out, "__") {
		out = strings.ReplaceAll(out, "__", "_")
	}
	return strings.Trim(out, "_")
}

func csharpCamelCase(s string) string {
	parts := strings.FieldsFunc(csharpSnakeCase(s), func(r rune) bool { return r == '_' })
	if len(parts) == 0 {
		return s
	}
	var result strings.Builder
	result.WriteString(parts[0])
	for _, p := range parts[1:] {
		if len(p) > 0 {
			result.WriteString(strings.ToUpper(p[:1]) + p[1:])
		}
	}
	return result.String()
}

func csharpPascalCase(s string) string {
	parts := strings.FieldsFunc(csharpSnakeCase(s), func(r rune) bool { return r == '_' })
	var result strings.Builder
	for _, p := range parts {
		if len(p) > 0 {
			result.WriteString(strings.ToUpper(p[:1]) + p[1:])
		}
	}
	return result.String()
}

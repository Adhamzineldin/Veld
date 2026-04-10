package lang

import (
	"fmt"
	"strings"
	"unicode"
)

// RustAdapter implements LanguageAdapter for Rust code generation.
// Generated code targets Axum (tokio async HTTP framework) and uses Serde for JSON.
type RustAdapter struct{}

// Metadata returns Rust language metadata.
func (a *RustAdapter) Metadata() LanguageMetadata {
	return LanguageMetadata{
		Name:              "rust",
		Version:           "1.75+",
		Runtime:           "compiled",
		Framework:         "axum",
		Features:          []string{"async", "serde", "error-handling"},
		ExportPath:        "src",
		ImportPaths:       []string{"axum", "serde", "tokio"},
		TypeMapperVersion: "1.0",
	}
}

// MapType converts Veld types to Rust types.
func (a *RustAdapter) MapType(veldType string) (string, []string, error) {
	veldType = strings.TrimSpace(veldType)

	builtins := map[string]string{
		"string":   "String",
		"int":      "i32",
		"float":    "f64",
		"decimal":  "String",
		"bool":     "bool",
		"date":     "String", // ISO date string; use chrono crate for full date support
		"datetime": "String", // ISO datetime string
		"uuid":     "String", // UUID as string; use uuid crate for typed UUIDs
		"bytes":    "Vec<u8>",
		"any":      "serde_json::Value",
		"json":     "serde_json::Value",
	}

	if rustType, ok := builtins[veldType]; ok {
		return rustType, nil, nil
	}

	// List<T> → Vec<T>
	if strings.HasPrefix(veldType, "List<") && strings.HasSuffix(veldType, ">") {
		inner := veldType[5 : len(veldType)-1]
		innerType, innerImports, err := a.MapType(inner)
		if err != nil {
			return "", nil, err
		}
		return "Vec<" + innerType + ">", innerImports, nil
	}

	// Map<string, V> → HashMap<String, V>
	if strings.HasPrefix(veldType, "Map<") && strings.HasSuffix(veldType, ">") {
		content := veldType[4 : len(veldType)-1]
		parts := strings.SplitN(content, ",", 2)
		if len(parts) != 2 {
			return "", nil, fmt.Errorf("invalid Map type: %s", veldType)
		}
		keyType := strings.TrimSpace(parts[0])
		valType := strings.TrimSpace(parts[1])

		if keyType != "string" {
			return "", nil, fmt.Errorf("Map key type must be 'string', got: %s", keyType)
		}

		valRustType, valImports, err := a.MapType(valType)
		if err != nil {
			return "", nil, err
		}
		return "HashMap<String, " + valRustType + ">", valImports, nil
	}

	// Custom types (model names) pass through as-is.
	return veldType, nil, nil
}

// NamingConvention converts names to Rust conventions.
func (a *RustAdapter) NamingConvention(name string, context NamingContext) string {
	switch context {
	case NamingContextExported:
		return rustPascalCase(name)
	case NamingContextPrivate, NamingContextDatabase:
		return rustSnakeCase(name)
	case NamingContextConstant:
		return strings.ToUpper(rustSnakeCase(name))
	case NamingContextPackage:
		return rustSnakeCase(name)
	default:
		return rustSnakeCase(name)
	}
}

// StructFieldTag for Rust: serde uses derive macros, not field tags.
// We return the serde rename attribute when the JSON key differs from the field name.
func (a *RustAdapter) StructFieldTag(fieldName string, fieldType string) string {
	snakeName := rustSnakeCase(fieldName)
	if snakeName != fieldName {
		return fmt.Sprintf(`#[serde(rename = "%s")]`, fieldName)
	}
	return ""
}

// ImportStatement generates a Rust use statement.
func (a *RustAdapter) ImportStatement(module string, alias string) string {
	if alias != "" {
		return fmt.Sprintf("use %s as %s;", module, alias)
	}
	return fmt.Sprintf("use %s;", module)
}

// CommentSyntax returns Rust comment style (same as Go and C).
func (a *RustAdapter) CommentSyntax() CommentStyle {
	return CommentStyle{
		Single:   "//",
		Multi:    "/*",
		MultiEnd: "*/",
	}
}

// FileExtension returns .rs.
func (a *RustAdapter) FileExtension() string {
	return ".rs"
}

// NullableType returns the Rust optional type.
func (a *RustAdapter) NullableType(baseType string) string {
	if strings.HasPrefix(baseType, "Option<") {
		return baseType // already optional
	}
	return "Option<" + baseType + ">"
}

// ── case helpers ──────────────────────────────────────────────────────────────

// rustSnakeCase converts any name to snake_case for Rust.
func rustSnakeCase(s string) string {
	var result strings.Builder
	var prev rune

	for i, r := range s {
		isUpper := unicode.IsUpper(r)
		if isUpper && i > 0 && !unicode.IsUpper(prev) && prev != '_' {
			result.WriteRune('_')
		}
		result.WriteRune(unicode.ToLower(r))
		prev = r
	}

	// Collapse consecutive underscores and trim.
	s = result.String()
	for strings.Contains(s, "__") {
		s = strings.ReplaceAll(s, "__", "_")
	}
	return strings.Trim(s, "_")
}

// rustPascalCase converts any name to PascalCase for Rust struct/enum names.
func rustPascalCase(s string) string {
	parts := strings.FieldsFunc(rustSnakeCase(s), func(r rune) bool { return r == '_' })
	var result strings.Builder
	for _, part := range parts {
		if len(part) > 0 {
			result.WriteString(strings.ToUpper(part[:1]) + part[1:])
		}
	}
	return result.String()
}

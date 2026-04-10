package lang

import (
	"fmt"
	"strings"
	"unicode"
)

// PhpAdapter implements LanguageAdapter for PHP (Laravel, PHP 8.1+) code generation.
// Generated code uses readonly classes for models, interfaces for service contracts,
// and Laravel controllers + route files for HTTP handling.
type PhpAdapter struct{}

// Metadata returns PHP language metadata.
func (a *PhpAdapter) Metadata() LanguageMetadata {
	return LanguageMetadata{
		Name:              "php",
		Version:           "8.1+",
		Runtime:           "PHP-FPM",
		Framework:         "laravel",
		Features:          []string{"readonly-classes", "backed-enums", "named-args"},
		ExportPath:        "app",
		ImportPaths:       []string{"App\\Models", "App\\Services", "App\\Http\\Controllers"},
		TypeMapperVersion: "1.0",
	}
}

// MapType converts Veld types to PHP types (used in type hints and PHPDoc).
func (a *PhpAdapter) MapType(veldType string) (string, []string, error) {
	veldType = strings.TrimSpace(veldType)

	builtins := map[string]string{
		"string":   "string",
		"int":      "int",
		"float":    "float",
		"decimal":  "string",
		"bool":     "bool",
		"date":     "string",
		"datetime": "string",
		"uuid":     "string",
		"long":     "int",
		"bytes":    "string",
		"time":     "string",
		"any":      "mixed",
		"json":     "array",
	}

	if phpType, ok := builtins[veldType]; ok {
		return phpType, nil, nil
	}

	// List<T> → array (with PHPDoc @var T[])
	if strings.HasPrefix(veldType, "List<") && strings.HasSuffix(veldType, ">") {
		inner := veldType[5 : len(veldType)-1]
		innerType, _, err := a.MapType(inner)
		if err != nil {
			return "", nil, err
		}
		return "array", []string{fmt.Sprintf("@var %s[]", innerType)}, nil
	}

	// Map<string, V> → array (with PHPDoc @var array<string, V>)
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
		valPhpType, _, err := a.MapType(strings.TrimSpace(parts[1]))
		if err != nil {
			return "", nil, err
		}
		return "array", []string{fmt.Sprintf("@var array<string, %s>", valPhpType)}, nil
	}

	// Custom types pass through as-is.
	return veldType, nil, nil
}

// NamingConvention converts names to PHP conventions.
func (a *PhpAdapter) NamingConvention(name string, context NamingContext) string {
	switch context {
	case NamingContextExported:
		return phpPascalCase(name) // Classes, interfaces, enums
	case NamingContextPrivate, NamingContextDatabase:
		return phpSnakeCase(name) // Properties, variables (PSR-12: camelCase, but Laravel uses snake_case for DB)
	case NamingContextConstant:
		return strings.ToUpper(phpSnakeCase(name))
	case NamingContextPackage:
		return phpPascalCase(name) // PHP namespaces are PascalCase
	default:
		return phpCamelCase(name) // methods: camelCase per PSR-12
	}
}

// StructFieldTag for PHP: properties use PHPDoc annotations.
func (a *PhpAdapter) StructFieldTag(fieldName string, fieldType string) string {
	return fmt.Sprintf("@var %s", fieldType)
}

// ImportStatement generates a PHP use statement.
func (a *PhpAdapter) ImportStatement(module string, alias string) string {
	if alias != "" {
		return fmt.Sprintf("use %s as %s;", module, alias)
	}
	return fmt.Sprintf("use %s;", module)
}

// CommentSyntax returns PHP comment style.
func (a *PhpAdapter) CommentSyntax() CommentStyle {
	return CommentStyle{Single: "//", Multi: "/*", MultiEnd: "*/"}
}

// FileExtension returns .php.
func (a *PhpAdapter) FileExtension() string { return ".php" }

// NullableType wraps a PHP type with the nullable prefix (?).
func (a *PhpAdapter) NullableType(baseType string) string {
	if strings.HasPrefix(baseType, "?") {
		return baseType // already nullable
	}
	return "?" + baseType
}

// ── case helpers ──────────────────────────────────────────────────────────────

func phpSnakeCase(s string) string {
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

func phpCamelCase(s string) string {
	parts := strings.FieldsFunc(phpSnakeCase(s), func(r rune) bool { return r == '_' })
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

func phpPascalCase(s string) string {
	parts := strings.FieldsFunc(phpSnakeCase(s), func(r rune) bool { return r == '_' })
	var result strings.Builder
	for _, p := range parts {
		if len(p) > 0 {
			result.WriteString(strings.ToUpper(p[:1]) + p[1:])
		}
	}
	return result.String()
}

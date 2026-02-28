package lang

import (
	"fmt"
	"strings"
	"unicode"
)

// GoAdapter implements LanguageAdapter for Go code generation.
type GoAdapter struct{}

// Metadata returns Go language metadata.
func (a *GoAdapter) Metadata() LanguageMetadata {
	return LanguageMetadata{
		Name:              "go",
		Version:           "1.21+",
		Runtime:           "compiled",
		Framework:         "chi",
		Features:          []string{"async", "middleware", "error-handling"},
		ExportPath:        ".",
		ImportPaths:       []string{"github.com/go-chi/chi/v5"},
		TypeMapperVersion: "1.0",
	}
}

// MapType converts Veld types to Go types.
func (a *GoAdapter) MapType(veldType string) (string, []string, error) {
	veldType = strings.TrimSpace(veldType)

	// Built-in types
	builtins := map[string]string{
		"string":   "string",
		"int":      "int64",
		"float":    "float64",
		"bool":     "bool",
		"date":     "time.Time",
		"datetime": "time.Time",
		"time":     "time.Time",
		"uuid":     "string",
		"bytes":    "[]byte",
		"json":     "map[string]interface{}",
		"any":      "interface{}",
	}

	if goType, ok := builtins[veldType]; ok {
		return goType, nil, nil
	}

	// List<T>
	if strings.HasPrefix(veldType, "List<") && strings.HasSuffix(veldType, ">") {
		inner := veldType[5 : len(veldType)-1]
		innerType, innerImports, err := a.MapType(inner)
		if err != nil {
			return "", nil, err
		}
		return "[]" + innerType, innerImports, nil
	}

	// Map<K, V>
	if strings.HasPrefix(veldType, "Map<") && strings.HasSuffix(veldType, ">") {
		content := veldType[4 : len(veldType)-1]
		parts := strings.SplitN(content, ",", 2)
		if len(parts) != 2 {
			return "", nil, fmt.Errorf("invalid Map type: %s", veldType)
		}
		keyType := strings.TrimSpace(parts[0])
		valType := strings.TrimSpace(parts[1])

		// Only string keys supported
		if keyType != "string" {
			return "", nil, fmt.Errorf("Map key type must be 'string', got: %s", keyType)
		}

		valGoType, valImports, err := a.MapType(valType)
		if err != nil {
			return "", nil, err
		}
		return "map[string]" + valGoType, valImports, nil
	}

	// Custom types (models) are passed through as-is
	return veldType, nil, nil
}

// NamingConvention converts names to Go conventions.
// PascalCase for exports, camelCase for private.
func (a *GoAdapter) NamingConvention(name string, context NamingContext) string {
	clean := toSnakeCase(name)

	switch context {
	case NamingContextExported:
		return toPascalCase(clean)
	case NamingContextPrivate:
		return toCamelCase(clean)
	case NamingContextConstant:
		return toShoutySnakeCase(clean)
	case NamingContextPackage, NamingContextDatabase:
		return clean
	default:
		return clean
	}
}

// StructFieldTag generates JSON struct field tags for Go.
func (a *GoAdapter) StructFieldTag(fieldName string, fieldType string) string {
	return fmt.Sprintf("`json:\"%s\"`", fieldName)
}

// ImportStatement generates Go import syntax.
func (a *GoAdapter) ImportStatement(module string, alias string) string {
	if alias != "" {
		return fmt.Sprintf("import %s \"%s\"", alias, module)
	}
	return fmt.Sprintf("import \"%s\"", module)
}

// CommentSyntax returns Go comment syntax.
func (a *GoAdapter) CommentSyntax() CommentStyle {
	return CommentStyle{
		Single:   "//",
		Multi:    "/*",
		MultiEnd: "*/",
	}
}

// FileExtension returns Go file extension.
func (a *GoAdapter) FileExtension() string {
	return ".go"
}

// NullableType returns Go's nullable type representation.
// Uses pointers for nullable types.
func (a *GoAdapter) NullableType(baseType string) string {
	if strings.HasPrefix(baseType, "*") {
		return baseType // already nullable
	}
	return "*" + baseType
}

// Helper functions for case conversion

// toSnakeCase converts a name to snake_case.
// Handles camelCase, PascalCase, and SCREAMING_SNAKE_CASE inputs.
func toSnakeCase(s string) string {
	var result strings.Builder
	var prev rune
	var prevUpper bool

	for i, r := range s {
		isUpper := unicode.IsUpper(r)

		// Insert underscore before uppercase letter when:
		// 1. Previous char was lowercase, OR
		// 2. Previous char was uppercase but next char is lowercase (e.g., "HTTPCode" -> "HTTP_Code")
		if isUpper && i > 0 && (unicode.IsLower(prev) || (prevUpper && i+1 < len(s) && unicode.IsLower(rune(s[i+1])))) {
			result.WriteRune('_')
		}

		result.WriteRune(unicode.ToLower(r))
		prev = r
		prevUpper = isUpper
	}

	// Clean up multiple underscores
	s = result.String()
	for strings.Contains(s, "__") {
		s = strings.ReplaceAll(s, "__", "_")
	}
	return strings.Trim(s, "_")
}

// toCamelCase converts a name to camelCase.
func toCamelCase(s string) string {
	parts := strings.Split(toSnakeCase(s), "_")
	if len(parts) == 0 {
		return s
	}

	result := parts[0]
	for _, part := range parts[1:] {
		if part != "" {
			result += strings.ToUpper(part[:1]) + part[1:]
		}
	}
	return result
}

// toPascalCase converts a name to PascalCase.
func toPascalCase(s string) string {
	camel := toCamelCase(s)
	if len(camel) == 0 {
		return camel
	}
	return strings.ToUpper(camel[:1]) + camel[1:]
}

// toShoutySnakeCase converts a name to SCREAMING_SNAKE_CASE.
func toShoutySnakeCase(s string) string {
	return strings.ToUpper(toSnakeCase(s))
}

// StripPackagePrefix removes package prefix from a type name.
// E.g., "pkg.TypeName" -> "TypeName"
func StripPackagePrefix(typeName string) string {
	if idx := strings.LastIndex(typeName, "."); idx >= 0 {
		return typeName[idx+1:]
	}
	return typeName
}

// TypeNeedsPointer checks if a type should be a pointer in Go.
// Primitive types and slices don't need pointers; structs usually do.
func TypeNeedsPointer(typeName string) bool {
	// Slice types don't need pointers
	if strings.HasPrefix(typeName, "[]") {
		return false
	}

	primitives := map[string]bool{
		"string":    true,
		"int":       true,
		"int64":     true,
		"float64":   true,
		"bool":      true,
		"[]byte":    true,
		"time.Time": true,
	}
	return !primitives[typeName]
}

// PathParamToGoVar converts a path parameter from `:param` syntax to Go variable.
// E.g., ":userId" -> "userId" (as extracted from path)
func PathParamToGoVar(param string) string {
	param = strings.TrimPrefix(param, ":")
	return toCamelCase(param)
}

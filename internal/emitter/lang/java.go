package lang

import (
	"fmt"
	"strings"
	"unicode"
)

// JavaAdapter implements LanguageAdapter for Java (Spring Boot, Java 17+) code generation.
// Generated code uses boxed types (Long, Double, Boolean) throughout so that every field
// can represent null without a primitive vs. reference-type distinction.
type JavaAdapter struct{}

// Metadata returns Java language metadata.
func (a *JavaAdapter) Metadata() LanguageMetadata {
	return LanguageMetadata{
		Name:              "java",
		Version:           "17+",
		Runtime:           "JVM",
		Framework:         "spring-boot",
		Features:          []string{"records", "generics", "spring-mvc"},
		ExportPath:        "src/main/java/com/example/generated",
		ImportPaths:       []string{"org.springframework", "java.util"},
		TypeMapperVersion: "1.0",
	}
}

// MapType converts Veld types to Java types.
// Boxed primitive types (Integer, Double, Boolean) are used so that optional fields can be null.
func (a *JavaAdapter) MapType(veldType string) (string, []string, error) {
	veldType = strings.TrimSpace(veldType)

	type entry struct {
		javaType string
		imports  []string
	}

	builtins := map[string]entry{
		"string":   {"String", nil},
		"int":      {"Integer", nil},
		"float":    {"Double", nil},
		"decimal":  {"java.math.BigDecimal", []string{"java.math.BigDecimal"}},
		"bool":     {"Boolean", nil},
		"date":     {"LocalDate", []string{"java.time.LocalDate"}},
		"datetime": {"LocalDateTime", []string{"java.time.LocalDateTime"}},
		"uuid":     {"UUID", []string{"java.util.UUID"}},
		"long":     {"Long", nil},
		"bytes":    {"byte[]", nil},
		"time":     {"LocalTime", []string{"java.time.LocalTime"}},
		"any":      {"Object", nil},
		"json":     {"Object", nil},
	}

	if e, ok := builtins[veldType]; ok {
		return e.javaType, e.imports, nil
	}

	// List<T> → java.util.List<T>
	if strings.HasPrefix(veldType, "List<") && strings.HasSuffix(veldType, ">") {
		inner := veldType[5 : len(veldType)-1]
		innerType, innerImports, err := a.MapType(inner)
		if err != nil {
			return "", nil, err
		}
		imports := append([]string{"java.util.List"}, innerImports...)
		return "List<" + innerType + ">", imports, nil
	}

	// Map<string, V> → java.util.Map<String, V>
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
		valJavaType, valImports, err := a.MapType(strings.TrimSpace(parts[1]))
		if err != nil {
			return "", nil, err
		}
		imports := append([]string{"java.util.Map"}, valImports...)
		return "Map<String, " + valJavaType + ">", imports, nil
	}

	// Custom types (model/enum names) pass through as-is.
	return veldType, nil, nil
}

// NamingConvention converts names to Java conventions.
func (a *JavaAdapter) NamingConvention(name string, context NamingContext) string {
	switch context {
	case NamingContextExported:
		return javaPascalCase(name)
	case NamingContextPrivate:
		return javaCamelCase(name)
	case NamingContextConstant:
		return strings.ToUpper(javaSnakeCase(name))
	case NamingContextPackage, NamingContextDatabase:
		return strings.ToLower(name)
	default:
		return javaCamelCase(name)
	}
}

// StructFieldTag for Java: fields use @JsonProperty annotations.
func (a *JavaAdapter) StructFieldTag(fieldName string, _ string) string {
	camel := javaCamelCase(fieldName)
	if camel != fieldName {
		return fmt.Sprintf(`@JsonProperty("%s")`, fieldName)
	}
	return ""
}

// ImportStatement generates a Java import statement.
func (a *JavaAdapter) ImportStatement(module string, alias string) string {
	if alias != "" {
		return fmt.Sprintf("import %s; // alias: %s", module, alias)
	}
	return fmt.Sprintf("import %s;", module)
}

// CommentSyntax returns Java comment style.
func (a *JavaAdapter) CommentSyntax() CommentStyle {
	return CommentStyle{Single: "//", Multi: "/*", MultiEnd: "*/"}
}

// FileExtension returns .java.
func (a *JavaAdapter) FileExtension() string { return ".java" }

// NullableType wraps a Java type in its nullable form.
// Primitive wrapper types are already nullable; reference types are unchanged.
func (a *JavaAdapter) NullableType(baseType string) string {
	// Primitives that need boxing:
	switch baseType {
	case "long":
		return "Long"
	case "double":
		return "Double"
	case "boolean":
		return "Boolean"
	case "int":
		return "Integer"
	}
	return baseType // reference types are already nullable in Java
}

// ── case helpers ──────────────────────────────────────────────────────────────

func javaSnakeCase(s string) string {
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

func javaCamelCase(s string) string {
	parts := strings.FieldsFunc(javaSnakeCase(s), func(r rune) bool { return r == '_' })
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

func javaPascalCase(s string) string {
	parts := strings.FieldsFunc(javaSnakeCase(s), func(r rune) bool { return r == '_' })
	var result strings.Builder
	for _, p := range parts {
		if len(p) > 0 {
			result.WriteString(strings.ToUpper(p[:1]) + p[1:])
		}
	}
	return result.String()
}

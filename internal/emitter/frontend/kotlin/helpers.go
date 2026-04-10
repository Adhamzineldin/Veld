package kotlin

// helpers.go — Veld-to-Kotlin type mapping, field type formatting, and utilities.

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// veldTypeToKotlin maps a Veld scalar or model name to its Kotlin equivalent.
func veldTypeToKotlin(t string) string {
	switch t {
	case "int":
		return "Int"
	case "float":
		return "Double"
	case "decimal":
		return "java.math.BigDecimal"
	case "bool":
		return "Boolean"
	case "long":
		return "Long"
	case "bytes":
		return "ByteArray"
	case "time":
		return "String"
	case "any", "json":
		return "Any"
	case "string", "date", "datetime", "uuid":
		return "String"
	default:
		return t
	}
}

// kotlinFieldType returns the full Kotlin type string for a field,
// handling maps, arrays, and scalars.
func kotlinFieldType(f ast.Field) string {
	if f.IsMap {
		return fmt.Sprintf("Map<String, %s>", veldTypeToKotlin(f.MapValueType))
	}
	base := veldTypeToKotlin(f.Type)
	if f.IsArray {
		return fmt.Sprintf("List<%s>", base)
	}
	return base
}

// collectAllFields returns all fields for a model, including inherited fields
// from the extends chain.
func collectAllFields(m ast.Model, models map[string]ast.Model) []ast.Field {
	if m.Extends == "" {
		return m.Fields
	}
	parent, ok := models[m.Extends]
	if !ok {
		return m.Fields
	}
	parentFields := collectAllFields(parent, models)
	return append(parentFields, m.Fields...)
}

// lcFirst lowercases the first character of a string.
func lcFirst(s string) string {
	if s == "" {
		return s
	}
	return strings.ToLower(s[:1]) + s[1:]
}

package dart

// helpers.go — Veld-to-Dart type mapping, field type formatting, and utilities.

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// veldTypeToDart maps a Veld scalar or model name to its Dart equivalent.
func veldTypeToDart(t string) string {
	switch t {
	case "int":
		return "int"
	case "float":
		return "double"
	case "bool":
		return "bool"
	case "long":
		return "int"
	case "bytes":
		return "Uint8List"
	case "time":
		return "String"
	case "any", "json":
		return "dynamic"
	case "string", "date", "datetime", "uuid", "decimal":
		return "String"
	default:
		return t
	}
}

// dartFieldType returns the full Dart type string for a field,
// handling maps, arrays, and scalars.
func dartFieldType(f ast.Field) string {
	if f.IsMap {
		return fmt.Sprintf("Map<String, %s>", veldTypeToDart(f.MapValueType))
	}
	base := veldTypeToDart(f.Type)
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

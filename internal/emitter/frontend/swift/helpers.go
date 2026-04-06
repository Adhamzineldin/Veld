package swift

// helpers.go — Veld-to-Swift type mapping, field type formatting, and utilities.

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// veldTypeToSwift maps a Veld scalar or model name to its Swift equivalent.
func veldTypeToSwift(t string) string {
	switch t {
	case "int":
		return "Int"
	case "float":
		return "Double"
	case "decimal":
		return "Decimal"
	case "bool":
		return "Bool"
	case "string", "date", "datetime", "uuid":
		return "String"
	default:
		return t
	}
}

// swiftFieldType returns the full Swift type string for a field,
// handling maps, arrays, and scalars.
func swiftFieldType(f ast.Field) string {
	if f.IsMap {
		return fmt.Sprintf("[String: %s]", veldTypeToSwift(f.MapValueType))
	}
	base := veldTypeToSwift(f.Type)
	if f.IsArray {
		return fmt.Sprintf("[%s]", base)
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

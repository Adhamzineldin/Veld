package tshelpers

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// VeldScalarToTS maps a Veld scalar or model name to its TypeScript equivalent.
func VeldScalarToTS(t string) string {
	switch t {
	case "int", "float":
		return "number"
	case "bool":
		return "boolean"
	case "date":
		return "ISODate"
	case "datetime":
		return "ISODateTime"
	case "uuid":
		return "UUID"
	case "decimal":
		return "Decimal"
	case "any", "json":
		return "any"
	default:
		return t // model/enum reference stays as-is
	}
}

// NeedsVeldScalarAliases checks whether any model in the AST uses uuid,
// decimal, date, or datetime types, returning which branded type aliases should be emitted.
func NeedsVeldScalarAliases(models []ast.Model) (needsUUID, needsDecimal, needsDate, needsDateTime bool) {
	for _, m := range models {
		for _, f := range m.Fields {
			if f.Type == "uuid" || f.MapValueType == "uuid" {
				needsUUID = true
			}
			if f.Type == "decimal" || f.MapValueType == "decimal" {
				needsDecimal = true
			}
			if f.Type == "date" || f.MapValueType == "date" {
				needsDate = true
			}
			if f.Type == "datetime" || f.MapValueType == "datetime" {
				needsDateTime = true
			}
		}
	}
	return
}

// ScalarAliasBlock returns the TypeScript type-alias declarations for branded
// scalar types (UUID, Decimal, ISODate, ISODateTime) that are used in the AST.
func ScalarAliasBlock(models []ast.Model) string {
	needsUUID, needsDecimal, needsDate, needsDateTime := NeedsVeldScalarAliases(models)
	if !needsUUID && !needsDecimal && !needsDate && !needsDateTime {
		return ""
	}
	var sb strings.Builder
	sb.WriteString("// ── Branded scalar types ─────────────────────────────────────────────\n")
	if needsUUID {
		sb.WriteString("/** A UUID string (e.g. \"550e8400-e29b-41d4-a716-446655440000\"). */\n")
		sb.WriteString("export type UUID = string & { readonly __brand: 'UUID' };\n")
	}
	if needsDecimal {
		sb.WriteString("/** A decimal value represented as a string for precision (e.g. \"12345.6789\"). */\n")
		sb.WriteString("export type Decimal = string & { readonly __brand: 'Decimal' };\n")
	}
	if needsDate {
		sb.WriteString("/** An ISO 8601 date string (e.g. \"2024-01-15\"). */\n")
		sb.WriteString("export type ISODate = string & { readonly __brand: 'ISODate' };\n")
	}
	if needsDateTime {
		sb.WriteString("/** An ISO 8601 datetime string (e.g. \"2024-01-15T09:30:00Z\"). */\n")
		sb.WriteString("export type ISODateTime = string & { readonly __brand: 'ISODateTime' };\n")
	}
	sb.WriteString("\n")
	return sb.String()
}

// VeldTypeToTS maps a Veld type name to its TypeScript equivalent,
// appending [] when isArray is true.
func VeldTypeToTS(t string, isArray bool) string {
	base := VeldScalarToTS(t)
	if isArray {
		return base + "[]"
	}
	return base
}

// VeldFieldToTS maps a full Field to its TypeScript type string,
// handling arrays, maps, union types, and scalar types.
func VeldFieldToTS(f ast.Field) string {
	if len(f.UnionTypes) > 0 {
		parts := make([]string, len(f.UnionTypes))
		for i, t := range f.UnionTypes {
			parts[i] = VeldScalarToTS(t)
		}
		return strings.Join(parts, " | ")
	}
	if f.IsMap {
		return fmt.Sprintf("Record<string, %s>", VeldScalarToTS(f.MapValueType))
	}
	return VeldTypeToTS(f.Type, f.IsArray)
}

// FormatOutputType returns the TS type for an action output, handling arrays.
// An empty output returns "void".
func FormatOutputType(act ast.Action) string {
	if act.Output == "" {
		return "void"
	}
	base := VeldScalarToTS(act.Output)
	if act.OutputArray {
		return base + "[]"
	}
	return base
}

// TSDefaultLiteral converts a Veld default value to a TypeScript literal.
// Examples: "0" → "0", "\"hello\"" → "\"hello\"", "true" → "true", "user" → "\"user\""
func TSDefaultLiteral(val, veldType string) string {
	// Already a quoted string
	if strings.HasPrefix(val, "\"") {
		return val
	}
	// Booleans
	if val == "true" || val == "false" {
		return val
	}
	// Numeric — check if it parses as a number
	if _, err := strconv.ParseFloat(val, 64); err == nil {
		return val
	}
	// Enum value or identifier — emit as a string literal
	return "\"" + val + "\""
}

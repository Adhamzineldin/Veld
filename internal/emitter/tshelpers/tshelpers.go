package tshelpers

import (
	"fmt"
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
	case "date", "datetime":
		return "string"
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

// NeedsVeldScalarAliases checks whether any model in the AST uses uuid or
// decimal types, returning which branded type aliases should be emitted.
func NeedsVeldScalarAliases(models []ast.Model) (needsUUID, needsDecimal bool) {
	for _, m := range models {
		for _, f := range m.Fields {
			if f.Type == "uuid" || f.MapValueType == "uuid" {
				needsUUID = true
			}
			if f.Type == "decimal" || f.MapValueType == "decimal" {
				needsDecimal = true
			}
		}
	}
	return
}

// ScalarAliasBlock returns the TypeScript type-alias declarations for branded
// scalar types (UUID, Decimal) that are used in the AST.
func ScalarAliasBlock(models []ast.Model) string {
	needsUUID, needsDecimal := NeedsVeldScalarAliases(models)
	if !needsUUID && !needsDecimal {
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

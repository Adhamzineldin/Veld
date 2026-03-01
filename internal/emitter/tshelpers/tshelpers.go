package tshelpers

import (
	"fmt"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// VeldScalarToTS maps a Veld scalar or model name to its TypeScript equivalent.
func VeldScalarToTS(t string) string {
	switch t {
	case "int", "float":
		return "number"
	case "bool":
		return "boolean"
	case "date", "datetime", "uuid":
		return "string"
	default:
		return t // model/enum reference stays as-is
	}
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
// handling arrays, maps, and scalar types.
func VeldFieldToTS(f ast.Field) string {
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

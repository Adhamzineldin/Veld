package tshelpers

import "github.com/veld-dev/veld/internal/ast"

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

// FormatOutputType returns the TS type for an action output, handling arrays.
func FormatOutputType(act ast.Action) string {
	base := VeldScalarToTS(act.Output)
	if act.OutputArray {
		return base + "[]"
	}
	return base
}

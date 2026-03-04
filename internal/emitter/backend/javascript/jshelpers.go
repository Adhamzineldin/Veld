package javascript

import (
	"fmt"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// veldScalarToJSDoc maps a Veld scalar type to its JSDoc equivalent.
func veldScalarToJSDoc(t string) string {
	switch t {
	case "int", "float":
		return "number"
	case "bool":
		return "boolean"
	case "date", "datetime", "uuid":
		return "string"
	case "any", "json":
		return "*"
	default:
		return t // model/enum reference stays as-is
	}
}

// veldFieldToJSDoc maps a full Field to its JSDoc type string.
func veldFieldToJSDoc(f ast.Field) string {
	if f.IsMap {
		return fmt.Sprintf("Object.<string, %s>", veldScalarToJSDoc(f.MapValueType))
	}
	base := veldScalarToJSDoc(f.Type)
	if f.IsArray {
		return base + "[]"
	}
	return base
}

// formatOutputTypeJSDoc returns the JSDoc type for an action output.
func formatOutputTypeJSDoc(act ast.Action) string {
	if act.Output == "" {
		return "void"
	}
	base := veldScalarToJSDoc(act.Output)
	if act.OutputArray {
		return base + "[]"
	}
	return base
}

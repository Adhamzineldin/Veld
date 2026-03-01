package swift

// types.go — emits Swift enums and Codable structs.

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// emitEnums writes Swift enum declarations with String raw values and Codable conformance.
func emitEnums(sb *strings.Builder, a ast.AST, allTypes map[string]bool) {
	for _, en := range a.Enums {
		if !allTypes[en.Name] {
			continue
		}
		sb.WriteString(fmt.Sprintf("enum %s: String, Codable {\n", en.Name))
		for _, v := range en.Values {
			sb.WriteString(fmt.Sprintf("    case %s = \"%s\"\n", v, v))
		}
		sb.WriteString("}\n\n")
	}
}

// emitStructs writes Codable struct declarations.
func emitStructs(sb *strings.Builder, a ast.AST, allTypes map[string]bool) {
	modelMap := make(map[string]ast.Model, len(a.Models))
	for _, m := range a.Models {
		modelMap[m.Name] = m
	}

	for _, m := range a.Models {
		if !allTypes[m.Name] {
			continue
		}
		allFields := collectAllFields(m, modelMap)
		sb.WriteString(fmt.Sprintf("struct %s: Codable {\n", m.Name))
		for _, f := range allFields {
			ft := swiftFieldType(f)
			if f.Optional {
				sb.WriteString(fmt.Sprintf("    var %s: %s?\n", f.Name, ft))
			} else {
				sb.WriteString(fmt.Sprintf("    var %s: %s\n", f.Name, ft))
			}
		}
		sb.WriteString("}\n\n")
	}
}

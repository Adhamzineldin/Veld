package kotlin

// types.go — emits Kotlin enums and @Serializable data classes.

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// emitEnums writes @Serializable enum class declarations.
func emitEnums(sb *strings.Builder, a ast.AST, allTypes map[string]bool) {
	for _, en := range a.Enums {
		if !allTypes[en.Name] {
			continue
		}
		sb.WriteString("@Serializable\n")
		sb.WriteString(fmt.Sprintf("enum class %s {\n", en.Name))
		for i, v := range en.Values {
			if i > 0 {
				sb.WriteString(",\n")
			}
			sb.WriteString(fmt.Sprintf("    @SerialName(\"%s\") %s", v, strings.ToUpper(v)))
		}
		sb.WriteString("\n}\n\n")
	}
}

// emitDataClasses writes @Serializable data class declarations.
func emitDataClasses(sb *strings.Builder, a ast.AST, allTypes map[string]bool) {
	modelMap := make(map[string]ast.Model, len(a.Models))
	for _, m := range a.Models {
		modelMap[m.Name] = m
	}

	for _, m := range a.Models {
		if !allTypes[m.Name] {
			continue
		}
		allFields := collectAllFields(m, modelMap)
		sb.WriteString("@Serializable\n")
		sb.WriteString(fmt.Sprintf("data class %s(\n", m.Name))
		for i, f := range allFields {
			ft := kotlinFieldType(f)
			if f.Optional {
				sb.WriteString(fmt.Sprintf("    val %s: %s? = null", f.Name, ft))
			} else {
				sb.WriteString(fmt.Sprintf("    val %s: %s", f.Name, ft))
			}
			if i < len(allFields)-1 {
				sb.WriteString(",")
			}
			sb.WriteString("\n")
		}
		sb.WriteString(")\n\n")
	}
}

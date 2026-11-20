package dart

// types.go — emits Dart enums and model classes with fromJson/toJson.

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// emitEnums writes Dart enum declarations for all used enums.
func emitEnums(sb *strings.Builder, a ast.AST, allTypes map[string]bool) {
	for _, en := range a.Enums {
		if !allTypes[en.Name] {
			continue
		}
		sb.WriteString(fmt.Sprintf("enum %s {\n", en.Name))
		for _, v := range en.Values {
			sb.WriteString(fmt.Sprintf("  %s,\n", v))
		}
		sb.WriteString("}\n\n")
	}
}

// emitModels writes Dart classes with constructor, fromJson factory, and toJson method.
func emitModels(sb *strings.Builder, a ast.AST, allTypes map[string]bool) {
	modelMap := make(map[string]ast.Model, len(a.Models))
	for _, m := range a.Models {
		modelMap[m.Name] = m
	}

	for _, m := range a.Models {
		if !allTypes[m.Name] {
			continue
		}
		allFields := collectAllFields(m, modelMap)

		// Class fields
		sb.WriteString(fmt.Sprintf("class %s {\n", m.Name))
		for _, f := range allFields {
			ft := dartFieldType(f)
			if f.Optional {
				sb.WriteString(fmt.Sprintf("  %s? %s;\n", ft, f.Name))
			} else {
				sb.WriteString(fmt.Sprintf("  %s %s;\n", ft, f.Name))
			}
		}
		sb.WriteString("\n")

		// Constructor
		sb.WriteString(fmt.Sprintf("  %s({", m.Name))
		for i, f := range allFields {
			if i > 0 {
				sb.WriteString(", ")
			}
			if f.Optional {
				sb.WriteString(fmt.Sprintf("this.%s", f.Name))
			} else {
				sb.WriteString(fmt.Sprintf("required this.%s", f.Name))
			}
		}
		sb.WriteString("});\n\n")

		// fromJson factory
		sb.WriteString(fmt.Sprintf("  factory %s.fromJson(Map<String, dynamic> json) {\n", m.Name))
		sb.WriteString(fmt.Sprintf("    return %s(\n", m.Name))
		for _, f := range allFields {
			sb.WriteString(fmt.Sprintf("      %s: json['%s'],\n", f.Name, f.Name))
		}
		sb.WriteString("    );\n  }\n\n")

		// toJson method
		sb.WriteString("  Map<String, dynamic> toJson() => {\n")
		for _, f := range allFields {
			if f.Optional {
				sb.WriteString(fmt.Sprintf("    if (%s != null) '%s': %s,\n", f.Name, f.Name, f.Name))
			} else {
				sb.WriteString(fmt.Sprintf("    '%s': %s,\n", f.Name, f.Name))
			}
		}
		sb.WriteString("  };\n")
		sb.WriteString("}\n\n")
	}
}

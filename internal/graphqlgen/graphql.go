// Package graphqlgen builds a GraphQL SDL schema from a Veld AST.
package graphqlgen

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

// BuildSchema generates a GraphQL SDL string from the AST.
func BuildSchema(a ast.AST) string {
	var sb strings.Builder

	for _, en := range a.Enums {
		if en.Description != "" {
			sb.WriteString(fmt.Sprintf("\"\"\"%s\"\"\"\n", en.Description))
		}
		sb.WriteString(fmt.Sprintf("enum %s {\n", en.Name))
		for _, v := range en.Values {
			sb.WriteString(fmt.Sprintf("  %s\n", v))
		}
		sb.WriteString("}\n\n")
	}

	modelMap := make(map[string]ast.Model)
	for _, m := range a.Models {
		modelMap[m.Name] = m
	}
	inputModels := make(map[string]bool)
	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			if act.Input != "" {
				inputModels[act.Input] = true
			}
			if act.Query != "" {
				inputModels[act.Query] = true
			}
		}
	}

	for _, m := range a.Models {
		allFields := flattenFields(m, modelMap)
		keyword := "type"
		if inputModels[m.Name] {
			keyword = "input"
		}
		if m.Description != "" {
			sb.WriteString(fmt.Sprintf("\"\"\"%s\"\"\"\n", m.Description))
		}
		sb.WriteString(fmt.Sprintf("%s %s {\n", keyword, m.Name))
		for _, f := range allFields {
			sb.WriteString(fmt.Sprintf("  %s: %s\n", f.Name, fieldType(f)))
		}
		sb.WriteString("}\n\n")
	}

	var queries, mutations []string
	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			method := strings.ToUpper(act.Method)
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			var args []string
			for _, p := range emitter.ExtractPathParams(routePath) {
				args = append(args, fmt.Sprintf("%s: String!", p))
			}
			if act.Input != "" {
				args = append(args, fmt.Sprintf("input: %s!", act.Input))
			}
			if act.Query != "" {
				args = append(args, fmt.Sprintf("query: %s", act.Query))
			}
			argStr := ""
			if len(args) > 0 {
				argStr = "(" + strings.Join(args, ", ") + ")"
			}
			line := fmt.Sprintf("  %s%s: %s", lcfirst(mod.Name)+act.Name, argStr, returnType(act))
			if method == "GET" {
				queries = append(queries, line)
			} else {
				mutations = append(mutations, line)
			}
		}
	}

	if len(queries) > 0 {
		sb.WriteString("type Query {\n")
		for _, q := range queries {
			sb.WriteString(q + "\n")
		}
		sb.WriteString("}\n\n")
	}
	if len(mutations) > 0 {
		sb.WriteString("type Mutation {\n")
		for _, m := range mutations {
			sb.WriteString(m + "\n")
		}
		sb.WriteString("}\n\n")
	}

	return sb.String()
}

func gqlType(t string) string {
	switch t {
	case "int":
		return "Int"
	case "float":
		return "Float"
	case "bool":
		return "Boolean"
	case "long":
		return "Int"
	case "bytes", "time", "any", "json":
		return "String"
	case "string", "date", "datetime", "uuid", "decimal":
		return "String"
	default:
		return t
	}
}

func fieldType(f ast.Field) string {
	if f.IsMap {
		return "String"
	}
	base := gqlType(f.Type)
	if f.IsArray {
		if f.Optional {
			return fmt.Sprintf("[%s]", base)
		}
		return fmt.Sprintf("[%s!]!", base)
	}
	if f.Optional {
		return base
	}
	return base + "!"
}

func returnType(act ast.Action) string {
	if act.Output == "" {
		return "Boolean"
	}
	base := gqlType(act.Output)
	if act.OutputArray {
		return fmt.Sprintf("[%s!]!", base)
	}
	return base + "!"
}

func flattenFields(m ast.Model, models map[string]ast.Model) []ast.Field {
	if m.Extends == "" {
		return m.Fields
	}
	parent, ok := models[m.Extends]
	if !ok {
		return m.Fields
	}
	return append(flattenFields(parent, models), m.Fields...)
}

func lcfirst(s string) string {
	if s == "" {
		return s
	}
	return strings.ToLower(s[:1]) + s[1:]
}

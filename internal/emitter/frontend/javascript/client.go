package javascript

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
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
		return t
	}
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

// writeAction writes a single SDK method inside a module API object.
func writeAction(sb *strings.Builder, mod ast.Module, act ast.Action) {
	outputType := formatOutputTypeJSDoc(act)
	routePath := act.Path
	if mod.Prefix != "" {
		routePath = mod.Prefix + act.Path
	}

	// Route doc comment.
	docMethod := strings.ToUpper(act.Method)
	if act.Deprecated != "" {
		sb.WriteString("    /**\n")
		if act.Description != "" {
			sb.WriteString(fmt.Sprintf("     * %s %s — %s\n", docMethod, routePath, act.Description))
		} else {
			sb.WriteString(fmt.Sprintf("     * %s %s\n", docMethod, routePath))
		}
		sb.WriteString(fmt.Sprintf("     * @deprecated %s\n", act.Deprecated))
	} else {
		sb.WriteString("    /**\n")
		if act.Description != "" {
			sb.WriteString(fmt.Sprintf("     * %s %s — %s\n", docMethod, routePath, act.Description))
		} else {
			sb.WriteString(fmt.Sprintf("     * %s %s\n", docMethod, routePath))
		}
	}

	pathParams := emitter.ExtractPathParams(routePath)
	method := strings.ToUpper(act.Method)
	fnName := strings.ToLower(method)
	if method == "DELETE" {
		fnName = "del"
	}

	// JSDoc @param annotations.
	for _, p := range pathParams {
		sb.WriteString(fmt.Sprintf("     * @param {string} %s\n", p))
	}
	if act.Input != "" {
		sb.WriteString(fmt.Sprintf("     * @param {%s} input\n", act.Input))
	}
	if act.Query != "" {
		sb.WriteString(fmt.Sprintf("     * @param {%s} [query]\n", act.Query))
	}
	sb.WriteString(fmt.Sprintf("     * @returns {Promise<%s>}\n", outputType))
	sb.WriteString("     */\n")

	// Build function signature params.
	var sigParams []string
	for _, p := range pathParams {
		sigParams = append(sigParams, p)
	}
	if act.Input != "" {
		sigParams = append(sigParams, "input")
	}
	if act.Query != "" {
		sigParams = append(sigParams, "query")
	}
	sig := strings.Join(sigParams, ", ")

	// Build URL expression.
	var urlExpr string
	if len(pathParams) > 0 {
		urlExpr = "`" + emitter.ToTemplateLiteral(routePath) + "`"
	} else {
		urlExpr = "'" + routePath + "'"
	}

	// Build query string suffix.
	queryAppend := ""
	if act.Query != "" {
		queryAppend = " + (query ? '?' + new URLSearchParams(query).toString() : '')"
	}

	camelName := emitter.ToCamelCase(act.Name)
	if method == "GET" {
		sb.WriteString(fmt.Sprintf("    %s(%s) {\n", camelName, sig))
		sb.WriteString(fmt.Sprintf("      return get(%s%s);\n", urlExpr, queryAppend))
		sb.WriteString("    },\n")
	} else {
		bodyArg := "input"
		if act.Input == "" {
			bodyArg = "{}"
		}
		sb.WriteString(fmt.Sprintf("    %s(%s) {\n", camelName, sig))
		sb.WriteString(fmt.Sprintf("      return %s(%s%s, %s);\n", fnName, urlExpr, queryAppend, bodyArg))
		sb.WriteString("    },\n")
	}
}

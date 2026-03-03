package typescript

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/tshelpers"
)

// writeAction writes a single SDK method inside a module API object.
func writeAction(sb *strings.Builder, mod ast.Module, act ast.Action) {
	outputType := tshelpers.FormatOutputType(act)
	routePath := act.Path
	if mod.Prefix != "" {
		routePath = mod.Prefix + act.Path
	}

	// Route doc comment: METHOD /full/path — Description (+ @deprecated if set)
	docMethod := strings.ToUpper(act.Method)
	if act.Deprecated != "" {
		// Multi-line JSDoc so IDEs render the strikethrough properly.
		sb.WriteString("    /**\n")
		if act.Description != "" {
			sb.WriteString(fmt.Sprintf("     * %s %s — %s\n", docMethod, routePath, act.Description))
		} else {
			sb.WriteString(fmt.Sprintf("     * %s %s\n", docMethod, routePath))
		}
		sb.WriteString(fmt.Sprintf("     * @deprecated %s\n", act.Deprecated))
		sb.WriteString("     */\n")
	} else if act.Description != "" {
		sb.WriteString(fmt.Sprintf("    /** %s %s — %s */\n", docMethod, routePath, act.Description))
	} else {
		sb.WriteString(fmt.Sprintf("    /** %s %s */\n", docMethod, routePath))
	}

	pathParams := emitter.ExtractPathParams(routePath)
	hasPathParams := len(pathParams) > 0
	method := strings.ToUpper(act.Method)
	fnName := strings.ToLower(method)
	if method == "DELETE" {
		fnName = "del"
	}

	// Build URL expression
	var urlExpr string
	if hasPathParams {
		urlExpr = "`" + emitter.ToTemplateLiteral(routePath) + "`"
	} else {
		urlExpr = "'" + routePath + "'"
	}

	// Build function signature params
	var sigParams []string
	for _, p := range pathParams {
		sigParams = append(sigParams, p+": string")
	}
	if act.Input != "" {
		sigParams = append(sigParams, "input: "+act.Input)
	}
	if act.Query != "" {
		sigParams = append(sigParams, "query?: "+act.Query)
	}

	sig := strings.Join(sigParams, ", ")

	// Build query string suffix
	queryAppend := ""
	if act.Query != "" {
		queryAppend = " + (query ? '?' + new URLSearchParams(query as Record<string, string>).toString() : '')"
	}

	// Build the call expression
	camelName := emitter.ToCamelCase(act.Name)
	if method == "GET" {
		sb.WriteString(fmt.Sprintf("    %s: (%s): Promise<%s> =>\n", camelName, sig, outputType))
		sb.WriteString(fmt.Sprintf("      get(%s%s),\n", urlExpr, queryAppend))
	} else {
		bodyArg := "input"
		if act.Input == "" {
			bodyArg = "{}"
		}
		sb.WriteString(fmt.Sprintf("    %s: (%s): Promise<%s> =>\n", camelName, sig, outputType))
		sb.WriteString(fmt.Sprintf("      %s(%s%s, %s),\n", fnName, urlExpr, queryAppend, bodyArg))
	}
}

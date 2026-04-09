package typescript

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/tshelpers"
)

// writeModuleClass writes a full TypeScript class for a module, e.g. AuthClient.
// defaultBase is the per-module base URL fallback (may be empty).
func writeModuleClass(sb *strings.Builder, a ast.AST, mod ast.Module, defaultBase string, serverSdk bool) {
	className := mod.Name + "Client"

	usedTypes := emitter.CollectUsedTypes(a, mod)

	hasQuery := false
	for _, act := range mod.Actions {
		if act.Query != "" {
			hasQuery = true
			break
		}
	}

	// Imports line (used in per-module files; for the bundled api.ts we skip this).
	// The caller decides whether to write imports; this function only writes the class body.

	sb.WriteString(fmt.Sprintf("class %s {\n", className))
	sb.WriteString("  private readonly base: string;\n")
	sb.WriteString("  private readonly hdrs: Record<string, string>;\n")
	sb.WriteString("\n")

	if serverSdk {
		// Server SDK: baseUrl is required, no env fallback.
		sb.WriteString("  constructor(baseUrl: string, headers?: Record<string, string>) {\n")
		sb.WriteString("    this.base = baseUrl;\n")
		sb.WriteString("    this.hdrs = headers ?? {};\n")
		sb.WriteString("  }\n")
	} else {
		sb.WriteString("  constructor(config?: VeldClientConfig | string) {\n")
		if defaultBase != "" {
			sb.WriteString(fmt.Sprintf("    this.base = resolveBase(config) || %q;\n", defaultBase))
		} else {
			sb.WriteString("    this.base = resolveBase(config);\n")
		}
		sb.WriteString("    this.hdrs = typeof config === 'object' && config !== null\n")
		sb.WriteString("      ? (config as VeldClientConfig).headers ?? {}\n")
		sb.WriteString("      : {};\n")
		sb.WriteString("  }\n")
	}

	sb.WriteString("\n")
	sb.WriteString("  private async request<T>(method: string, path: string, body?: unknown, extraHeaders?: Record<string, string>): Promise<T> {\n")
	sb.WriteString("    const res = await fetch(this.base + path, {\n")
	sb.WriteString("      method,\n")
	sb.WriteString("      headers: { 'Content-Type': 'application/json', ...this.hdrs, ...extraHeaders },\n")
	sb.WriteString("      body: body !== undefined ? JSON.stringify(body) : undefined,\n")
	sb.WriteString("    });\n")
	sb.WriteString("    if (!res.ok) throw new VeldApiError(res.status, await res.text());\n")
	sb.WriteString("    if (res.status === 204) return undefined as T;\n")
	sb.WriteString("    return res.json() as Promise<T>;\n")
	sb.WriteString("  }\n")

	_ = usedTypes // types are imported at file level by the caller
	_ = hasQuery  // used by writeActionMethod

	for _, act := range mod.Actions {
		sb.WriteString("\n")
		writeActionMethod(sb, mod, act)
	}

	// Typed error code constants.
	if emitter.HasErrors(mod) {
		sb.WriteString("\n  readonly errors = {\n")
		for _, act := range mod.Actions {
			if len(act.Errors) == 0 {
				continue
			}
			camelAction := emitter.ToCamelCase(act.Name)
			sb.WriteString(fmt.Sprintf("    %s: {\n", camelAction))
			for _, errName := range act.Errors {
				code := emitter.ErrorCode(act.Name, errName)
				camelErr := emitter.ToCamelCase(errName)
				sb.WriteString(fmt.Sprintf("      %s: '%s',\n", camelErr, code))
			}
			sb.WriteString("    },\n")
		}
		sb.WriteString("  } as const;\n")
	}

	sb.WriteString("}\n")
}

// writeActionMethod writes a single method inside a module client class.
func writeActionMethod(sb *strings.Builder, mod ast.Module, act ast.Action) {
	outputType := tshelpers.FormatOutputType(act)
	routePath := act.Path
	if mod.Prefix != "" {
		routePath = mod.Prefix + act.Path
	}

	docMethod := strings.ToUpper(act.Method)
	if act.Deprecated != "" {
		sb.WriteString("  /**\n")
		if act.Description != "" {
			sb.WriteString(fmt.Sprintf("   * %s %s — %s\n", docMethod, routePath, act.Description))
		} else {
			sb.WriteString(fmt.Sprintf("   * %s %s\n", docMethod, routePath))
		}
		sb.WriteString(fmt.Sprintf("   * @deprecated %s\n", act.Deprecated))
		sb.WriteString("   */\n")
	} else if act.Description != "" {
		sb.WriteString(fmt.Sprintf("  /** %s %s — %s */\n", docMethod, routePath, act.Description))
	} else {
		sb.WriteString(fmt.Sprintf("  /** %s %s */\n", docMethod, routePath))
	}

	// WebSocket action — generate a connect method instead of a fetch call.
	if strings.ToUpper(act.Method) == "WS" {
		writeWsActionMethod(sb, mod, act, routePath)
		return
	}

	pathParams := emitter.ExtractPathParams(routePath)
	method := strings.ToUpper(act.Method)

	var urlExpr string
	if len(pathParams) > 0 {
		urlExpr = "`" + emitter.ToTemplateLiteral(routePath) + "`"
	} else {
		urlExpr = "'" + routePath + "'"
	}

	// Build signature params.
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
	if act.Headers != "" {
		sigParams = append(sigParams, "headers: "+act.Headers)
	}
	sig := strings.Join(sigParams, ", ")

	queryAppend := ""
	if act.Query != "" {
		queryAppend = " + buildQueryString(query as Record<string, unknown>)"
	}

	headersArg := ""
	if act.Headers != "" {
		headersArg = ", headers"
	}

	camelName := emitter.ToCamelCase(act.Name)

	if method == "GET" {
		sb.WriteString(fmt.Sprintf("  %s(%s): Promise<%s> {\n", camelName, sig, outputType))
		sb.WriteString(fmt.Sprintf("    return this.request('GET', %s%s, undefined%s);\n", urlExpr, queryAppend, headersArg))
		sb.WriteString("  }\n")
	} else {
		bodyArg := "input"
		if act.Input == "" {
			bodyArg = "{}"
		}
		httpMethod := method
		if method == "DELETE" {
			httpMethod = "DELETE"
		}
		sb.WriteString(fmt.Sprintf("  %s(%s): Promise<%s> {\n", camelName, sig, outputType))
		sb.WriteString(fmt.Sprintf("    return this.request('%s', %s%s, %s%s);\n", httpMethod, urlExpr, queryAppend, bodyArg, headersArg))
		sb.WriteString("  }\n")
	}
}

// writeWsActionMethod writes a WebSocket connect method inside a module client class.
func writeWsActionMethod(sb *strings.Builder, mod ast.Module, act ast.Action, routePath string) {
	receiveType := act.Stream
	if receiveType == "" {
		receiveType = "unknown"
	}
	sendType := act.Emit
	if sendType == "" {
		sendType = "unknown"
	}

	pathParams := emitter.ExtractPathParams(routePath)
	var sigParams []string
	for _, p := range pathParams {
		sigParams = append(sigParams, p+": string")
	}
	sig := strings.Join(sigParams, ", ")

	var urlExpr string
	if len(pathParams) > 0 {
		urlExpr = "`${wsBase}" + emitter.ToTemplateLiteral(routePath) + "`"
	} else {
		urlExpr = "`${wsBase}" + routePath + "`"
	}

	camelName := emitter.ToCamelCase(act.Name)
	connectName := "subscribeTo" + mod.Name
	if act.Name != "" {
		connectName = "subscribeTo" + act.Name
	}
	_ = camelName

	sb.WriteString(fmt.Sprintf("  %s(%s): VeldWebSocket<%s, %s> {\n", connectName, sig, receiveType, sendType))
	sb.WriteString("    const wsBase = this.base.replace(/^http/, 'ws');\n")
	sb.WriteString(fmt.Sprintf("    return new VeldWebSocket<%s, %s>(%s).connect();\n", receiveType, sendType, urlExpr))
	sb.WriteString("  }\n")
}

// writeVeldApiClient writes the root VeldApiClient class that composes all module clients,
// plus the default `api` export.
func writeVeldApiClient(sb *strings.Builder, modules []ast.Module, serverSdk bool) {
	sb.WriteString("\nexport class VeldApiClient {\n")
	for _, mod := range modules {
		sb.WriteString(fmt.Sprintf("  public readonly %s: %sClient;\n", strings.ToLower(mod.Name), mod.Name))
	}
	sb.WriteString("\n")

	if serverSdk {
		sb.WriteString("  constructor(baseUrl: string, headers?: Record<string, string>) {\n")
		for _, mod := range modules {
			sb.WriteString(fmt.Sprintf("    this.%s = new %sClient(baseUrl, headers);\n",
				strings.ToLower(mod.Name), mod.Name))
		}
	} else {
		sb.WriteString("  constructor(config?: VeldClientConfig | string) {\n")
		for _, mod := range modules {
			sb.WriteString(fmt.Sprintf("    this.%s = new %sClient(config);\n",
				strings.ToLower(mod.Name), mod.Name))
		}
	}

	sb.WriteString("  }\n")
	sb.WriteString("}\n")

	if !serverSdk {
		sb.WriteString("\nexport const api = new VeldApiClient();\n")
	}
}

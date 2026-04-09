package kotlin

// client.go — emits the VeldApi object with HTTP helper and per-module sub-objects.

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

// emitApiObject writes the VeldApi singleton: HTTP request helper and
// per-module nested objects with SDK methods.
func emitApiObject(sb *strings.Builder, a ast.AST, opts emitter.EmitOptions) {
	baseUrlDefault := ""
	if opts.BaseUrl != "" {
		baseUrlDefault = opts.BaseUrl
	}
	sb.WriteString(fmt.Sprintf("object VeldApi {\n    private val client = HttpClient.newHttpClient()\n    var baseUrl: String = \"%s\"\n\n", baseUrlDefault))

	// HTTP helper
	sb.WriteString(`    private fun request(method: String, path: String, body: String? = null): String {
        val builder = HttpRequest.newBuilder()
            .uri(URI.create("$baseUrl$path"))
            .header("Content-Type", "application/json")
        when (method) {
            "GET" -> builder.GET()
            "DELETE" -> if (body != null) builder.method("DELETE", HttpRequest.BodyPublishers.ofString(body)) else builder.DELETE()
            else -> builder.method(method, HttpRequest.BodyPublishers.ofString(body ?: "{}"))
        }
        val response = client.send(builder.build(), HttpResponse.BodyHandlers.ofString())
        if (response.statusCode() >= 400) {
            throw VeldApiError(response.statusCode(), response.body())
        }
        return response.body()
    }

`)

	// Per-module objects
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("    object %s {\n", mod.Name))
		for _, act := range mod.Actions {
			writeAction(sb, mod, act)
		}
		sb.WriteString("    }\n\n")
	}

	sb.WriteString("}\n")
}

// writeAction writes a single SDK method (HTTP or WebSocket).
func writeAction(sb *strings.Builder, mod ast.Module, act ast.Action) {
	routePath := act.Path
	if mod.Prefix != "" {
		routePath = mod.Prefix + act.Path
	}
	method := strings.ToUpper(act.Method)
	pathParams := emitter.ExtractPathParams(routePath)

	// Doc comment: /** METHOD /full/path — description */
	desc := act.Description
	if desc != "" {
		desc = " — " + desc
	}
	if method == "WS" {
		sb.WriteString(fmt.Sprintf("        /** WS %s%s */\n", routePath, desc))
	} else {
		sb.WriteString(fmt.Sprintf("        /** %s %s%s */\n", method, routePath, desc))
	}

	// Return type
	outputType := "Unit"
	if act.Output != "" {
		base := veldTypeToKotlin(act.Output)
		if act.OutputArray {
			outputType = fmt.Sprintf("List<%s>", base)
		} else {
			outputType = base
		}
	}

	// Parameters
	var sigParams []string
	for _, p := range pathParams {
		sigParams = append(sigParams, fmt.Sprintf("%s: String", p))
	}
	if act.Input != "" {
		sigParams = append(sigParams, fmt.Sprintf("input: %s", act.Input))
	}
	if act.Query != "" {
		sigParams = append(sigParams, fmt.Sprintf("query: %s? = null", act.Query))
	}
	if act.Headers != "" {
		sigParams = append(sigParams, "headers: Map<String, String> = emptyMap()")
	}
	sig := strings.Join(sigParams, ", ")

	urlExpr := emitter.ToTemplateLiteral(routePath)

	sb.WriteString(fmt.Sprintf("        fun %s(%s): %s {\n", lcFirst(act.Name), sig, outputType))

	// Query string
	if act.Query != "" {
		sb.WriteString(fmt.Sprintf("            var url = \"%s\"\n", urlExpr))
		sb.WriteString("            // query params omitted for brevity\n")
	}

	bodyArg := "null"
	if act.Input != "" {
		bodyArg = "json.encodeToString(input)"
	}

	path := fmt.Sprintf("\"%s\"", urlExpr)
	if act.Query != "" {
		path = "url"
	}

	// ── WebSocket ─────────────────────────────────────────────────────────
	if method == "WS" {
		sb.WriteString(fmt.Sprintf("            // WebSocket: connect to $baseUrl%s\n", urlExpr))
		sb.WriteString("            throw UnsupportedOperationException(\"WebSocket not yet supported in JVM HTTP client\")\n")
	} else if act.Output == "" {
		sb.WriteString(fmt.Sprintf("            request(\"%s\", %s, %s)\n", method, path, bodyArg))
	} else {
		sb.WriteString(fmt.Sprintf("            val body = request(\"%s\", %s, %s)\n", method, path, bodyArg))
		sb.WriteString("            return json.decodeFromString(body)\n")
	}

	sb.WriteString("        }\n\n")
}

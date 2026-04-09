package swift

// client.go — emits the VeldApi enum namespace with HTTP helper and per-module sub-enums.

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

// emitApiEnum writes the VeldApi namespace enum: HTTP request helper and
// per-module nested enums with SDK methods.
func emitApiEnum(sb *strings.Builder, a ast.AST, opts emitter.EmitOptions) {
	baseUrlDefault := ""
	if opts.BaseUrl != "" {
		baseUrlDefault = opts.BaseUrl
	}
	sb.WriteString(fmt.Sprintf("enum VeldApi {\n    static var baseURL = \"%s\"\n\n", baseUrlDefault))

	// HTTP helper
	sb.WriteString(`    private static func request(_ method: String, _ path: String, body: Data? = nil) async throws -> Data {
        guard let url = URL(string: "\(baseURL)\(path)") else {
            throw VeldApiError(status: 0, body: "Invalid URL")
        }
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body
        let (data, response) = try await URLSession.shared.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw VeldApiError(status: 0, body: "Invalid response")
        }
        if httpResponse.statusCode >= 400 {
            throw VeldApiError(status: httpResponse.statusCode, body: String(data: data, encoding: .utf8) ?? "")
        }
        return data
    }

`)

	// Per-module enums
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("    enum %s {\n", mod.Name))
		for _, act := range mod.Actions {
			writeAction(sb, mod, act)
		}
		sb.WriteString("    }\n\n")
	}

	sb.WriteString("}\n")
}

// writeAction writes a single SDK static method (HTTP or WebSocket).
func writeAction(sb *strings.Builder, mod ast.Module, act ast.Action) {
	routePath := act.Path
	if mod.Prefix != "" {
		routePath = mod.Prefix + act.Path
	}
	method := strings.ToUpper(act.Method)
	pathParams := emitter.ExtractPathParams(routePath)

	// Doc comment: /// METHOD /full/path — description
	desc := act.Description
	if desc != "" {
		desc = " — " + desc
	}
	if method == "WS" {
		sb.WriteString(fmt.Sprintf("        /// WS %s%s\n", routePath, desc))
	} else {
		sb.WriteString(fmt.Sprintf("        /// %s %s%s\n", method, routePath, desc))
	}

	// Return type
	outputType := "Void"
	if act.Output != "" {
		base := veldTypeToSwift(act.Output)
		if act.OutputArray {
			outputType = fmt.Sprintf("[%s]", base)
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
		sigParams = append(sigParams, fmt.Sprintf("query: %s? = nil", act.Query))
	}
	if act.Headers != "" {
		sigParams = append(sigParams, "headers: [String: String] = [:]")
	}
	sig := strings.Join(sigParams, ", ")

	urlExpr := emitter.ToTemplateLiteral(routePath)

	sb.WriteString(fmt.Sprintf("        static func %s(%s) async throws -> %s {\n", lcFirst(act.Name), sig, outputType))

	// ── WebSocket ─────────────────────────────────────────────────────────
	if method == "WS" {
		sb.WriteString(fmt.Sprintf("            // WebSocket: connect to \\(baseURL)%s\n", urlExpr))
		sb.WriteString("            fatalError(\"WebSocket not yet supported\")\n")
	} else {
		bodyArg := "nil"
		if act.Input != "" {
			bodyArg = "try JSONEncoder().encode(input)"
		}

		sb.WriteString(fmt.Sprintf("            let data = try await request(\"%s\", \"%s\", body: %s)\n", method, urlExpr, bodyArg))

		if act.Output != "" {
			sb.WriteString(fmt.Sprintf("            return try JSONDecoder().decode(%s.self, from: data)\n", outputType))
		}
	}

	sb.WriteString("        }\n\n")
}

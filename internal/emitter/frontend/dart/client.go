package dart

// client.go — emits the VeldApi class with HTTP helpers and per-module methods.

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

// emitApiClass writes the VeldApi class: constructor, _request helper, and
// all module methods (HTTP + WebSocket).
func emitApiClass(sb *strings.Builder, a ast.AST, opts emitter.EmitOptions) {
	if opts.BaseUrl != "" {
		sb.WriteString(fmt.Sprintf("class VeldApi {\n  final String baseUrl;\n  final HttpClient _client = HttpClient();\n\n  VeldApi({this.baseUrl = '%s'});\n\n", opts.BaseUrl))
	} else {
		sb.WriteString("class VeldApi {\n  final String baseUrl;\n  final HttpClient _client = HttpClient();\n\n  VeldApi({this.baseUrl = ''});\n\n")
	}

	// HTTP helper
	sb.WriteString(`  Future<Map<String, dynamic>> _request(String method, String path, {Map<String, dynamic>? body}) async {
    final request = await _client.openUrl(method, Uri.parse('$baseUrl$path'));
    request.headers.set('Content-Type', 'application/json');
    if (body != null) {
      request.write(jsonEncode(body));
    }
    final response = await request.close();
    final responseBody = await response.transform(utf8.decoder).join();
    if (response.statusCode >= 400) {
      throw VeldApiError(response.statusCode, responseBody);
    }
    if (responseBody.isEmpty) return {};
    return jsonDecode(responseBody);
  }

`)

	// Per-module methods
	for _, mod := range a.Modules {
		sb.WriteString(fmt.Sprintf("  // — %s\n", mod.Name))
		for _, act := range mod.Actions {
			writeAction(sb, mod, act)
		}
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

	// Doc comment: /// METHOD /full/path — description
	desc := act.Description
	if desc != "" {
		desc = " — " + desc
	}
	if method == "WS" {
		sb.WriteString(fmt.Sprintf("  /// WS %s%s\n", routePath, desc))
	} else {
		sb.WriteString(fmt.Sprintf("  /// %s %s%s\n", method, routePath, desc))
	}

	// ── WebSocket action ──────────────────────────────────────────────────
	if method == "WS" {
		var sigParams []string
		for _, p := range pathParams {
			sigParams = append(sigParams, fmt.Sprintf("String %s", p))
		}
		sig := strings.Join(sigParams, ", ")
		wsPath := emitter.ToTemplateLiteral(routePath)
		sb.WriteString(fmt.Sprintf("  Future<WebSocket> %s(%s) async {\n", lcFirst(act.Name), sig))
		sb.WriteString(fmt.Sprintf("    return WebSocket.connect('$baseUrl%s');\n", wsPath))
		sb.WriteString("  }\n\n")
		return
	}

	// ── HTTP action ───────────────────────────────────────────────────────
	outputType := "void"
	if act.Output != "" {
		base := veldTypeToDart(act.Output)
		if act.OutputArray {
			outputType = fmt.Sprintf("List<%s>", base)
		} else {
			outputType = base
		}
	}

	var sigParams []string
	for _, p := range pathParams {
		sigParams = append(sigParams, fmt.Sprintf("String %s", p))
	}
	if act.Input != "" {
		sigParams = append(sigParams, fmt.Sprintf("%s input", act.Input))
	}
	if act.Query != "" {
		sigParams = append(sigParams, fmt.Sprintf("{%s? query}", act.Query))
	}
	sig := strings.Join(sigParams, ", ")

	urlExpr := emitter.ToTemplateLiteral(routePath)

	sb.WriteString(fmt.Sprintf("  Future<%s> %s(%s) async {\n", outputType, lcFirst(act.Name), sig))

	if act.Query != "" {
		sb.WriteString(fmt.Sprintf("    var url = '%s';\n", urlExpr))
		sb.WriteString("    if (query != null) {\n")
		sb.WriteString("      final qs = query!.toJson().entries.map((e) => '${e.key}=${e.value}').join('&');\n")
		sb.WriteString("      url = '$url?$qs';\n")
		sb.WriteString("    }\n")
	}

	bodyArg := "null"
	if act.Input != "" {
		bodyArg = "input.toJson()"
	}

	path := urlExpr
	if act.Query != "" {
		path = "url"
	} else {
		path = "'" + urlExpr + "'"
	}

	if act.Output == "" {
		sb.WriteString(fmt.Sprintf("    await _request('%s', %s", method, path))
		if act.Input != "" {
			sb.WriteString(fmt.Sprintf(", body: %s", bodyArg))
		}
		sb.WriteString(");\n")
	} else {
		sb.WriteString(fmt.Sprintf("    final json = await _request('%s', %s", method, path))
		if act.Input != "" {
			sb.WriteString(fmt.Sprintf(", body: %s", bodyArg))
		}
		sb.WriteString(");\n")
		if act.OutputArray {
			sb.WriteString(fmt.Sprintf("    return (json as List).map((e) => %s.fromJson(e)).toList();\n", veldTypeToDart(act.Output)))
		} else if emitter.IsPrimitive(act.Output) {
			sb.WriteString("    return json;\n")
		} else {
			sb.WriteString(fmt.Sprintf("    return %s.fromJson(json);\n", veldTypeToDart(act.Output)))
		}
	}
	sb.WriteString("  }\n\n")
}

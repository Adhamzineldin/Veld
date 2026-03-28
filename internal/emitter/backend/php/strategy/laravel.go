package strategy

import (
	"fmt"
	"strings"
)

// LaravelStrategy generates Laravel 10.x controller code.
// It produces classes extending Controller, uses Illuminate\Http\Request,
// and returns response()->json() / response()->noContent() helpers.
type LaravelStrategy struct{}

func (s *LaravelStrategy) ControllerAnnotations() []string { return nil }

func (s *LaravelStrategy) ControllerBaseClass() string { return "Controller" }

func (s *LaravelStrategy) ControllerUses() []string {
	return []string{
		"use Illuminate\\Http\\Request;",
		"use Illuminate\\Http\\JsonResponse;",
	}
}

func (s *LaravelStrategy) RequestType() string { return "Request" }

func (s *LaravelStrategy) ReturnOk(expr string) string {
	return fmt.Sprintf("return response()->json(%s);", expr)
}

func (s *LaravelStrategy) ReturnCreated(expr string) string {
	return fmt.Sprintf("return response()->json(%s, 201);", expr)
}

func (s *LaravelStrategy) ReturnNoContent() string {
	return "return response()->noContent();"
}

func (s *LaravelStrategy) ReturnError(statusCode int, msgExpr string) string {
	return fmt.Sprintf("return response()->json(['error' => %s], %d);", msgExpr, statusCode)
}

func (s *LaravelStrategy) ComposerRequire() map[string]string {
	return map[string]string{
		"php":               "^8.1",
		"laravel/framework": "^10.0",
	}
}

func (s *LaravelStrategy) WSActionMethod(actionName, routePath, emitType, streamType string) string {
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("\n    /** WebSocket handler for %s — use Laravel WebSockets or Reverb */\n", routePath))
	sb.WriteString(fmt.Sprintf("    public function %sConnect($connection, array $params): void\n    {\n", actionName))
	sb.WriteString(fmt.Sprintf("        // implement: $this->service->on%sConnect($connection, $params);\n", capitalize(actionName)))
	sb.WriteString("    }\n")
	if emitType != "" {
		sb.WriteString(fmt.Sprintf("\n    public function %sMessage($connection, array $data): void\n    {\n", actionName))
		sb.WriteString(fmt.Sprintf("        // data is expected to conform to %s\n", emitType))
		sb.WriteString(fmt.Sprintf("        // implement: $this->service->on%sMessage($connection, $data);\n", capitalize(actionName)))
		sb.WriteString("    }\n")
	}
	if streamType != "" {
		sb.WriteString(fmt.Sprintf("\n    public function %sClose($connection): void\n    {\n", actionName))
		sb.WriteString(fmt.Sprintf("        // implement: $this->service->on%sClose($connection);\n", capitalize(actionName)))
		sb.WriteString("    }\n")
	}
	return sb.String()
}

// capitalize returns s with its first letter upper-cased.
func capitalize(s string) string {
	if s == "" {
		return s
	}
	return strings.ToUpper(s[:1]) + s[1:]
}

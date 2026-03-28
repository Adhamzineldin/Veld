package strategy

import (
	"fmt"
	"strings"
)

// PlainStrategy generates pure typed Python with no HTTP framework dependency.
// Route handlers are plain functions annotated with a comment indicating the HTTP method and path.
type PlainStrategy struct{}

func (s *PlainStrategy) RouterImports(moduleName string) []string { return nil }

func (s *PlainStrategy) RouterSetup(moduleName string) string { return "" }

func (s *PlainStrategy) RouteDecorator(moduleName, path, method string) string {
	return fmt.Sprintf("# %s %s", strings.ToUpper(method), path)
}

func (s *PlainStrategy) ExtractBody(inputType string) string {
	return fmt.Sprintf("body = data  # %s", inputType)
}

func (s *PlainStrategy) ReturnOk(expr string) string {
	return fmt.Sprintf("return %s", expr)
}

func (s *PlainStrategy) ReturnCreated(expr string) string {
	return fmt.Sprintf("return %s  # 201", expr)
}

func (s *PlainStrategy) ReturnNoContent() string { return "return None  # 204" }

func (s *PlainStrategy) ReturnError(statusExpr, msgExpr string) string {
	return fmt.Sprintf("raise RuntimeError(%s)", msgExpr)
}

func (s *PlainStrategy) RegisterRoute(moduleName, fnName, flaskPath, methods string) string {
	return fmt.Sprintf("# register: %s %s -> %s", methods, flaskPath, fnName)
}

func (s *PlainStrategy) RequirementsEntries() []string { return nil }

func (s *PlainStrategy) WSHandlerCode(actionName, routePath, streamType, emitType string, pathParams []string) string {
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("\n    # WebSocket: WS %s\n", routePath))
	if len(pathParams) > 0 {
		sb.WriteString(fmt.Sprintf("    # Implement: service.on_%s_connect(client_id, %s)\n", actionName, strings.Join(pathParams, ", ")))
	} else {
		sb.WriteString(fmt.Sprintf("    # Implement: service.on_%s_connect(client_id)\n", actionName))
	}
	if emitType != "" {
		sb.WriteString(fmt.Sprintf("    # Implement: service.on_%s_message(client_id, data: %s)\n", actionName, emitType))
	}
	if streamType != "" {
		sb.WriteString(fmt.Sprintf("    # Broadcast: service.on_%s_stream(client_id) -> %s\n", actionName, streamType))
	}
	sb.WriteString(fmt.Sprintf("    # Implement: service.on_%s_close(client_id)\n", actionName))
	return sb.String()
}

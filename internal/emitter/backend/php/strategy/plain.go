package strategy

import "fmt"

// PlainStrategy generates PHP interfaces only — no Laravel/Symfony dependency.
// Users implement the generated interfaces and wire them into any PHP HTTP layer.
type PlainStrategy struct{}

func (s *PlainStrategy) ControllerAnnotations() []string { return nil }
func (s *PlainStrategy) ControllerBaseClass() string     { return "" }
func (s *PlainStrategy) ControllerUses() []string        { return nil }
func (s *PlainStrategy) RequestType() string             { return "array" }

func (s *PlainStrategy) ReturnOk(expr string) string {
	return fmt.Sprintf("return %s;", expr)
}

func (s *PlainStrategy) ReturnCreated(expr string) string {
	return fmt.Sprintf("return %s; // 201", expr)
}

func (s *PlainStrategy) ReturnNoContent() string { return "// 204 No Content" }

func (s *PlainStrategy) ReturnWithStatus(code int, expr string) string {
	return fmt.Sprintf("return %s; // %d", expr, code)
}

func (s *PlainStrategy) ReturnError(statusCode int, msgExpr string) string {
	return fmt.Sprintf("throw new \\RuntimeException(%s);", msgExpr)
}

func (s *PlainStrategy) ComposerRequire() map[string]string { return nil }

func (s *PlainStrategy) WSActionMethod(actionName, routePath, emitType, streamType string) string {
	msg := fmt.Sprintf("\n    // WebSocket: WS %s\n    // Implement: service->on%sConnect($connection, $params)\n",
		routePath, actionName)
	if emitType != "" {
		msg += fmt.Sprintf("    // Implement: service->on%sMessage($connection, $data /* %s */)\n", actionName, emitType)
	}
	if streamType != "" {
		msg += fmt.Sprintf("    // Broadcast: service->on%sStream($connection) -> %s\n", actionName, streamType)
	}
	msg += fmt.Sprintf("    // Implement: service->on%sClose($connection)\n", actionName)
	return msg
}

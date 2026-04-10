package strategy

import "fmt"

// PlainStrategy generates Rust trait definitions only — no HTTP framework.
// Users implement the traits and wire them into any HTTP server of their choice.
type PlainStrategy struct{}

func (s *PlainStrategy) HandlerImports() []string { return nil }
func (s *PlainStrategy) RouterImports() []string  { return nil }

func (s *PlainStrategy) WrapHandler(method, returnType, serviceCall string, statusCode int) string {
	return serviceCall
}

func (s *PlainStrategy) BuildRouter(routes []RouteEntry) string { return "" }

func (s *PlainStrategy) CargoTomlDependencies() []string {
	return []string{
		`serde = { version = "1", features = ["derive"] }`,
		`serde_json = "1"`,
		`tokio = { version = "1", features = ["full"] }`,
	}
}

func (s *PlainStrategy) MainRsContent() string {
	return "// Wire your service traits into an HTTP framework of your choice.\nfn main() {}\n"
}

func (s *PlainStrategy) WSHandlerCode(handlerName, routePath, serviceName, streamType, emitType string, pathParams []string) string {
	if emitType == "" {
		emitType = "serde_json::Value"
	}
	msg := fmt.Sprintf("// WebSocket: WS %s\n// Implement: service.on_%s_connect(conn, params) -> ()\n", routePath, handlerName)
	msg += fmt.Sprintf("// Implement: service.on_%s_message(conn, msg: %s) -> ()\n", handlerName, emitType)
	if streamType != "" {
		msg += fmt.Sprintf("// Broadcast helper: sends %s to client\n", streamType)
	}
	msg += fmt.Sprintf("// Implement: service.on_%s_close(conn) -> ()\n", handlerName)
	return msg
}

package strategy

import (
	"fmt"
	"strings"
)

// FlaskStrategy generates Flask blueprint route handlers.
// It produces the same output that the Python emitter previously generated directly.
type FlaskStrategy struct{}

func (s *FlaskStrategy) RouterImports(moduleName string) []string {
	return []string{"from flask import request, jsonify"}
}

func (s *FlaskStrategy) RouterSetup(moduleName string) string { return "" }

func (s *FlaskStrategy) RouteDecorator(moduleName, path, method string) string {
	// Flask decorators are not used here — registration is via app.add_url_rule.
	// This method is a no-op for Flask; route registration happens in RegisterRoute.
	return ""
}

func (s *FlaskStrategy) ExtractBody(inputType string) string {
	// Flask body extraction uses request.get_json(); the caller wraps this with
	// either Pydantic schema validation or a zero-dep validator depending on opts.Validate.
	// The strategy only provides the framework-specific raw read.
	return "request.get_json() or {}"
}

func (s *FlaskStrategy) ReturnOk(expr string) string {
	return fmt.Sprintf("return jsonify(%s)", expr)
}

func (s *FlaskStrategy) ReturnCreated(expr string) string {
	return fmt.Sprintf("return jsonify(%s), 201", expr)
}

func (s *FlaskStrategy) ReturnNoContent() string {
	return "return '', 204"
}

func (s *FlaskStrategy) ReturnError(statusExpr, msgExpr string) string {
	return fmt.Sprintf("return jsonify(%s), %s", msgExpr, statusExpr)
}

func (s *FlaskStrategy) RegisterRoute(moduleName, fnName, flaskPath, methods string) string {
	return fmt.Sprintf("app.add_url_rule('%s', '%s', %s, methods=%s)",
		flaskPath, fnName, fnName, methods)
}

func (s *FlaskStrategy) RequirementsEntries() []string {
	return []string{"flask>=3.0.0"}
}

func (s *FlaskStrategy) WSHandlerCode(actionName, routePath, streamType, emitType string, pathParams []string) string {
	// Flask-SocketIO namespace is the route path.
	namespace := routePath
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("\n    @socketio.on('connect', namespace='%s')\n", namespace))
	sb.WriteString(fmt.Sprintf("    def on_%s_connect():\n", actionName))
	if len(pathParams) > 0 {
		sb.WriteString(fmt.Sprintf("        # implement: service.on_%s_connect(request.sid, %s)\n", actionName, strings.Join(pathParams, ", ")))
	} else {
		sb.WriteString(fmt.Sprintf("        # implement: service.on_%s_connect(request.sid)\n", actionName))
	}
	sb.WriteString("        pass\n")
	if emitType != "" {
		sb.WriteString(fmt.Sprintf("\n    @socketio.on('message', namespace='%s')\n", namespace))
		sb.WriteString(fmt.Sprintf("    def on_%s_message(data):\n", actionName))
		sb.WriteString(fmt.Sprintf("        # data is expected to be a %s payload\n", emitType))
		sb.WriteString(fmt.Sprintf("        # implement: service.on_%s_message(request.sid, data)\n", actionName))
		sb.WriteString("        pass\n")
	}
	if streamType != "" {
		sb.WriteString(fmt.Sprintf("\n    @socketio.on('disconnect', namespace='%s')\n", namespace))
		sb.WriteString(fmt.Sprintf("    def on_%s_disconnect():\n", actionName))
		sb.WriteString(fmt.Sprintf("        # implement: service.on_%s_close(request.sid)\n", actionName))
		sb.WriteString("        pass\n")
	}
	return sb.String()
}

// routes.go - Route handler generation for Rust backend
package rustbackend

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	ruststrategy "github.com/Adhamzineldin/Veld/internal/emitter/backend/rust/strategy"
	"github.com/Adhamzineldin/Veld/internal/emitter/codegen"
	"github.com/Adhamzineldin/Veld/internal/emitter/lang"
)

// generateRouter writes src/router.rs with the framework router setup.
func (e *RustEmitter) generateRouter(a ast.AST, strat ruststrategy.RustFrameworkStrategy) []byte {
	w := codegen.NewWriter("    ")
	w.Writeln(header)

	// Emit router imports from the strategy.
	for _, imp := range strat.RouterImports() {
		w.Writeln(fmt.Sprintf("use %s;", imp))
	}
	if len(strat.RouterImports()) > 0 {
		w.BlankLine()
	}

	for _, mod := range a.Modules {
		modName := e.adapter.NamingConvention(mod.Name, lang.NamingContextPrivate)
		w.Writeln(fmt.Sprintf("use crate::%s::router as %s_router;", modName, modName))
	}
	w.BlankLine()

	w.Writeln("/// Build the complete router with all module routes.")
	w.Writeln("pub fn build_router() -> impl std::any::Any {")
	w.Indent()

	// Collect route entries for strategy (WS actions use "GET" for the upgrade request).
	var routes []ruststrategy.RouteEntry
	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			method := act.Method
			if method == "WS" {
				method = "GET" // WebSocket upgrade is always a GET request
			}
			routes = append(routes, ruststrategy.RouteEntry{
				Method:  method,
				Path:    fullAxumPath(mod, act),
				Handler: e.adapter.NamingConvention(act.Name, lang.NamingContextPrivate),
			})
		}
	}

	routerCode := strat.BuildRouter(routes)
	if routerCode != "" {
		w.Writeln(routerCode)
	} else {
		// Plain strategy: merge module sub-routers if available.
		if len(a.Modules) == 0 {
			w.Writeln("()")
		} else {
			parts := make([]string, len(a.Modules))
			for i, mod := range a.Modules {
				modName := e.adapter.NamingConvention(mod.Name, lang.NamingContextPrivate)
				parts[i] = fmt.Sprintf("%s_router()", modName)
			}
			w.Writeln(parts[0])
			for _, p := range parts[1:] {
				w.Writeln(fmt.Sprintf("    .merge(%s)", p))
			}
		}
	}

	w.Dedent()
	w.Writeln("}")
	return w.Bytes()
}

// generateModuleRoutes writes src/{module}/mod.rs with handlers for one module.
func (e *RustEmitter) generateModuleRoutes(a ast.AST, mod ast.Module, strat ruststrategy.RustFrameworkStrategy) ([]byte, error) {
	w := codegen.NewWriter("    ")
	w.Writeln(header)

	// Framework-specific handler imports.
	handlerImports := strat.HandlerImports()
	if len(handlerImports) > 0 {
		// First entry may be a multi-line block opener (e.g. "use axum::{")
		w.Writeln("use " + handlerImports[0])
		for _, imp := range handlerImports[1:] {
			w.Writeln(imp)
		}
	}
	w.BlankLine()
	w.Writeln("use crate::models::*;")
	w.Writeln(fmt.Sprintf("use crate::services::%sService;", e.adapter.NamingConvention(mod.Name, lang.NamingContextExported)))
	w.BlankLine()

	// router() function.
	w.Writeln(fmt.Sprintf("/// Build the sub-router for the %s module.", mod.Name))
	w.Writeln("pub fn router<S>() -> impl std::any::Any")
	w.Writeln("where")
	w.Indent()
	w.Writeln(fmt.Sprintf("S: %sService + Clone + Send + Sync + 'static,", e.adapter.NamingConvention(mod.Name, lang.NamingContextExported)))
	w.Dedent()
	w.WriteBlock("{")

	// Build route entries from this module (skip WS — handled separately).
	var routes []ruststrategy.RouteEntry
	for _, act := range mod.Actions {
		if act.Method == "WS" {
			continue
		}
		routePath := fullAxumPath(mod, act)
		handlerName := e.adapter.NamingConvention(act.Name, lang.NamingContextPrivate)
		routes = append(routes, ruststrategy.RouteEntry{
			Method:  act.Method,
			Path:    routePath,
			Handler: handlerName,
		})
	}

	routerCode := strat.BuildRouter(routes)
	if routerCode != "" {
		w.Writeln(routerCode)
	} else {
		w.Writeln("()")
	}

	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// Handler functions (HTTP) and WS handler stubs.
	for _, act := range mod.Actions {
		w.BlankLine()
		if act.Method == "WS" {
			routePath := fullAxumPath(mod, act)
			handlerName := e.adapter.NamingConvention(act.Name, lang.NamingContextPrivate)
			serviceName := e.adapter.NamingConvention(mod.Name, lang.NamingContextExported) + "Service"
			pathParams := emitter.ExtractPathParams(routePath)
			wsCode := strat.WSHandlerCode(handlerName, routePath, serviceName, act.Stream, act.Emit, pathParams)
			for _, line := range strings.Split(strings.TrimRight(wsCode, "\n"), "\n") {
				w.Writeln(line)
			}
		} else {
			if err := e.writeHandler(w, mod, act, strat); err != nil {
				return nil, err
			}
		}
	}

	return w.Bytes(), nil
}

// writeHandler generates a single async handler function.
func (e *RustEmitter) writeHandler(w *codegen.Writer, mod ast.Module, act ast.Action, strat ruststrategy.RustFrameworkStrategy) error {
	handlerName := e.adapter.NamingConvention(act.Name, lang.NamingContextPrivate)
	methodName := e.adapter.NamingConvention(act.Name, lang.NamingContextPrivate)
	serviceName := e.adapter.NamingConvention(mod.Name, lang.NamingContextExported) + "Service"
	routePath := fullAxumPath(mod, act)
	pathParams := emitter.ExtractPathParams(routePath)

	hasInput := act.Input != ""
	hasOutput := act.Output != ""

	// Build the function signature parameters.
	var sigParams []string
	sigParams = append(sigParams, fmt.Sprintf("State(svc): State<Arc<dyn %s>>", serviceName))

	if len(pathParams) > 0 {
		if len(pathParams) == 1 {
			paramType := "String"
			sigParams = append(sigParams, fmt.Sprintf("Path(%s): Path<%s>", rustParamName(e, pathParams[0]), paramType))
		} else {
			types := make([]string, len(pathParams))
			names := make([]string, len(pathParams))
			for i, p := range pathParams {
				types[i] = "String"
				names[i] = rustParamName(e, p)
			}
			sigParams = append(sigParams, fmt.Sprintf("Path((%s)): Path<(%s)>", strings.Join(names, ", "), strings.Join(types, ", ")))
		}
	}

	if hasInput {
		sigParams = append(sigParams, fmt.Sprintf("Json(payload): Json<%s>", e.adapter.NamingConvention(act.Input, lang.NamingContextExported)))
	}

	// Return type.
	var returnType string
	if hasOutput {
		outputType := e.adapter.NamingConvention(act.Output, lang.NamingContextExported)
		if act.OutputArray {
			returnType = fmt.Sprintf("Result<(StatusCode, Json<Vec<%s>>), (StatusCode, Json<serde_json::Value>)>", outputType)
		} else {
			returnType = fmt.Sprintf("Result<(StatusCode, Json<%s>), (StatusCode, Json<serde_json::Value>)>", outputType)
		}
	} else {
		returnType = "Result<StatusCode, (StatusCode, Json<serde_json::Value>)>"
	}

	if act.Description != "" {
		w.Writeln(fmt.Sprintf("/// %s", act.Description))
	} else {
		w.Writeln(fmt.Sprintf("/// Handle %s %s.", strings.ToUpper(act.Method), routePath))
	}

	w.WriteBlock(fmt.Sprintf("pub async fn %s(%s) -> %s {", handlerName, strings.Join(sigParams, ", "), returnType))

	// Build service call arguments.
	var callArgs []string
	for _, p := range pathParams {
		callArgs = append(callArgs, rustParamName(e, p))
	}
	if hasInput {
		callArgs = append(callArgs, "payload")
	}
	if act.Headers != "" {
		// TODO: extract headers from axum HeaderMap once full Axum extractor is wired
		callArgs = append(callArgs, fmt.Sprintf("%s::default()", e.adapter.NamingConvention(act.Headers, lang.NamingContextExported)))
	}

	serviceCallExpr := fmt.Sprintf("svc.%s(%s).await", methodName, strings.Join(callArgs, ", "))
	wrapped := strat.WrapHandler(act.Method, returnType, serviceCallExpr)

	// Error helper closure (only needed when the strategy generates response code).
	if wrapped != serviceCallExpr {
		w.Writeln("let err_response = |e: String| {")
		w.Indent()
		w.Writeln("(StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({\"error\": e})))")
		w.Dedent()
		w.Writeln("};")
		w.BlankLine()
	}

	for _, line := range strings.Split(wrapped, "\n") {
		w.Writeln(line)
	}

	w.Dedent()
	w.Writeln("}")
	return nil
}

// generateServices writes src/services.rs with async trait definitions.
func (e *RustEmitter) generateServices(a ast.AST) ([]byte, error) {
	w := codegen.NewWriter("    ")
	w.Writeln(header)

	w.Writeln("use async_trait::async_trait;")
	w.BlankLine()
	w.Writeln("use crate::models::*;")
	w.BlankLine()

	for _, mod := range a.Modules {
		serviceName := e.adapter.NamingConvention(mod.Name, lang.NamingContextExported) + "Service"
		w.Writeln(fmt.Sprintf("/// %sService defines the contract for %s business logic.", e.adapter.NamingConvention(mod.Name, lang.NamingContextExported), mod.Name))
		w.Writeln("/// Implement this trait in your application code.")
		w.Writeln("#[async_trait]")
		w.WriteBlock(fmt.Sprintf("pub trait %s: Send + Sync {", serviceName))

		for _, act := range mod.Actions {
			if act.Method == "WS" {
				e.writeWSTraitMethods(w, mod, act)
				continue
			}
			sig, err := e.buildTraitMethod(mod, act)
			if err != nil {
				return nil, err
			}
			if act.Description != "" {
				w.Writeln(fmt.Sprintf("    /// %s", act.Description))
			}
			for _, errName := range act.Errors {
				code := emitter.ErrorCode(act.Name, errName)
				w.Writeln(fmt.Sprintf("    /// # Errors\n    /// Returns `%sError::%s` — %s", act.Name, emitter.ToCamelCase(errName), code))
			}
			w.Writeln(sig)
		}

		w.Dedent()
		w.Writeln("}")
		w.BlankLine()
	}

	return w.Bytes(), nil
}

// writeWSTraitMethods appends WS lifecycle trait methods for a WS action to w.
func (e *RustEmitter) writeWSTraitMethods(w *codegen.Writer, mod ast.Module, act ast.Action) {
	methodName := e.adapter.NamingConvention(act.Name, lang.NamingContextPrivate)
	routePath := fullAxumPath(mod, act)
	pathParams := emitter.ExtractPathParams(routePath)

	// on_{action}_connect
	var connectParams []string
	connectParams = append(connectParams, "&self")
	connectParams = append(connectParams, "socket: axum::extract::ws::WebSocket")
	for _, p := range pathParams {
		connectParams = append(connectParams, fmt.Sprintf("%s: String", rustParamName(e, p)))
	}
	if act.Description != "" {
		w.Writeln(fmt.Sprintf("    /// %s — called on WebSocket connect.", act.Description))
	} else {
		w.Writeln(fmt.Sprintf("    /// Called when a client opens the WS %s connection.", routePath))
	}
	w.Writeln(fmt.Sprintf("    async fn on_%s_connect(%s) -> Result<(), Box<dyn std::error::Error>>;", methodName, strings.Join(connectParams, ", ")))

	// on_{action}_message — only when emit type is set
	if act.Emit != "" {
		emitType := mapRustOutputType(e, act.Emit)
		w.Writeln(fmt.Sprintf("    /// Called when a client sends a %s message.", act.Emit))
		w.Writeln(fmt.Sprintf("    async fn on_%s_message(&self, socket: &mut axum::extract::ws::WebSocket, msg: %s) -> Result<(), Box<dyn std::error::Error>>;", methodName, emitType))
	}

	// on_{action}_close — always included
	w.Writeln(fmt.Sprintf("    /// Called when the WS %s connection is closed.", routePath))
	w.Writeln(fmt.Sprintf("    async fn on_%s_close(&self, socket: axum::extract::ws::WebSocket) -> Result<(), Box<dyn std::error::Error>>;", methodName))
}

// buildTraitMethod builds the async trait method signature for an action.
func (e *RustEmitter) buildTraitMethod(mod ast.Module, act ast.Action) (string, error) {
	methodName := e.adapter.NamingConvention(act.Name, lang.NamingContextPrivate)
	routePath := fullAxumPath(mod, act)
	pathParams := emitter.ExtractPathParams(routePath)

	var params []string
	params = append(params, "&self")

	for _, p := range pathParams {
		params = append(params, fmt.Sprintf("%s: String", rustParamName(e, p)))
	}
	if act.Input != "" {
		inputType := e.adapter.NamingConvention(act.Input, lang.NamingContextExported)
		params = append(params, fmt.Sprintf("input: %s", inputType))
	}
	if act.Headers != "" {
		headersType := e.adapter.NamingConvention(act.Headers, lang.NamingContextExported)
		params = append(params, fmt.Sprintf("headers: %s", headersType))
	}

	returnType := buildRustReturnType(e, act)
	return fmt.Sprintf("    async fn %s(%s) -> %s;", methodName, strings.Join(params, ", "), returnType), nil
}

// buildRustReturnType returns the Result type for a trait method.
func buildRustReturnType(e *RustEmitter, act ast.Action) string {
	if act.Output == "" {
		return "Result<(), Box<dyn std::error::Error>>"
	}
	outputType := mapRustOutputType(e, act.Output)
	if act.OutputArray {
		return fmt.Sprintf("Result<Vec<%s>, Box<dyn std::error::Error>>", outputType)
	}
	return fmt.Sprintf("Result<%s, Box<dyn std::error::Error>>", outputType)
}

// mapRustOutputType maps a Veld output type to its Rust equivalent.
func mapRustOutputType(e *RustEmitter, t string) string {
	switch t {
	case "string", "uuid", "date", "datetime", "decimal":
		return "String"
	case "int":
		return "i32"
	case "float":
		return "f64"
	case "bool":
		return "bool"
	case "any", "json":
		return "serde_json::Value"
	default:
		return e.adapter.NamingConvention(t, lang.NamingContextExported)
	}
}

// ── helpers ───────────────────────────────────────────────────────────────────

// fullAxumPath joins the module prefix and action path, converting :param to :param
// (Axum uses the same `:param` syntax as Veld — no conversion needed).
func fullAxumPath(mod ast.Module, act ast.Action) string {
	if mod.Prefix != "" {
		return mod.Prefix + act.Path
	}
	return act.Path
}

// axumMethod maps a Veld HTTP method to the Axum routing function name.
func axumMethod(method string) string {
	switch strings.ToUpper(method) {
	case "GET":
		return "get"
	case "POST":
		return "post"
	case "PUT":
		return "put"
	case "DELETE":
		return "delete"
	case "PATCH":
		return "patch"
	default:
		return "get"
	}
}

// successStatusCode returns the Axum StatusCode constant for a successful response.
func successStatusCode(act ast.Action) string {
	if strings.ToUpper(act.Method) == "POST" {
		return "StatusCode::CREATED"
	}
	return "StatusCode::OK"
}

// rustParamName converts a path param name to a Rust snake_case variable name.
func rustParamName(e *RustEmitter, param string) string {
	return e.adapter.NamingConvention(param, lang.NamingContextPrivate)
}

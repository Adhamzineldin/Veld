// routes.go - Axum route handler generation for Rust backend
package rustbackend

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/codegen"
	"github.com/Adhamzineldin/Veld/internal/emitter/lang"
)

// generateRouter writes src/router.rs with the Axum router setup.
func (e *RustEmitter) generateRouter(a ast.AST) []byte {
	w := codegen.NewWriter("    ")
	w.Writeln(header)

	w.Writeln("use axum::Router;")
	w.BlankLine()
	for _, mod := range a.Modules {
		modName := e.adapter.NamingConvention(mod.Name, lang.NamingContextPrivate)
		w.Writeln(fmt.Sprintf("use crate::%s::router as %s_router;", modName, modName))
	}
	w.BlankLine()

	w.Writeln("/// Build the complete Axum router with all module routes.")
	w.Writeln("pub fn build_router() -> Router {")
	w.Indent()

	if len(a.Modules) == 0 {
		w.Writeln("Router::new()")
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

	w.Dedent()
	w.Writeln("}")
	return w.Bytes()
}

// generateModuleRoutes writes src/{module}/mod.rs with Axum handlers for one module.
func (e *RustEmitter) generateModuleRoutes(a ast.AST, mod ast.Module) ([]byte, error) {
	w := codegen.NewWriter("    ")
	w.Writeln(header)

	// Standard Axum imports.
	w.Writeln("use axum::{")
	w.Indent()
	w.Writeln("extract::{Json, Path, State},")
	w.Writeln("http::StatusCode,")
	w.Writeln("response::IntoResponse,")
	w.Writeln("routing::{delete, get, patch, post, put},")
	w.Writeln("Router,")
	w.Dedent()
	w.Writeln("};")
	w.Writeln("use std::sync::Arc;")
	w.BlankLine()
	w.Writeln("use crate::models::*;")
	w.Writeln(fmt.Sprintf("use crate::services::%sService;", e.adapter.NamingConvention(mod.Name, lang.NamingContextExported)))
	w.BlankLine()

	// router() function.
	w.Writeln(fmt.Sprintf("/// Build the Axum sub-router for the %s module.", mod.Name))
	w.Writeln("pub fn router<S>() -> Router<S>")
	w.Writeln("where")
	w.Indent()
	w.Writeln(fmt.Sprintf("S: %sService + Clone + Send + Sync + 'static,", e.adapter.NamingConvention(mod.Name, lang.NamingContextExported)))
	w.Dedent()
	w.WriteBlock("{")
	w.Writeln("Router::new()")
	w.Indent()

	for _, act := range mod.Actions {
		routePath := fullAxumPath(mod, act)
		axumFn := axumMethod(act.Method)
		handlerName := e.adapter.NamingConvention(act.Name, lang.NamingContextPrivate)
		w.Writeln(fmt.Sprintf(".route(%q, %s(%s))", routePath, axumFn, handlerName))
	}

	w.Dedent()
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// Handler functions.
	for _, act := range mod.Actions {
		if err := e.writeHandler(w, mod, act); err != nil {
			return nil, err
		}
		w.BlankLine()
	}

	return w.Bytes(), nil
}

// writeHandler generates a single async Axum handler function.
func (e *RustEmitter) writeHandler(w *codegen.Writer, mod ast.Module, act ast.Action) error {
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
		// Axum Path extractor for path parameters.
		if len(pathParams) == 1 {
			paramType := "String"
			sigParams = append(sigParams, fmt.Sprintf("Path(%s): Path<%s>", rustParamName(e, pathParams[0]), paramType))
		} else {
			// Multiple path params use a tuple.
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

	serviceCallStr := fmt.Sprintf("svc.%s(%s).await", methodName, strings.Join(callArgs, ", "))

	// Error helper closure.
	w.Writeln("let err_response = |e: String| {")
	w.Indent()
	w.Writeln("(StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({\"error\": e})))")
	w.Dedent()
	w.Writeln("};")
	w.BlankLine()

	if hasOutput {
		w.Writeln(fmt.Sprintf("let result = %s.map_err(|e| err_response(e.to_string()))?;", serviceCallStr))
		statusCode := successStatusCode(act)
		w.Writeln(fmt.Sprintf("Ok((%s, Json(result)))", statusCode))
	} else if strings.ToUpper(act.Method) == "DELETE" {
		w.Writeln(fmt.Sprintf("%s.map_err(|e| err_response(e.to_string()))?;", serviceCallStr))
		w.Writeln("Ok(StatusCode::NO_CONTENT)")
	} else {
		w.Writeln(fmt.Sprintf("%s.map_err(|e| err_response(e.to_string()))?;", serviceCallStr))
		w.Writeln("Ok(StatusCode::OK)")
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
			sig, err := e.buildTraitMethod(mod, act)
			if err != nil {
				return nil, err
			}
			if act.Description != "" {
				w.Writeln(fmt.Sprintf("    /// %s", act.Description))
			}
			w.Writeln(sig)
		}

		w.Dedent()
		w.Writeln("}")
		w.BlankLine()
	}

	return w.Bytes(), nil
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

	returnType := buildRustReturnType(e, act)
	return fmt.Sprintf("    async fn %s(%s) -> %s;", methodName, strings.Join(params, ", "), returnType), nil
}

// buildRustReturnType returns the Result type for a trait method.
func buildRustReturnType(e *RustEmitter, act ast.Action) string {
	if act.Output == "" {
		return "Result<(), Box<dyn std::error::Error>>"
	}
	outputType := e.adapter.NamingConvention(act.Output, lang.NamingContextExported)
	if act.OutputArray {
		return fmt.Sprintf("Result<Vec<%s>, Box<dyn std::error::Error>>", outputType)
	}
	return fmt.Sprintf("Result<%s, Box<dyn std::error::Error>>", outputType)
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

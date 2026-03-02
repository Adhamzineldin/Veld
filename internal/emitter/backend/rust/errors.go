// errors.go — emits src/errors.rs with Axum-compatible error types and
// per-module typed error constructors.
package rustbackend

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/codegen"
)

// generateErrors returns bytes for src/errors.rs with AppError + IntoResponse impl.
func (e *RustEmitter) generateErrors() []byte {
	w := codegen.NewWriter("    ")
	w.Writeln(header)
	w.Writeln("use axum::{")
	w.Writeln("    http::StatusCode,")
	w.Writeln("    response::{IntoResponse, Response},")
	w.Writeln("    Json,")
	w.Writeln("};")
	w.Writeln("use serde::Serialize;")
	w.BlankLine()

	// ApiErrorResponse
	w.Writeln("/// Standard API error response body.")
	w.Writeln("#[derive(Debug, Clone, Serialize)]")
	w.WriteBlock("pub struct ApiErrorResponse {")
	w.Writeln("pub error: String,")
	w.Writeln(`#[serde(skip_serializing_if = "String::is_empty")]`)
	w.Writeln("pub code: String,")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// AppError struct (replaces enum for richer error data)
	w.Writeln("/// Application error type that converts into HTTP responses.")
	w.Writeln("#[derive(Debug)]")
	w.WriteBlock("pub struct AppError {")
	w.Writeln("pub status: StatusCode,")
	w.Writeln("pub message: String,")
	w.Writeln("pub code: String,")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// Constructor helpers
	w.WriteBlock("impl AppError {")
	w.WriteBlock("pub fn new(status: StatusCode, message: impl Into<String>, code: impl Into<String>) -> Self {")
	w.Writeln("Self { status, message: message.into(), code: code.into() }")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()
	w.WriteBlock("pub fn not_found(message: impl Into<String>) -> Self {")
	w.Writeln(`Self::new(StatusCode::NOT_FOUND, message, "")`)
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()
	w.WriteBlock("pub fn bad_request(message: impl Into<String>) -> Self {")
	w.Writeln(`Self::new(StatusCode::BAD_REQUEST, message, "")`)
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()
	w.WriteBlock("pub fn internal(message: impl Into<String>) -> Self {")
	w.Writeln(`Self::new(StatusCode::INTERNAL_SERVER_ERROR, message, "")`)
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// IntoResponse impl
	w.WriteBlock("impl IntoResponse for AppError {")
	w.WriteBlock("fn into_response(self) -> Response {")
	w.WriteBlock("let body = ApiErrorResponse {")
	w.Writeln("error: self.message,")
	w.Writeln("code: self.code,")
	w.Dedent()
	w.Writeln("};")
	w.BlankLine()
	w.Writeln("(self.status, Json(body)).into_response()")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// From<String> impl
	w.WriteBlock("impl From<String> for AppError {")
	w.WriteBlock("fn from(msg: String) -> Self {")
	w.Writeln("AppError::internal(msg)")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")

	return w.Bytes()
}

// generateModuleErrors returns bytes for src/{module}_errors.rs with typed error factories.
func (e *RustEmitter) generateModuleErrors(mod ast.Module) []byte {
	if !emitter.HasErrors(mod) {
		return nil
	}

	w := codegen.NewWriter("    ")
	w.Writeln(header)
	w.Writeln("use axum::http::StatusCode;")
	w.Writeln("use crate::errors::AppError;")
	w.BlankLine()

	moduleLower := strings.ToLower(mod.Name)
	_ = moduleLower // used only for file naming

	for _, act := range mod.Actions {
		if len(act.Errors) == 0 {
			continue
		}
		for _, errName := range act.Errors {
			code := emitter.ErrorCode(act.Name, errName)
			status := emitter.ErrorHTTPStatus(errName)
			fnName := emitter.ToSnakeCase(act.Name) + "_" + emitter.ToSnakeCase(errName)
			rustStatus := rustStatusCode(status)

			w.Writeln(fmt.Sprintf("/// Create a %s error for the %s action.", errName, act.Name))
			w.WriteBlock(fmt.Sprintf("pub fn %s(message: impl Into<String>) -> AppError {", fnName))
			w.Writeln(fmt.Sprintf(`AppError::new(%s, message, "%s")`, rustStatus, code))
			w.Dedent()
			w.Writeln("}")
			w.BlankLine()
		}
	}

	return w.Bytes()
}

func rustStatusCode(status int) string {
	switch status {
	case 400:
		return "StatusCode::BAD_REQUEST"
	case 401:
		return "StatusCode::UNAUTHORIZED"
	case 403:
		return "StatusCode::FORBIDDEN"
	case 404:
		return "StatusCode::NOT_FOUND"
	case 409:
		return "StatusCode::CONFLICT"
	case 410:
		return "StatusCode::GONE"
	case 422:
		return "StatusCode::UNPROCESSABLE_ENTITY"
	case 429:
		return "StatusCode::TOO_MANY_REQUESTS"
	case 501:
		return "StatusCode::NOT_IMPLEMENTED"
	case 503:
		return "StatusCode::SERVICE_UNAVAILABLE"
	default:
		return "StatusCode::INTERNAL_SERVER_ERROR"
	}
}

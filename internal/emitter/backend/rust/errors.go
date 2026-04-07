// errors.go — emits src/errors.rs with Axum-compatible error types,
// 7 convenience constructors, helper methods, and per-module typed error factories.
// Matches the comprehensive approach used by the TypeScript/Node emitter.
package rustbackend

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/codegen"
)

// generateErrors returns bytes for src/errors.rs with AppError + IntoResponse impl
// + 7 convenience constructors + helper methods.
func (e *RustEmitter) generateErrors() []byte {
	w := codegen.NewWriter("    ")
	w.Writeln(header)
	w.Writeln("use axum::{")
	w.Writeln("    http::StatusCode,")
	w.Writeln("    response::{IntoResponse, Response},")
	w.Writeln("    Json,")
	w.Writeln("};")
	w.Writeln("use serde::Serialize;")
	w.Writeln("use std::fmt;")
	w.BlankLine()

	// ── ApiErrorResponse ─────────────────────────────────────────────────
	w.Writeln("/// Standard API error response body.")
	w.Writeln("#[derive(Debug, Clone, Serialize)]")
	w.WriteBlock("pub struct ApiErrorResponse {")
	w.Writeln("pub error: String,")
	w.Writeln(`#[serde(skip_serializing_if = "String::is_empty")]`)
	w.Writeln("pub code: String,")
	w.Writeln("pub status: u16,")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// ── AppError struct ──────────────────────────────────────────────────
	w.Writeln("/// Application error type that converts into HTTP responses.")
	w.Writeln("/// Every Veld-generated error factory returns AppError.")
	w.Writeln("#[derive(Debug)]")
	w.WriteBlock("pub struct AppError {")
	w.Writeln("pub status: StatusCode,")
	w.Writeln("pub message: String,")
	w.Writeln("pub code: String,")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// ── Display impl ─────────────────────────────────────────────────────
	w.WriteBlock("impl fmt::Display for AppError {")
	w.WriteBlock("fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {")
	w.Writeln(`write!(f, "{}: {}", self.code, self.message)`)
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// ── std::error::Error impl ───────────────────────────────────────────
	w.Writeln("impl std::error::Error for AppError {}")
	w.BlankLine()

	// ── Constructor + convenience methods + helpers ───────────────────────
	w.WriteBlock("impl AppError {")

	// new()
	w.WriteBlock("pub fn new(status: StatusCode, message: impl Into<String>, code: impl Into<String>) -> Self {")
	w.Writeln("Self { status, message: message.into(), code: code.into() }")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// 7 convenience constructors
	type errDef struct {
		name      string
		status    string
		msg       string
		codeConst string
	}
	defs := []errDef{
		{"not_found", "StatusCode::NOT_FOUND", "Not Found", "NOT_FOUND"},
		{"bad_request", "StatusCode::BAD_REQUEST", "Bad Request", "BAD_REQUEST"},
		{"unauthorized", "StatusCode::UNAUTHORIZED", "Unauthorized", "UNAUTHORIZED"},
		{"forbidden", "StatusCode::FORBIDDEN", "Forbidden", "FORBIDDEN"},
		{"conflict", "StatusCode::CONFLICT", "Conflict", "CONFLICT"},
		{"validation_failed", "StatusCode::UNPROCESSABLE_ENTITY", "Validation Failed", "VALIDATION_FAILED"},
		{"internal", "StatusCode::INTERNAL_SERVER_ERROR", "Internal Server Error", "INTERNAL_SERVER_ERROR"},
	}

	for _, d := range defs {
		w.Writeln(fmt.Sprintf("/// Create a %s error. Defaults to %q if no message provided.", d.msg, d.msg))
		w.WriteBlock(fmt.Sprintf("pub fn %s(message: impl Into<String>) -> Self {", d.name))
		w.Writeln(fmt.Sprintf(`let msg: String = message.into();`))
		w.Writeln(fmt.Sprintf(`let msg = if msg.is_empty() { "%s".to_string() } else { msg };`, d.msg))
		w.Writeln(fmt.Sprintf(`Self::new(%s, msg, "%s")`, d.status, d.codeConst))
		w.Dedent()
		w.Writeln("}")
		w.BlankLine()
	}

	// ── Helper methods ───────────────────────────────────────────────────
	w.Writeln("// ── Helper Methods ──────────────────────────────────────────────")
	w.BlankLine()

	w.Writeln("/// Check if this error matches a specific error code.")
	w.WriteBlock("pub fn is_error_code(&self, code: &str) -> bool {")
	w.Writeln("self.code == code")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	w.Writeln("/// Check if this error has a specific HTTP status.")
	w.WriteBlock("pub fn is_http_status(&self, status: StatusCode) -> bool {")
	w.Writeln("self.status == status")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	w.Writeln("/// Convert to a JSON-safe ApiErrorResponse.")
	w.WriteBlock("pub fn to_error_response(&self) -> ApiErrorResponse {")
	w.WriteBlock("ApiErrorResponse {")
	w.Writeln("error: self.message.clone(),")
	w.Writeln("code: self.code.clone(),")
	w.Writeln("status: self.status.as_u16(),")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// ── Free-standing helper functions ────────────────────────────────────
	w.Writeln("// ── Free-standing Helpers ───────────────────────────────────────")
	w.BlankLine()

	w.Writeln("/// Check if a boxed error is an AppError.")
	w.WriteBlock("pub fn is_app_error(err: &dyn std::error::Error) -> bool {")
	w.Writeln("err.downcast_ref::<AppError>().is_some()")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	w.Writeln("/// Convert any error into an ApiErrorResponse.")
	w.Writeln("/// AppError instances preserve their code and status;")
	w.Writeln("/// other errors become 500 INTERNAL_SERVER_ERROR.")
	w.WriteBlock("pub fn to_error_response(err: &dyn std::error::Error) -> ApiErrorResponse {")
	w.WriteBlock("if let Some(app_err) = err.downcast_ref::<AppError>() {")
	w.Writeln("return app_err.to_error_response();")
	w.Dedent()
	w.Writeln("}")
	w.WriteBlock("ApiErrorResponse {")
	w.Writeln("error: err.to_string(),")
	w.Writeln(`code: "INTERNAL_ERROR".to_string(),`)
	w.Writeln("status: 500,")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// ── IntoResponse impl ────────────────────────────────────────────────
	w.WriteBlock("impl IntoResponse for AppError {")
	w.WriteBlock("fn into_response(self) -> Response {")
	w.Writeln("let body = self.to_error_response();")
	w.Writeln("(self.status, Json(body)).into_response()")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// ── From<String> impl ────────────────────────────────────────────────
	w.WriteBlock("impl From<String> for AppError {")
	w.WriteBlock("fn from(msg: String) -> Self {")
	w.Writeln("AppError::internal(msg)")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// ── From<&str> impl ──────────────────────────────────────────────────
	w.WriteBlock("impl From<&str> for AppError {")
	w.WriteBlock("fn from(msg: &str) -> Self {")
	w.Writeln("AppError::internal(msg)")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")

	return w.Bytes()
}

// generateModuleErrors returns bytes for src/{module}_errors.rs with nested
// module-per-action pattern — mirrors TS: users_errors::get_user::not_found("msg").
func (e *RustEmitter) generateModuleErrors(mod ast.Module) []byte {
	if !emitter.HasErrors(mod) {
		return nil
	}

	w := codegen.NewWriter("    ")
	w.Writeln(header)
	w.BlankLine()

	for _, act := range mod.Actions {
		if len(act.Errors) == 0 {
			continue
		}
		snakeAction := emitter.ToSnakeCase(act.Name)
		w.Writeln(fmt.Sprintf("/// Error factories and constants for %s — mirrors TS `%sErrors.%s.error(\"msg\")`.",
			act.Name, strings.ToLower(mod.Name), emitter.ToCamelCase(act.Name)))
		w.WriteBlock(fmt.Sprintf("pub mod %s {", snakeAction))
		w.Writeln("use axum::http::StatusCode;")
		w.Writeln("use crate::errors::AppError;")
		w.BlankLine()
		for _, errName := range act.Errors {
			code := emitter.ErrorCode(act.Name, errName)
			status := emitter.ActionErrorStatus(act, errName)
			snakeErr := emitter.ToSnakeCase(errName)
			constCode := emitter.ToScreamingSnake(errName) + "_CODE"
			constStatus := emitter.ToScreamingSnake(errName) + "_STATUS"
			rustStatus := rustStatusCode(status)
			w.Writeln(fmt.Sprintf("/// mirrors TS .%s.code", snakeErr))
			w.Writeln(fmt.Sprintf("pub const %s: &str = \"%s\";", constCode, code))
			w.Writeln(fmt.Sprintf("pub const %s: u16 = %d;", constStatus, status))
			w.BlankLine()
			w.Writeln(fmt.Sprintf("pub fn %s(message: impl Into<String>) -> AppError {", snakeErr))
			w.Writeln(fmt.Sprintf("    AppError::new(%s, message, %s)", rustStatus, constCode))
			w.Writeln("}")
			w.BlankLine()
		}
		w.Dedent()
		w.Writeln("}")
		w.BlankLine()
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

// generateModuleMiddleware returns bytes for src/{module}_middleware.rs with
// a typed trait per module — one method per middleware name.
func (e *RustEmitter) generateModuleMiddleware(mod ast.Module) []byte {
	allMiddleware := emitter.CollectModuleMiddleware(mod)
	if len(allMiddleware) == 0 {
		return nil
	}

	w := codegen.NewWriter("    ")
	w.Writeln(header)
	w.Writeln("use axum::{")
	w.Writeln("    extract::Request,")
	w.Writeln("    middleware::Next,")
	w.Writeln("    response::Response,")
	w.Writeln("};")
	w.Writeln("use std::future::Future;")
	w.BlankLine()

	traitName := mod.Name + "Middleware"
	w.Writeln(fmt.Sprintf("/// Middleware trait for the %s module.", mod.Name))
	w.Writeln("/// Implement this trait and register it with the router.")
	w.WriteBlock(fmt.Sprintf("pub trait %s: Send + Sync + 'static {", traitName))

	for _, mw := range allMiddleware {
		snakeMw := emitter.ToSnakeCase(mw)
		w.Writeln(fmt.Sprintf("/// %s middleware handler.", mw))
		w.Writeln(fmt.Sprintf("fn %s(&self, request: Request, next: Next) -> impl Future<Output = Response> + Send;", snakeMw))
		w.BlankLine()
	}

	w.Dedent()
	w.Writeln("}")

	return w.Bytes()
}

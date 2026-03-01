// errors.go — emits src/errors.rs with Axum-compatible error types.
package rustbackend

import (
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
	w.Writeln("pub code: u16,")
	w.Writeln("pub message: String,")
	w.Writeln(`#[serde(skip_serializing_if = "Option::is_none")]`)
	w.Writeln("pub details: Option<String>,")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// AppError enum
	w.Writeln("/// Application error type that converts into HTTP responses.")
	w.Writeln("#[derive(Debug)]")
	w.WriteBlock("pub enum AppError {")
	w.Writeln("BadRequest(String),")
	w.Writeln("NotFound(String),")
	w.Writeln("ValidationFailed(String),")
	w.Writeln("Internal(String),")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// IntoResponse impl
	w.WriteBlock("impl IntoResponse for AppError {")
	w.WriteBlock("fn into_response(self) -> Response {")
	w.WriteBlock("let (status, message) = match &self {")
	w.Writeln(`AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),`)
	w.Writeln(`AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),`)
	w.Writeln(`AppError::ValidationFailed(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),`)
	w.Writeln(`AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),`)
	w.Dedent()
	w.Writeln("};")
	w.BlankLine()
	w.WriteBlock("let body = ApiErrorResponse {")
	w.Writeln("code: status.as_u16(),")
	w.Writeln("message,")
	w.Writeln("details: None,")
	w.Dedent()
	w.Writeln("};")
	w.BlankLine()
	w.Writeln("(status, Json(body)).into_response()")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// From<String> impl
	w.WriteBlock("impl From<String> for AppError {")
	w.WriteBlock("fn from(msg: String) -> Self {")
	w.Writeln("AppError::Internal(msg)")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")

	return w.Bytes()
}

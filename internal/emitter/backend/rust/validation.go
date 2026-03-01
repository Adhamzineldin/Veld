// validation.go — emits src/validation.rs with validator crate integration.
package rustbackend

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter/codegen"
)

// generateValidation returns the bytes for src/validation.rs with validation helpers.
func (e *RustEmitter) generateValidation(a ast.AST) []byte {
	w := codegen.NewWriter("    ")
	w.Writeln(header)
	w.Writeln("use serde::Serialize;")
	w.Writeln("use validator::Validate;")
	w.BlankLine()

	// ValidationError struct
	w.Writeln("/// A single field validation error.")
	w.Writeln("#[derive(Debug, Clone, Serialize)]")
	w.WriteBlock("pub struct ValidationError {")
	w.Writeln("pub field: String,")
	w.Writeln("pub message: String,")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// validate_struct helper
	w.Writeln("/// Validates any struct implementing the Validate trait.")
	w.Writeln("/// Returns Ok(()) or a vector of field-level errors.")
	w.WriteBlock("pub fn validate_struct<T: Validate>(data: &T) -> Result<(), Vec<ValidationError>> {")
	w.WriteBlock("match data.validate() {")
	w.Writeln("Ok(()) => Ok(()),")
	w.WriteBlock("Err(errors) => {")
	w.Writeln("let mut result = Vec::new();")
	w.WriteBlock("for (field, errs) in errors.field_errors() {")
	w.WriteBlock("for e in errs {")
	w.WriteBlock("result.push(ValidationError {")
	w.Writeln("field: field.to_string(),")
	w.Writeln(fmt.Sprintf("message: e.message.as_ref().map(|m| m.to_string()).unwrap_or_else(|| format!(\"validation failed: {:?}\", e.code)),"))
	w.Dedent()
	w.Writeln("});")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.Writeln("Err(result)")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	_ = a

	return w.Bytes()
}

// rustValidateAttr returns #[validate(...)] attributes for a field.
func rustValidateAttr(f ast.Field) string {
	var attrs []string

	if !f.Optional && f.Type == "string" && !f.IsArray && !f.IsMap {
		attrs = append(attrs, "length(min = 1)")
	}
	if strings.Contains(strings.ToLower(f.Name), "email") {
		attrs = append(attrs, "email")
	}
	if strings.HasSuffix(strings.ToLower(f.Name), "url") {
		attrs = append(attrs, "url")
	}

	if len(attrs) == 0 {
		return ""
	}
	return fmt.Sprintf("#[validate(%s)]", strings.Join(attrs, ", "))
}

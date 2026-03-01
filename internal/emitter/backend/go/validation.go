// validation.go — emits internal/models/validation.go with go-playground/validator helpers.
package gobackend

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter/codegen"
)

// generateValidation writes internal/models/validation.go with a shared validator
// instance and Validate() methods on each model.
func (e *GoEmitter) generateValidation(a ast.AST, outDir string) error {
	w := codegen.NewWriter("\t")
	w.Writeln(header)
	w.Writeln("package models")
	w.BlankLine()

	im := codegen.NewImportManager()
	im.Add("fmt", codegen.GroupStdlib)
	im.Add("strings", codegen.GroupStdlib)
	im.Add("github.com/go-playground/validator/v10", codegen.GroupThirdParty)
	w.Write(im.Format("go"))
	w.BlankLine()

	// Shared validator instance
	w.Writeln("// Validate is the shared validator instance for all generated models.")
	w.Writeln("var validate = validator.New()")
	w.BlankLine()

	// ValidationError type
	w.Writeln("// ValidationError holds details of a single field validation failure.")
	w.WriteBlock("type ValidationError struct {")
	w.Writeln(`Field   string ` + "`json:\"field\"`")
	w.Writeln(`Message string ` + "`json:\"message\"`")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// ValidateStruct helper
	w.Writeln("// ValidateStruct validates any struct using the shared validator and returns")
	w.Writeln("// a slice of field-level errors. Returns nil if validation passes.")
	w.WriteBlock("func ValidateStruct(s interface{}) []ValidationError {")
	w.WriteBlock("if err := validate.Struct(s); err != nil {")
	w.Writeln("var errs []ValidationError")
	w.WriteBlock("for _, e := range err.(validator.ValidationErrors) {")
	w.Writeln("errs = append(errs, ValidationError{")
	w.Indent()
	w.Writeln(`Field:   e.Field(),`)
	w.Writeln(`Message: fmt.Sprintf("failed on '%s' validation", e.Tag()),`)
	w.Dedent()
	w.Writeln("})")
	w.Dedent()
	w.Writeln("}")
	w.Writeln("return errs")
	w.Dedent()
	w.Writeln("}")
	w.Writeln("return nil")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// FormatValidationErrors helper
	w.Writeln("// FormatValidationErrors formats validation errors into a human-readable string.")
	w.WriteBlock("func FormatValidationErrors(errs []ValidationError) string {")
	w.Writeln("msgs := make([]string, len(errs))")
	w.WriteBlock("for i, e := range errs {")
	w.Writeln(`msgs[i] = fmt.Sprintf("%s: %s", e.Field, e.Message)`)
	w.Dedent()
	w.Writeln("}")
	w.Writeln(`return strings.Join(msgs, "; ")`)
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	_ = a // validation tags are on the struct fields themselves; this file provides the validator runtime

	dir := filepath.Join(outDir, "internal", "models")
	return os.WriteFile(filepath.Join(dir, "validation.go"), w.Bytes(), 0644)
}

// buildValidateTag returns a `validate:"..."` struct tag for a field.
// Called from types.go buildJSONTag — we extend it to include validation.
func buildValidateTag(f ast.Field) string {
	var rules []string

	if !f.Optional {
		rules = append(rules, "required")
	}

	switch f.Type {
	case "string":
		if !f.IsArray && !f.IsMap {
			rules = append(rules, "min=1")
		}
	case "int", "float":
		// numeric fields are valid as-is
	case "uuid":
		if !f.Optional {
			rules = append(rules, "uuid")
		}
	}

	if f.Name == "email" || strings.Contains(strings.ToLower(f.Name), "email") {
		rules = append(rules, "email")
	}
	if f.Name == "url" || strings.HasSuffix(strings.ToLower(f.Name), "url") {
		rules = append(rules, "url")
	}

	if len(rules) == 0 {
		return ""
	}
	return fmt.Sprintf(`validate:"%s"`, strings.Join(rules, ","))
}

// buildFullTag returns a combined struct tag with json and validate tags.
func buildFullTag(f ast.Field) string {
	jsonPart := f.Name
	if f.Optional {
		jsonPart += ",omitempty"
	}
	validatePart := buildValidateTag(f)
	if validatePart != "" {
		return fmt.Sprintf("`json:\"%s\" %s`", jsonPart, validatePart)
	}
	return fmt.Sprintf("`json:\"%s\"`", jsonPart)
}

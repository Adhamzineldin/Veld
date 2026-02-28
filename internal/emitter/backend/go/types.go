// types.go - Type generation for Go backend
package gobackend

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/veld-dev/veld/internal/ast"
	"github.com/veld-dev/veld/internal/emitter/codegen"
	"github.com/veld-dev/veld/internal/emitter/lang"
)

// generateCommonTypes generates common types file with enums and error types.
func (e *GoEmitter) generateCommonTypes(a ast.AST, outDir string) error {
	w := codegen.NewWriter("\t")

	langStyle := e.adapter.CommentSyntax()
	style := codegen.CommentStyle{
		Single:   langStyle.Single,
		Multi:    langStyle.Multi,
		MultiEnd: langStyle.MultiEnd,
	}

	// Package declaration
	w.Writeln("package models")
	w.BlankLine()

	// Imports
	im := codegen.NewImportManager()
	im.Add("time", codegen.GroupStdlib)
	im.Add("encoding/json", codegen.GroupStdlib)

	fmt.Fprint(w, im.Format("go"))
	w.BlankLine()

	// Generate all enums
	for _, enum := range a.Enums {
		w.WriteComment(fmt.Sprintf("// %s represents the %s enum", enum.Name, strings.ToLower(enum.Name)), style)
		w.WriteBlock(fmt.Sprintf("const ("))

		for i, val := range enum.Values {
			constName := e.adapter.NamingConvention(enum.Name+"_"+val, lang.NamingContextConstant)
			w.Writeln(fmt.Sprintf("%s = \"%s\"", constName, val))
			if i < len(enum.Values)-1 {
				// Separate for readability
			}
		}

		w.Dedent()
		w.Writeln(")")
		w.BlankLine()
	}

	// Generate error response type
	w.WriteComment("// ErrorResponse represents an API error response", style)
	w.WriteBlock("type ErrorResponse struct {")
	w.Writeln("Error   string `json:\"error\"`")
	w.Writeln("Message string `json:\"message,omitempty\"`")
	w.Writeln("Status  int    `json:\"status\"`")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// Generate models
	for _, model := range a.Models {
		if err := e.writeModel(w, model); err != nil {
			return err
		}
		w.BlankLine()
	}

	// Write to file
	filePath := filepath.Join(outDir, "internal", "models", "types.go")
	return os.WriteFile(filePath, w.Bytes(), 0644)
}

// writeModel writes a Go struct for a Veld model.
func (e *GoEmitter) writeModel(w *codegen.Writer, model ast.Model) error {
	langStyle := e.adapter.CommentSyntax()
	style := codegen.CommentStyle{
		Single:   langStyle.Single,
		Multi:    langStyle.Multi,
		MultiEnd: langStyle.MultiEnd,
	}

	// Comment
	w.WriteComment(fmt.Sprintf("// %s represents a %s", model.Name, strings.ToLower(model.Name)), style)

	// Struct definition
	w.WriteBlock(fmt.Sprintf("type %s struct {", model.Name))

	for _, field := range model.Fields {
		goType, _, err := e.adapter.MapType(field.Type)
		if err != nil {
			return fmt.Errorf("failed to map type for field %s.%s: %w", model.Name, field.Name, err)
		}

		// Use field naming conventions
		fieldName := e.adapter.NamingConvention(field.Name, lang.NamingContextExported)
		fieldTag := e.adapter.StructFieldTag(field.Name, goType)

		w.Writeln(fmt.Sprintf("%s %s %s", fieldName, goType, fieldTag))
	}

	w.Dedent()
	w.Writeln("}")

	return nil
}

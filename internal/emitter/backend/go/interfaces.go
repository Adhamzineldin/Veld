// interfaces.go - Service interface generation for Go backend
package gobackend

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/codegen"
	"github.com/Adhamzineldin/Veld/internal/emitter/lang"
)

// generateInterface writes internal/interfaces/{module}.go for a single module.
// The generated interface mirrors the Node emitter's I{Module}Service.ts pattern.
func (e *GoEmitter) generateInterface(a ast.AST, mod ast.Module, outDir string) error {
	w := codegen.NewWriter("\t")
	w.Writeln(header)
	w.Writeln("package interfaces")
	w.BlankLine()

	im := codegen.NewImportManager()
	im.Add("context", codegen.GroupStdlib)

	// Only import models if this module uses any named types.
	if hasNamedTypes(mod) {
		im.Add(goModuleName+"/internal/models", codegen.GroupLocal)
	}
	w.Write(im.Format("go"))
	w.BlankLine()

	moduleName := e.adapter.NamingConvention(mod.Name, lang.NamingContextExported)
	w.Writeln(fmt.Sprintf("// %sService defines the contract for %s business logic.", moduleName, mod.Name))
	w.Writeln("// Implement this interface in your application code.")
	w.WriteBlock(fmt.Sprintf("type %sService interface {", moduleName))

	for _, act := range mod.Actions {
		if act.Method == "WS" {
			// WS actions get lifecycle method signatures instead of a single service call.
			actionName := e.adapter.NamingConvention(act.Name, lang.NamingContextExported)
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			pathParams := emitter.ExtractPathParams(routePath)

			// On{Action}Connect
			var connectParams []string
			connectParams = append(connectParams, "conn interface{}")
			for _, p := range pathParams {
				connectParams = append(connectParams, e.adapter.NamingConvention(p, lang.NamingContextPrivate)+" string")
			}
			if act.Description != "" {
				w.Writeln(fmt.Sprintf("// On%sConnect — called when a client opens the WS connection. %s", actionName, act.Description))
			} else {
				w.Writeln(fmt.Sprintf("// On%sConnect — called when a client opens the WS %s connection.", actionName, routePath))
			}
			w.Writeln(fmt.Sprintf("On%sConnect(%s) error", actionName, strings.Join(connectParams, ", ")))

			// On{Action}Message — only when emit type is set
			if act.Emit != "" {
				emitType := mapGoOutputType(act.Emit)
				w.Writeln(fmt.Sprintf("// On%sMessage — called when a client sends a %s message.", actionName, act.Emit))
				w.Writeln(fmt.Sprintf("On%sMessage(conn interface{}, msg %s) error", actionName, emitType))
			}

			// On{Action}Close — always included
			w.Writeln(fmt.Sprintf("// On%sClose — called when the WS connection is closed.", actionName))
			w.Writeln(fmt.Sprintf("On%sClose(conn interface{}) error", actionName))
			continue
		}

		sig := e.buildServiceMethodSignature(a, mod, act)
		if act.Description != "" || len(act.Errors) > 0 {
			if act.Description != "" {
				w.Writeln(fmt.Sprintf("// %s — %s", act.Name, act.Description))
			} else {
				w.Writeln(fmt.Sprintf("// %s handles the %s %s action.", act.Name, act.Method, act.Path))
			}
			for _, errName := range act.Errors {
				code := emitter.ErrorCode(act.Name, errName)
				w.Writeln(fmt.Sprintf("// Returns %sError: %s — %s", act.Name, code, errName))
			}
		}
		w.Writeln(sig)
	}

	w.Dedent()
	w.Writeln("}")

	dir := filepath.Join(outDir, "internal", "interfaces")
	fileName := strings.ToLower(mod.Name) + ".go"
	return os.WriteFile(filepath.Join(dir, fileName), w.Bytes(), 0644)
}

// buildServiceMethodSignature constructs the Go method signature for an action.
// Examples:
//
//	Login(ctx context.Context, input *models.LoginInput) (*models.User, error)
//	List(ctx context.Context, query *models.UserFilters) ([]models.User, error)
//	Delete(ctx context.Context, id string) error
func (e *GoEmitter) buildServiceMethodSignature(_ ast.AST, mod ast.Module, act ast.Action) string {
	methodName := e.adapter.NamingConvention(act.Name, lang.NamingContextExported)

	var params []string
	params = append(params, "ctx context.Context")

	// Path parameters → individual string args.
	routePath := act.Path
	if mod.Prefix != "" {
		routePath = mod.Prefix + act.Path
	}
	for _, p := range emitter.ExtractPathParams(routePath) {
		paramName := e.adapter.NamingConvention(p, lang.NamingContextPrivate)
		params = append(params, paramName+" string")
	}

	// Body input — pointer to model, or value for primitives/any.
	if act.Input != "" {
		if isPrimitiveOrAny(act.Input) {
			params = append(params, "input "+mapGoOutputType(act.Input))
		} else {
			params = append(params, "input *models."+act.Input)
		}
	}

	// Query params — pointer to model, or value for primitives/any.
	if act.Query != "" {
		if isPrimitiveOrAny(act.Query) {
			params = append(params, "query "+mapGoOutputType(act.Query))
		} else {
			params = append(params, "query *models."+act.Query)
		}
	}

	returnType := buildReturnType(act)
	return fmt.Sprintf("%s(%s) %s", methodName, strings.Join(params, ", "), returnType)
}

// buildReturnType returns the Go return signature for a service method.
func buildReturnType(act ast.Action) string {
	if act.Output == "" {
		return "error"
	}
	goType := mapGoOutputType(act.Output)
	if act.OutputArray {
		return fmt.Sprintf("([]%s, error)", goType)
	}
	// Primitives and any are returned by value; models by pointer.
	if isPrimitiveOrAny(act.Output) {
		return fmt.Sprintf("(%s, error)", goType)
	}
	return fmt.Sprintf("(*%s, error)", goType)
}

// mapGoOutputType maps a Veld output type to a Go type.
func mapGoOutputType(t string) string {
	switch t {
	case "string", "uuid", "date", "datetime":
		return "string"
	case "int":
		return "int"
	case "float":
		return "float64"
	case "bool":
		return "bool"
	case "any":
		return "interface{}"
	case "json":
		return "interface{}"
	default:
		return "models." + t
	}
}

// isPrimitiveOrAny returns true if the type is a Veld built-in scalar or any.
func isPrimitiveOrAny(t string) bool {
	switch t {
	case "string", "int", "float", "bool", "date", "datetime", "uuid", "any", "json":
		return true
	default:
		return false
	}
}

// hasNamedTypes returns true if any action in the module uses model types (not primitives/any).
func hasNamedTypes(mod ast.Module) bool {
	for _, act := range mod.Actions {
		if act.Input != "" && !isPrimitiveOrAny(act.Input) {
			return true
		}
		if act.Output != "" && !isPrimitiveOrAny(act.Output) {
			return true
		}
		if act.Query != "" && !isPrimitiveOrAny(act.Query) {
			return true
		}
	}
	return false
}

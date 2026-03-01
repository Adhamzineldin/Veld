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
		sig := e.buildServiceMethodSignature(a, mod, act)
		if act.Description != "" {
			w.Writeln(fmt.Sprintf("// %s — %s", act.Name, act.Description))
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
func (e *GoEmitter) buildServiceMethodSignature(a ast.AST, mod ast.Module, act ast.Action) string {
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

	// Body input → pointer to model.
	if act.Input != "" {
		params = append(params, "input *models."+act.Input)
	}

	// Query params → pointer to model (caller populates from URL).
	if act.Query != "" {
		params = append(params, "query *models."+act.Query)
	}

	returnType := buildReturnType(act)
	return fmt.Sprintf("%s(%s) %s", methodName, strings.Join(params, ", "), returnType)
}

// buildReturnType returns the Go return signature for a service method.
func buildReturnType(act ast.Action) string {
	if act.Output == "" {
		return "error"
	}
	if act.OutputArray {
		return fmt.Sprintf("([]models.%s, error)", act.Output)
	}
	return fmt.Sprintf("(*models.%s, error)", act.Output)
}

// hasNamedTypes returns true if any action in the module uses model types.
func hasNamedTypes(mod ast.Module) bool {
	for _, act := range mod.Actions {
		if act.Input != "" || act.Output != "" || act.Query != "" {
			return true
		}
	}
	return false
}

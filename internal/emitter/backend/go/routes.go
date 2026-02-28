// routes.go - HTTP routes generation for Go backend
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

// generateRoutesSetup generates the main routes setup file.
func (e *GoEmitter) generateRoutesSetup(a ast.AST, outDir string) error {
	w := codegen.NewWriter("\t")

	langStyle := e.adapter.CommentSyntax()
	style := codegen.CommentStyle{
		Single:   langStyle.Single,
		Multi:    langStyle.Multi,
		MultiEnd: langStyle.MultiEnd,
	}

	w.Writeln("package routes")
	w.BlankLine()

	im := codegen.NewImportManager()
	im.Add("net/http", codegen.GroupStdlib)
	im.Add("github.com/go-chi/chi/v5", codegen.GroupThirdParty)

	w.Writeln(im.Format("go"))
	w.BlankLine()

	// Setup routes function
	w.WriteComment("// SetupRoutes configures all HTTP routes for the API", style)
	w.WriteBlock("func SetupRoutes(r *chi.Mux, services interface{}) {")

	for _, m := range a.Modules {
		setupName := e.adapter.NamingConvention("setup"+m.Name+"Routes", lang.NamingContextPrivate)
		w.Writeln(fmt.Sprintf("%s(r, services)", setupName))
	}

	w.Dedent()
	w.Writeln("}")

	filePath := filepath.Join(outDir, "internal", "routes", "routes.go")
	return os.WriteFile(filePath, w.Bytes(), 0644)
}

// generateModuleRoutes generates route handlers for a module's actions.
// This is called for each module to generate module-specific routes.
func (e *GoEmitter) generateModuleRoutes(m ast.Module, outDir string) error {
	w := codegen.NewWriter("\t")

	langStyle := e.adapter.CommentSyntax()
	style := codegen.CommentStyle{
		Single:   langStyle.Single,
		Multi:    langStyle.Multi,
		MultiEnd: langStyle.MultiEnd,
	}

	w.Writeln("package routes")
	w.BlankLine()

	im := codegen.NewImportManager()
	im.Add("context", codegen.GroupStdlib)
	im.Add("encoding/json", codegen.GroupStdlib)
	im.Add("net/http", codegen.GroupStdlib)
	im.Add("github.com/go-chi/chi/v5", codegen.GroupThirdParty)

	w.Writeln(im.Format("go"))
	w.BlankLine()

	// Setup function for this module
	setupFuncName := e.adapter.NamingConvention("setup"+m.Name+"Routes", lang.NamingContextPrivate)
	w.WriteComment(fmt.Sprintf("// %s sets up routes for the %s module", setupFuncName, m.Name), style)
	w.WriteBlock(fmt.Sprintf("func %s(r *chi.Mux, services interface{}) {", setupFuncName))

	// Add route for each action
	for _, action := range m.Actions {
		handlerName := e.adapter.NamingConvention(action.Name+"Handler", lang.NamingContextPrivate)
		httpMethod := strings.ToUpper(action.Method)
		path := action.Path

		switch httpMethod {
		case "GET":
			w.Writeln(fmt.Sprintf("r.Get(\"%s\", %s(services))", path, handlerName))
		case "POST":
			w.Writeln(fmt.Sprintf("r.Post(\"%s\", %s(services))", path, handlerName))
		case "PUT":
			w.Writeln(fmt.Sprintf("r.Put(\"%s\", %s(services))", path, handlerName))
		case "DELETE":
			w.Writeln(fmt.Sprintf("r.Delete(\"%s\", %s(services))", path, handlerName))
		case "PATCH":
			w.Writeln(fmt.Sprintf("r.Patch(\"%s\", %s(services))", path, handlerName))
		default:
			w.Writeln(fmt.Sprintf("r.MethodFunc(\"%s\", \"%s\", %s(services))", httpMethod, path, handlerName))
		}
	}

	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// Write handler functions for each action
	for _, action := range m.Actions {
		if err := e.writeActionHandler(w, m, action); err != nil {
			return err
		}
		w.BlankLine()
	}

	filePath := filepath.Join(outDir, "internal", "routes", strings.ToLower(m.Name)+".go")
	return os.WriteFile(filePath, w.Bytes(), 0644)
}

// writeActionHandler writes a handler function for an action.
func (e *GoEmitter) writeActionHandler(w *codegen.Writer, m ast.Module, a ast.Action) error {
	style := e.adapter.CommentSyntax()
	handlerName := e.adapter.NamingConvention(a.Name+"Handler", lang.NamingContextPrivate)
	serviceName := e.adapter.NamingConvention(m.Name+"Service", lang.NamingContextExported)
	methodName := e.adapter.NamingConvention(a.Name, lang.NamingContextExported)

	// Handler function signature
	w.WriteComment(fmt.Sprintf("// %s handles %s %s requests", handlerName, strings.ToUpper(a.Method), a.Path), style)
	w.WriteBlock(fmt.Sprintf("func %s(services interface{}) http.HandlerFunc {", handlerName))
	w.WriteBlock("return func(w http.ResponseWriter, r *http.Request) {")

	// Type assertion
	w.Writeln(fmt.Sprintf("svc := services.(%s)", serviceName))
	w.BlankLine()

	// Parse input if needed
	if a.Input != "void" && a.Input != "" {
		w.Writeln(fmt.Sprintf("var req %s", a.Input))
		w.WriteBlock("if err := json.NewDecoder(r.Body).Decode(&req); err != nil {")
		w.Writeln("w.Header().Set(\"Content-Type\", \"application/json\")")
		w.Writeln("w.WriteHeader(http.StatusBadRequest)")
		w.Writeln("json.NewEncoder(w).Encode(map[string]interface{}{\"error\": \"Invalid request\"})")
		w.Writeln("return")
		w.Dedent()
		w.Writeln("}")
		w.BlankLine()
	}

	// Call service method
	hasOutput := a.Output != "void" && a.Output != ""
	if hasOutput {
		w.Writeln(fmt.Sprintf("resp, err := svc.%s(r.Context())", methodName))
		if a.Input != "void" && a.Input != "" {
			// Need to rewrite to pass input
			w.Writeln(fmt.Sprintf("resp, err := svc.%s(r.Context(), &req)", methodName))
		}
	} else {
		w.Writeln(fmt.Sprintf("err := svc.%s(r.Context())", methodName))
		if a.Input != "void" && a.Input != "" {
			w.Writeln(fmt.Sprintf("err := svc.%s(r.Context(), &req)", methodName))
		}
	}

	// Error handling
	w.WriteBlock("if err != nil {")
	w.Writeln("w.Header().Set(\"Content-Type\", \"application/json\")")
	w.Writeln("w.WriteHeader(http.StatusInternalServerError)")
	w.Writeln("json.NewEncoder(w).Encode(map[string]interface{}{\"error\": err.Error()})")
	w.Writeln("return")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// Write response
	w.Writeln("w.Header().Set(\"Content-Type\", \"application/json\")")

	// Determine status code based on HTTP method and output
	if hasOutput {
		if a.Method == "POST" {
			w.Writeln("w.WriteHeader(http.StatusCreated)")
		} else {
			w.Writeln("w.WriteHeader(http.StatusOK)")
		}
		w.Writeln("json.NewEncoder(w).Encode(resp)")
	} else {
		if a.Method == "DELETE" {
			w.Writeln("w.WriteHeader(http.StatusNoContent)")
		} else {
			w.Writeln("w.WriteHeader(http.StatusOK)")
		}
	}

	w.Dedent()
	w.Writeln("})")
	w.BlankLine()

	w.Dedent()
	w.Writeln("}")

	return nil
}

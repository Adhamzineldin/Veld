// middleware.go - Middleware and server entry-point generation for Go backend
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

// generateMiddleware writes internal/middleware/errors.go with panic recovery.
func (e *GoEmitter) generateMiddleware(outDir string) error {
	w := codegen.NewWriter("\t")
	w.Writeln(header)
	w.Writeln("package middleware")
	w.BlankLine()

	im := codegen.NewImportManager()
	im.Add("encoding/json", codegen.GroupStdlib)
	im.Add("log", codegen.GroupStdlib)
	im.Add("net/http", codegen.GroupStdlib)
	w.Write(im.Format("go"))
	w.BlankLine()

	w.Writeln("// RecoverPanic is an HTTP middleware that recovers from panics,")
	w.Writeln("// logs the error, and returns a 500 JSON response to the caller.")
	w.WriteBlock("func RecoverPanic(next http.Handler) http.Handler {")
	w.WriteBlock("return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {")
	w.WriteBlock("defer func() {")
	w.WriteBlock("if rec := recover(); rec != nil {")
	w.Writeln("log.Printf(\"panic recovered: %v\", rec)")
	w.Writeln("w.Header().Set(\"Content-Type\", \"application/json\")")
	w.Writeln("w.WriteHeader(http.StatusInternalServerError)")
	w.Writeln("json.NewEncoder(w).Encode(map[string]interface{}{\"error\": \"internal server error\"}) //nolint:errcheck")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}()")
	w.BlankLine()
	w.Writeln("next.ServeHTTP(w, r)")
	w.Dedent()
	w.Writeln("})")
	w.Dedent()
	w.Writeln("}")

	dir := filepath.Join(outDir, "internal", "middleware")
	return os.WriteFile(filepath.Join(dir, "errors.go"), w.Bytes(), 0644)
}

// generateServer writes server.go (server constructor + Services struct).
func (e *GoEmitter) generateServer(a ast.AST, outDir string) error {
	w := codegen.NewWriter("\t")
	w.Writeln(header)
	w.Writeln("package main")
	w.BlankLine()

	im := codegen.NewImportManager()
	im.Add("net/http", codegen.GroupStdlib)
	im.Add("time", codegen.GroupStdlib)
	im.Add("github.com/go-chi/chi/v5", codegen.GroupThirdParty)
	im.Add(goModuleName+"/internal/interfaces", codegen.GroupLocal)
	im.Add(goModuleName+"/internal/middleware", codegen.GroupLocal)
	im.Add(goModuleName+"/internal/routes", codegen.GroupLocal)
	w.Write(im.Format("go"))
	w.BlankLine()

	// Services struct — one field per module.
	w.Writeln("// Services holds the concrete implementations of all service interfaces.")
	w.Writeln("// Populate these fields with your own implementations before calling NewServer.")
	w.WriteBlock("type Services struct {")
	for _, mod := range a.Modules {
		fieldName := e.adapter.NamingConvention(mod.Name, lang.NamingContextExported)
		ifaceType := "interfaces." + fieldName + "Service"
		w.Writeln(fmt.Sprintf("%s %s", fieldName, ifaceType))
	}
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// NewServer function.
	w.Writeln("// NewServer creates and configures the HTTP server.")
	w.WriteBlock("func NewServer(svc *Services) *http.Server {")
	w.Writeln("r := chi.NewRouter()")
	w.BlankLine()
	w.Writeln("r.Use(middleware.RecoverPanic)")
	w.BlankLine()
	w.Writeln(fmt.Sprintf("routes.SetupRoutes(r, %s)", buildServerSetupArgs(e, a.Modules)))
	w.BlankLine()
	w.WriteBlock("return &http.Server{")
	w.Writeln("Addr:         \":8080\",")
	w.Writeln("Handler:      r,")
	w.Writeln("ReadTimeout:  15 * time.Second,")
	w.Writeln("WriteTimeout: 15 * time.Second,")
	w.Writeln("IdleTimeout:  60 * time.Second,")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")

	return os.WriteFile(filepath.Join(outDir, "server.go"), w.Bytes(), 0644)
}

// generateMain writes main.go (the entry point).
func (e *GoEmitter) generateMain(outDir string) error {
	w := codegen.NewWriter("\t")
	w.Writeln(header)
	w.Writeln("package main")
	w.BlankLine()

	im := codegen.NewImportManager()
	im.Add("log", codegen.GroupStdlib)
	w.Write(im.Format("go"))
	w.BlankLine()

	w.Writeln("func main() {")
	w.Indent()
	w.Writeln("svc := &Services{}")
	w.Writeln("// TODO: initialise your service implementations here")
	w.Writeln("// e.g. svc.Auth = &myapp.AuthServiceImpl{DB: db}")
	w.BlankLine()
	w.Writeln("server := NewServer(svc)")
	w.Writeln("log.Printf(\"listening on %s\", server.Addr)")
	w.Writeln("log.Fatal(server.ListenAndServe())")
	w.Dedent()
	w.Writeln("}")

	return os.WriteFile(filepath.Join(outDir, "main.go"), w.Bytes(), 0644)
}

// generateGoMod writes a minimal go.mod for the generated module.
func (e *GoEmitter) generateGoMod(outDir string) error {
	w := codegen.NewWriter("")
	w.Writeln(fmt.Sprintf("module %s", goModuleName))
	w.BlankLine()
	w.Writeln("go 1.21")
	w.BlankLine()
	w.Writeln("require (")
	w.Writeln("\tgithub.com/go-chi/chi/v5 v5.0.12")
	w.Writeln(")")

	return os.WriteFile(filepath.Join(outDir, "go.mod"), w.Bytes(), 0644)
}

// buildServerSetupArgs builds the arguments to pass to routes.SetupRoutes from server.go.
// e.g. "svc.Auth, svc.Food"
func buildServerSetupArgs(e *GoEmitter, modules []ast.Module) string {
	var parts []string
	for _, mod := range modules {
		fieldName := e.adapter.NamingConvention(mod.Name, lang.NamingContextExported)
		parts = append(parts, "svc."+fieldName)
	}
	return strings.Join(parts, ", ")
}

// generateModuleMiddleware writes internal/interfaces/i_{module}_middleware.go
// with a typed interface per module — one method per middleware name.
func (e *GoEmitter) generateModuleMiddleware(mod ast.Module, outDir string) error {
	allMiddleware := emitter.CollectModuleMiddleware(mod)
	if len(allMiddleware) == 0 {
		return nil
	}

	dir := filepath.Join(outDir, "internal", "middleware")
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}
	moduleLower := strings.ToLower(mod.Name)

	w := codegen.NewWriter("\t")
	w.Writeln(header)
	w.Writeln("package middleware")
	w.BlankLine()
	w.Writeln("import \"net/http\"")
	w.BlankLine()

	interfaceName := "I" + mod.Name + "Middleware"
	w.Writeln(fmt.Sprintf("// %s defines the middleware hooks for the %s module.", interfaceName, mod.Name))
	w.Writeln("// Implement this interface and pass it to the route registrar.")
	w.WriteBlock(fmt.Sprintf("type %s interface {", interfaceName))
	for _, mw := range allMiddleware {
		exportedMw := e.adapter.NamingConvention(mw, lang.NamingContextExported)
		w.Writeln(fmt.Sprintf("// %s wraps the handler with %s middleware.", exportedMw, mw))
		w.Writeln(fmt.Sprintf("%s(next http.Handler) http.Handler", exportedMw))
	}
	w.Dedent()
	w.Writeln("}")

	return os.WriteFile(filepath.Join(dir, "i_"+moduleLower+"_middleware.go"), w.Bytes(), 0644)
}

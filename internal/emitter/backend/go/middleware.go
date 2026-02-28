// middleware.go - Middleware and server generation for Go backend
package gobackend

import (
	"os"
	"path/filepath"

	"github.com/veld-dev/veld/internal/ast"
	"github.com/veld-dev/veld/internal/emitter/codegen"
)

// generateErrorMiddleware generates error handling middleware.
func (e *GoEmitter) generateErrorMiddleware(outDir string) error {
	w := codegen.NewWriter("\t")

	w.Writeln("package middleware")
	w.BlankLine()

	w.Writeln("import (")
	w.Writeln("\t\"encoding/json\"")
	w.Writeln("\t\"log\"")
	w.Writeln("\t\"net/http\"")
	w.Writeln(")")
	w.BlankLine()

	langStyle := e.adapter.CommentSyntax()
	style := codegen.CommentStyle{
		Single:   langStyle.Single,
		Multi:    langStyle.Multi,
		MultiEnd: langStyle.MultiEnd,
	}
	w.WriteComment("// RecoverPanic recovers from panics and returns a 500 error", style)
	w.WriteBlock("func RecoverPanic(next http.Handler) http.Handler {")
	w.WriteBlock("return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {")
	w.WriteBlock("defer func() {")
	w.WriteBlock("if err := recover(); err != nil {")
	w.Writeln("log.Printf(\"Panic recovered: %v\", err)")
	w.Writeln("w.Header().Set(\"Content-Type\", \"application/json\")")
	w.Writeln("w.WriteHeader(http.StatusInternalServerError)")
	w.Writeln("json.NewEncoder(w).Encode(map[string]interface{}{")
	w.Writeln("\t\"error\": \"Internal server error\",")
	w.Writeln("})")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.Writeln("next.ServeHTTP(w, r)")
	w.Dedent()
	w.Writeln("})")
	w.Dedent()
	w.Writeln("}")

	filePath := filepath.Join(outDir, "internal", "middleware", "errors.go")
	return os.WriteFile(filePath, w.Bytes(), 0644)
}

// generateServerSetup generates server.go with server setup.
func (e *GoEmitter) generateServerSetup(a ast.AST, outDir string) error {
	w := codegen.NewWriter("\t")

	w.Writeln("package main")
	w.BlankLine()

	w.Writeln("import (")
	w.Writeln("\t\"net/http\"")
	w.Writeln("\t\"time\"")
	w.Writeln("\t\"github.com/go-chi/chi/v5\"")
	w.Writeln("\t\"yourmodule/internal/middleware\"")
	w.Writeln("\t\"yourmodule/internal/routes\"")
	w.Writeln(")")
	w.BlankLine()

	langStyle := e.adapter.CommentSyntax()
	style := codegen.CommentStyle{
		Single:   langStyle.Single,
		Multi:    langStyle.Multi,
		MultiEnd: langStyle.MultiEnd,
	}
	w.WriteComment("// Services holds all service implementations", style)
	w.WriteBlock("type Services struct {")
	// TODO: Add service fields based on modules
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	w.WriteComment("// NewServer creates and configures the HTTP server", style)
	w.WriteBlock("func NewServer(services *Services) *http.Server {")
	w.Writeln("r := chi.NewRouter()")
	w.BlankLine()
	w.WriteComment("// Add global middleware", style)
	w.Writeln("r.Use(middleware.RecoverPanic)")
	w.BlankLine()
	w.WriteComment("// Setup routes", style)
	w.Writeln("routes.SetupRoutes(r, services)")
	w.BlankLine()
	w.WriteBlock("return &http.Server{")
	w.Writeln("Addr:         \":8080\",")
	w.Writeln("Handler:      r,")
	w.Writeln("ReadTimeout:  15 * time.Second,")
	w.Writeln("WriteTimeout: 15 * time.Second,")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	w.WriteComment("// main starts the HTTP server", style)
	w.WriteBlock("func main() {")
	w.Writeln("services := &Services{}")
	w.Writeln("server := NewServer(services)")
	w.BlankLine()
	w.Writeln("log.Fatal(server.ListenAndServe())")
	w.Dedent()
	w.Writeln("}")

	filePath := filepath.Join(outDir, "server.go")
	return os.WriteFile(filePath, w.Bytes(), 0644)
}

// generateGoMod generates a basic go.mod file.
func (e *GoEmitter) generateGoMod(outDir string) error {
	w := codegen.NewWriter("")

	w.Writeln("module example.com/veld-generated")
	w.BlankLine()
	w.Writeln("go 1.21")
	w.BlankLine()
	w.Writeln("require (")
	w.Writeln("\tgithub.com/go-chi/chi/v5 v5.0.10")
	w.Writeln(")")

	filePath := filepath.Join(outDir, "go.mod")
	return os.WriteFile(filePath, w.Bytes(), 0644)
}

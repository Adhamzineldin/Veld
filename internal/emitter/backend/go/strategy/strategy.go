package strategy

// GoFrameworkStrategy provides framework-specific Go HTTP routing code.
type GoFrameworkStrategy interface {
	// RouterType returns the Go type for the router/mux variable.
	RouterType() string
	// RouterConstructor returns code to create a new router instance.
	RouterConstructor() string
	// RouterParamType returns the type used in function signatures for the router parameter.
	RouterParamType() string
	// RegisterRoute returns a statement to register a handler function.
	RegisterRoute(method, path, handlerFunc string) string
	// ExtractPathParam returns a Go statement to extract a named path param into varName.
	ExtractPathParam(varName, paramName string) string
	// GoImports returns the import paths required by the generated routing code.
	GoImports() []string
	// GoModRequire returns go.mod require entries (e.g. "github.com/go-chi/chi/v5 v5.0.12").
	GoModRequire() []string
	// ServerListenAndServe returns the expression to start the HTTP server.
	ServerListenAndServe(addrExpr, handlerExpr string) string
}

// New returns the GoFrameworkStrategy for the given framework name.
// "" or "plain" → PlainStrategy (net/http, Go 1.22+ ServeMux).
// "chi" → ChiStrategy.
// "gin" → GinStrategy (stub).
func New(framework string) GoFrameworkStrategy {
	switch framework {
	case "chi":
		return &ChiStrategy{}
	case "gin":
		return &GinStrategy{}
	default: // "", "plain"
		return &PlainStrategy{}
	}
}

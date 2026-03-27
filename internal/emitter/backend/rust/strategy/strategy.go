package strategy

// RustFrameworkStrategy provides framework-specific Rust HTTP code generation.
type RustFrameworkStrategy interface {
	// HandlerImports returns use declarations needed in route handler files.
	HandlerImports() []string
	// RouterImports returns use declarations for the router/main setup.
	RouterImports() []string
	// WrapHandler wraps a service call expression into a framework response.
	// method: HTTP method, returnType: Rust return type, serviceCall: the service method call expression
	WrapHandler(method, returnType, serviceCall string) string
	// BuildRouter returns Rust code to build and return the application router.
	// routes is a slice of (method, path, handlerFn) tuples.
	BuildRouter(routes []RouteEntry) string
	// CargoTomlDependencies returns lines to add to [dependencies] in Cargo.toml.
	CargoTomlDependencies() []string
	// MainRsContent returns the content of main.rs (server startup).
	MainRsContent() string
}

// RouteEntry describes one HTTP route for router construction.
type RouteEntry struct {
	Method  string
	Path    string
	Handler string
}

// New returns the RustFrameworkStrategy for the given framework name.
// Empty string or "plain" → PlainStrategy (trait definitions only, no HTTP framework).
// "axum" → AxumStrategy (Axum 0.7 HTTP server).
func New(framework string) RustFrameworkStrategy {
	switch framework {
	case "axum":
		return &AxumStrategy{}
	default: // "", "plain"
		return &PlainStrategy{}
	}
}

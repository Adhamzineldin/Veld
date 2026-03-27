package strategy

// PythonFrameworkStrategy provides framework-specific Python code for HTTP route generation.
type PythonFrameworkStrategy interface {
	// RouterImports returns framework import lines for route files.
	RouterImports(moduleName string) []string
	// RouterSetup returns the router/blueprint variable declaration (empty string = none).
	RouterSetup(moduleName string) string
	// RouteDecorator returns the decorator line for an action.
	RouteDecorator(moduleName, path, method string) string
	// ExtractBody returns Python code to extract the request body into a variable.
	// The variable name used must match what service call expects.
	ExtractBody(inputType string) string
	// ReturnOk returns code to return a 200/success response.
	ReturnOk(expr string) string
	// ReturnCreated returns code to return a 201 Created response.
	ReturnCreated(expr string) string
	// ReturnNoContent returns code to return a 204 No Content response.
	ReturnNoContent() string
	// ReturnError returns code to return an error response.
	ReturnError(statusExpr, msgExpr string) string
	// RegisterRoute returns the line to register a handler function with the app.
	RegisterRoute(moduleName, fnName, flaskPath, methods string) string
	// RequirementsEntries returns pip package lines for requirements.txt.
	RequirementsEntries() []string
}

// New returns the PythonFrameworkStrategy for the given framework name.
// "" or "plain" → PlainStrategy (no HTTP framework).
// "flask" → FlaskStrategy.
// "fastapi" → FastAPIStrategy.
func New(framework string) PythonFrameworkStrategy {
	switch framework {
	case "flask":
		return &FlaskStrategy{}
	case "fastapi":
		return &FastAPIStrategy{}
	default: // "", "plain"
		return &PlainStrategy{}
	}
}

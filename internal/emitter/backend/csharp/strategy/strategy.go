package strategy

// CSharpFrameworkStrategy provides framework-specific C# HTTP code generation.
type CSharpFrameworkStrategy interface {
	// ControllerBaseClass returns the base class for controllers (e.g. "ControllerBase").
	ControllerBaseClass() string
	// ControllerAnnotations returns class-level attributes (e.g. ["[ApiController]"]).
	ControllerAnnotations() []string
	// RouteAnnotation returns the method-level route attribute.
	RouteAnnotation(method, path string) string
	// InputParamAnnotation returns the attribute for a request body parameter.
	InputParamAnnotation() string
	// PathParamAnnotation returns the attribute for a route parameter.
	PathParamAnnotation(name string) string
	// ResponseWrapper returns the return type for action methods.
	ResponseWrapper() string
	// OkResponse returns code for a 200 response.
	OkResponse(expr string) string
	// CreatedResponse returns code for a 201 response.
	CreatedResponse(expr string) string
	// NoContentResponse returns code for a 204 response.
	NoContentResponse() string
	// ErrorResponse returns code for a 500 response.
	ErrorResponse(errVar string) string
	// ControllerUsings returns using directives for controller files.
	ControllerUsings() []string
	// ProjectFileContent returns the .csproj file content.
	ProjectFileContent() string
	// WSActionMethod returns C# code (comment block or real stub) for a WebSocket action.
	// actionName: PascalCase, routePath: full path, emitType/streamType: C# types (may be "").
	WSActionMethod(actionName, routePath, emitType, streamType string) string
}

// New returns the CSharpFrameworkStrategy for the given framework name.
// Empty string or "plain" → PlainStrategy (interfaces only, no ASP.NET dependency).
// "aspnet" or "asp.net" → AspNetStrategy (ASP.NET Core controllers).
func New(framework string) CSharpFrameworkStrategy {
	switch framework {
	case "aspnet", "asp.net":
		return &AspNetStrategy{}
	default: // "", "plain"
		return &PlainStrategy{}
	}
}

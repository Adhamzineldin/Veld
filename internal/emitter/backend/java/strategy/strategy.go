package strategy

import "github.com/Adhamzineldin/Veld/internal/ast"

// FrameworkStrategy provides framework-specific code generation rules for a Java HTTP
// backend. Swap implementations to target Spring Boot, Micronaut, Quarkus, or plain Java.
type FrameworkStrategy interface {
	// ControllerAnnotations returns class-level annotations for a module controller.
	ControllerAnnotations(mod ast.Module) []string
	// RouteAnnotation returns the handler method annotation, e.g. @GetMapping("/path").
	RouteAnnotation(method, path string) string
	// InputParamAnnotation returns the annotation for a request body parameter.
	InputParamAnnotation() string
	// PathParamAnnotation returns the annotation for a named path variable.
	PathParamAnnotation(name string) string
	// QueryParamAnnotation returns the annotation for query parameters.
	QueryParamAnnotation() string
	// QueryParamType returns the fully-qualified Java type for query params.
	QueryParamType() string
	// ResponseWrapper returns the handler return type, e.g. "ResponseEntity<?>".
	ResponseWrapper() string
	// OkResponse returns a return statement for a 200 OK response wrapping expr.
	OkResponse(expr string) string
	// CreatedResponse returns a return statement for a 201 Created response wrapping expr.
	CreatedResponse(expr string) string
	// NoContentResponse returns a return statement for a 204 No Content response.
	NoContentResponse() string
	// ErrorResponse returns a return statement for a 500 error response using errVar.
	ErrorResponse(errVar string) string
	// ControllerImports returns framework imports required in controller files.
	ControllerImports() []string
	// MiddlewareImports returns framework imports required in middleware files.
	MiddlewareImports() []string
	// ValidationImports returns framework imports required in validation files.
	ValidationImports() []string
	// GlobalExceptionHandlerSource returns the complete source of GlobalExceptionHandler.java.
	// Return an empty string to skip generating the file.
	GlobalExceptionHandlerSource(ctrlPkg, modelsPkg string) string
	// BuildFile returns the build descriptor filename and its full contents.
	BuildFile() (name, content string)
	// WSControllerMethod returns the Java controller method stub(s) for a WebSocket action.
	// actionName: PascalCase action name, routePath: full route path,
	// emitType: client→server Java type (may be "String"), streamType: server→client type (may be "").
	WSControllerMethod(actionName, routePath, emitType, streamType string) string
}

// New returns the FrameworkStrategy for the given name.
// Empty string or "plain" → PlainStrategy (no framework dependency).
// "spring" or "spring-boot" → SpringStrategy (Spring Boot 3.x).
func New(framework string) FrameworkStrategy {
	switch framework {
	case "spring", "spring-boot":
		return &SpringStrategy{}
	default: // "", "plain"
		return &PlainStrategy{}
	}
}

package strategy

// PHPFrameworkStrategy provides framework-specific PHP HTTP code generation.
type PHPFrameworkStrategy interface {
	// ControllerAnnotations returns doc-comment annotations above controller classes.
	ControllerAnnotations() []string
	// ControllerBaseClass returns the base class controllers extend (e.g. "Controller").
	ControllerBaseClass() string
	// ControllerUses returns use statements for controller files.
	ControllerUses() []string
	// RequestType returns the PHP type for the HTTP request parameter.
	RequestType() string
	// ReturnOk returns PHP code to return a successful 200 response.
	ReturnOk(expr string) string
	// ReturnCreated returns PHP code for a 201 response.
	ReturnCreated(expr string) string
	// ReturnNoContent returns PHP code for a 204 response.
	ReturnNoContent() string
	// ReturnError returns PHP code for an error response.
	ReturnError(statusCode int, msgExpr string) string
	// ComposerRequire returns composer.json require entries (package → version).
	ComposerRequire() map[string]string
	// WSActionMethod returns PHP code for a WebSocket action in the controller.
	// actionName: camelCase, routePath: full path, emitType: PHP type hint (may be ""),
	// streamType: server→client type (may be "").
	WSActionMethod(actionName, routePath, emitType, streamType string) string
}

// New returns the PHPFrameworkStrategy for the given framework name.
// Empty string or "plain" → PlainStrategy (interfaces only, no framework dependency).
// "laravel" → LaravelStrategy (Laravel 10.x controllers + routes).
func New(framework string) PHPFrameworkStrategy {
	switch framework {
	case "laravel":
		return &LaravelStrategy{}
	default: // "", "plain"
		return &PlainStrategy{}
	}
}

package strategy

import "fmt"

// LaravelStrategy generates Laravel 10.x controller code.
// It produces classes extending Controller, uses Illuminate\Http\Request,
// and returns response()->json() / response()->noContent() helpers.
type LaravelStrategy struct{}

func (s *LaravelStrategy) ControllerAnnotations() []string { return nil }

func (s *LaravelStrategy) ControllerBaseClass() string { return "Controller" }

func (s *LaravelStrategy) ControllerUses() []string {
	return []string{
		"use Illuminate\\Http\\Request;",
		"use Illuminate\\Http\\JsonResponse;",
	}
}

func (s *LaravelStrategy) RequestType() string { return "Request" }

func (s *LaravelStrategy) ReturnOk(expr string) string {
	return fmt.Sprintf("return response()->json(%s);", expr)
}

func (s *LaravelStrategy) ReturnCreated(expr string) string {
	return fmt.Sprintf("return response()->json(%s, 201);", expr)
}

func (s *LaravelStrategy) ReturnNoContent() string {
	return "return response()->noContent();"
}

func (s *LaravelStrategy) ReturnError(statusCode int, msgExpr string) string {
	return fmt.Sprintf("return response()->json(['error' => %s], %d);", msgExpr, statusCode)
}

func (s *LaravelStrategy) ComposerRequire() map[string]string {
	return map[string]string{
		"php":                "^8.1",
		"laravel/framework": "^10.0",
	}
}

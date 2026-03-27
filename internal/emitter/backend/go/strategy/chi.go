package strategy

import (
	"fmt"
	"strings"
)

// ChiStrategy generates Chi router-based route handlers.
// It produces the same output that the Go emitter previously generated directly.
// Route registration: r.Get("/path", handler).
// Path parameter extraction: chi.URLParam(r, "name").
type ChiStrategy struct{}

func (s *ChiStrategy) RouterType() string { return "chi.Router" }

func (s *ChiStrategy) RouterConstructor() string { return "chi.NewRouter()" }

func (s *ChiStrategy) RouterParamType() string { return "chi.Router" }

func (s *ChiStrategy) RegisterRoute(method, path, handlerFunc string) string {
	chiMethod := chiMethodName(method)
	return fmt.Sprintf(`r.%s(%q, %s)`, chiMethod, path, handlerFunc)
}

func (s *ChiStrategy) ExtractPathParam(varName, paramName string) string {
	return fmt.Sprintf(`%s := chi.URLParam(r, %q)`, varName, paramName)
}

func (s *ChiStrategy) GoImports() []string {
	return []string{
		"net/http",
		"github.com/go-chi/chi/v5",
	}
}

func (s *ChiStrategy) GoModRequire() []string {
	return []string{"github.com/go-chi/chi/v5 v5.0.12"}
}

func (s *ChiStrategy) ServerListenAndServe(addrExpr, handlerExpr string) string {
	return fmt.Sprintf("http.ListenAndServe(%s, %s)", addrExpr, handlerExpr)
}

// chiMethodName maps an HTTP method string to the Chi router method name.
func chiMethodName(method string) string {
	switch strings.ToUpper(method) {
	case "GET":
		return "Get"
	case "POST":
		return "Post"
	case "PUT":
		return "Put"
	case "DELETE":
		return "Delete"
	case "PATCH":
		return "Patch"
	default:
		return "MethodFunc"
	}
}

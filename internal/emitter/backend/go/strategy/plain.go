package strategy

import (
	"fmt"
	"strings"
)

// PlainStrategy generates net/http routing using the Go 1.22+ ServeMux pattern.
// Route registration uses mux.HandleFunc("METHOD /path", handler).
// Path parameters are extracted using r.PathValue("name").
// No external dependencies are required.
type PlainStrategy struct{}

func (s *PlainStrategy) RouterType() string { return "*http.ServeMux" }

func (s *PlainStrategy) RouterConstructor() string { return "http.NewServeMux()" }

func (s *PlainStrategy) RouterParamType() string { return "*http.ServeMux" }

func (s *PlainStrategy) RegisterRoute(method, path, handlerFunc string) string {
	return fmt.Sprintf(`mux.HandleFunc("%s %s", %s)`, strings.ToUpper(method), path, handlerFunc)
}

func (s *PlainStrategy) ExtractPathParam(varName, paramName string) string {
	return fmt.Sprintf(`%s := r.PathValue("%s")`, varName, paramName)
}

func (s *PlainStrategy) GoImports() []string {
	return []string{"net/http"}
}

func (s *PlainStrategy) GoModRequire() []string { return nil }

func (s *PlainStrategy) ServerListenAndServe(addrExpr, handlerExpr string) string {
	return fmt.Sprintf("http.ListenAndServe(%s, %s)", addrExpr, handlerExpr)
}

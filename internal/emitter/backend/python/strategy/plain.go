package strategy

import (
	"fmt"
	"strings"
)

// PlainStrategy generates pure typed Python with no HTTP framework dependency.
// Route handlers are plain functions annotated with a comment indicating the HTTP method and path.
type PlainStrategy struct{}

func (s *PlainStrategy) RouterImports(moduleName string) []string { return nil }

func (s *PlainStrategy) RouterSetup(moduleName string) string { return "" }

func (s *PlainStrategy) RouteDecorator(moduleName, path, method string) string {
	return fmt.Sprintf("# %s %s", strings.ToUpper(method), path)
}

func (s *PlainStrategy) ExtractBody(inputType string) string {
	return fmt.Sprintf("body = data  # %s", inputType)
}

func (s *PlainStrategy) ReturnOk(expr string) string {
	return fmt.Sprintf("return %s", expr)
}

func (s *PlainStrategy) ReturnCreated(expr string) string {
	return fmt.Sprintf("return %s  # 201", expr)
}

func (s *PlainStrategy) ReturnNoContent() string { return "return None  # 204" }

func (s *PlainStrategy) ReturnError(statusExpr, msgExpr string) string {
	return fmt.Sprintf("raise RuntimeError(%s)", msgExpr)
}

func (s *PlainStrategy) RegisterRoute(moduleName, fnName, flaskPath, methods string) string {
	return fmt.Sprintf("# register: %s %s -> %s", methods, flaskPath, fnName)
}

func (s *PlainStrategy) RequirementsEntries() []string { return nil }

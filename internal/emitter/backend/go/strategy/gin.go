package strategy

import (
	"fmt"
	"strings"
)

// GinStrategy generates Gin framework route handlers.
// This is a stub implementation — Gin uses c.Param() for path params
// and r.Run() to start the server.
type GinStrategy struct{}

func (s *GinStrategy) RouterType() string { return "*gin.Engine" }

func (s *GinStrategy) RouterConstructor() string { return "gin.Default()" }

func (s *GinStrategy) RouterParamType() string { return "*gin.RouterGroup" }

func (s *GinStrategy) RegisterRoute(method, path, handlerFunc string) string {
	ginPath := convertChiPathToGin(path)
	ginMethod := strings.Title(strings.ToLower(method)) //nolint:staticcheck
	return fmt.Sprintf(`r.%s("%s", %s)`, ginMethod, ginPath, handlerFunc)
}

func (s *GinStrategy) ExtractPathParam(varName, paramName string) string {
	return fmt.Sprintf(`%s := c.Param("%s")`, varName, paramName)
}

func (s *GinStrategy) GoImports() []string {
	return []string{"github.com/gin-gonic/gin"}
}

func (s *GinStrategy) GoModRequire() []string {
	return []string{"github.com/gin-gonic/gin v1.9.1"}
}

func (s *GinStrategy) ServerListenAndServe(addrExpr, handlerExpr string) string {
	return fmt.Sprintf("r.Run(%s)", addrExpr)
}

// convertChiPathToGin converts Chi-style {param} path segments to Gin :param style.
// Chi uses {id}, Gin uses :id. Veld uses :id natively, so this is a no-op for now.
func convertChiPathToGin(path string) string {
	return path
}

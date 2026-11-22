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

func (s *GinStrategy) WSHandlerCode(actionName, routePath, streamType, emitType string, pathParams []string, svcArg, svcType string) string {
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("\t// WebSocket: WS %s\n", routePath))
	sb.WriteString(fmt.Sprintf("\tr.GET(%q, func(c *gin.Context) {\n", routePath))
	sb.WriteString("\t\t// Upgrade to WebSocket using gorilla/websocket or nhooyr.io/websocket.\n")
	for _, p := range pathParams {
		sb.WriteString(fmt.Sprintf("\t\t%s := c.Param(%q)\n", p, p))
	}
	if len(pathParams) > 0 {
		paramList := ""
		for _, p := range pathParams {
			paramList += ", " + p
		}
		sb.WriteString(fmt.Sprintf("\t\t// TODO: implement %s.On%sConnect(conn%s)\n", svcArg, actionName, paramList))
	} else {
		sb.WriteString(fmt.Sprintf("\t\t// TODO: implement %s.On%sConnect(conn)\n", svcArg, actionName))
	}
	if emitType != "" {
		sb.WriteString(fmt.Sprintf("\t\t// TODO: on message: %s.On%sMessage(conn, msg %s)\n", svcArg, actionName, emitType))
	}
	if streamType != "" {
		sb.WriteString(fmt.Sprintf("\t\t// TODO: broadcast helper sends %s to client\n", streamType))
	}
	sb.WriteString("\t})\n")
	return sb.String()
}

// convertChiPathToGin converts Chi-style {param} path segments to Gin :param style.
// Chi uses {id}, Gin uses :id. Veld uses :id natively, so this is a no-op for now.
func convertChiPathToGin(path string) string {
	return path
}

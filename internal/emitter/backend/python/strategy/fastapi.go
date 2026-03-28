package strategy

import (
	"fmt"
	"strings"
)

// FastAPIStrategy generates FastAPI router code with automatic Pydantic validation.
// This is a stub — FastAPI auto-injects request bodies via function parameters,
// so body extraction and response wrapping differ significantly from Flask.
type FastAPIStrategy struct{}

func (s *FastAPIStrategy) RouterImports(moduleName string) []string {
	return []string{
		"from fastapi import APIRouter",
		"from fastapi import HTTPException",
	}
}

func (s *FastAPIStrategy) RouterSetup(moduleName string) string {
	return fmt.Sprintf("router = APIRouter(prefix='/%s')", strings.ToLower(moduleName))
}

func (s *FastAPIStrategy) RouteDecorator(moduleName, path, method string) string {
	return fmt.Sprintf("@router.%s('%s')", strings.ToLower(method), path)
}

func (s *FastAPIStrategy) ExtractBody(inputType string) string {
	// FastAPI auto-injects via function signature; body is already available.
	return ""
}

func (s *FastAPIStrategy) ReturnOk(expr string) string {
	return fmt.Sprintf("return %s", expr)
}

func (s *FastAPIStrategy) ReturnCreated(expr string) string {
	// FastAPI handles status codes via response_model_status_code on the decorator.
	return fmt.Sprintf("return %s  # status_code=201 set on decorator", expr)
}

func (s *FastAPIStrategy) ReturnNoContent() string {
	return "return None"
}

func (s *FastAPIStrategy) ReturnError(statusExpr, msgExpr string) string {
	return fmt.Sprintf("raise HTTPException(status_code=%s, detail=%s)", statusExpr, msgExpr)
}

func (s *FastAPIStrategy) RegisterRoute(moduleName, fnName, flaskPath, methods string) string {
	// FastAPI uses decorator-based registration; app.include_router is done by the user.
	return fmt.Sprintf("# router.include_router(router, prefix='/%s')", strings.ToLower(moduleName))
}

func (s *FastAPIStrategy) RequirementsEntries() []string {
	return []string{
		"fastapi>=0.104.0",
		"uvicorn[standard]>=0.24.0",
	}
}

func (s *FastAPIStrategy) WSHandlerCode(actionName, routePath, streamType, emitType string, pathParams []string) string {
	// FastAPI native WebSocket endpoint.
	var sb strings.Builder
	// Build function parameter list: websocket first, then path params.
	sigParts := []string{"websocket: WebSocket"}
	for _, p := range pathParams {
		sigParts = append(sigParts, p+": str")
	}
	sigStr := strings.Join(sigParts, ", ")

	sb.WriteString(fmt.Sprintf("\n    @router.websocket('%s')\n", routePath))
	sb.WriteString(fmt.Sprintf("    async def %s_ws(%s):\n", actionName, sigStr))
	sb.WriteString("        await websocket.accept()\n")
	if len(pathParams) > 0 {
		sb.WriteString(fmt.Sprintf("        # implement: await service.on_%s_connect(websocket, %s)\n", actionName, strings.Join(pathParams, ", ")))
	} else {
		sb.WriteString(fmt.Sprintf("        # implement: await service.on_%s_connect(websocket)\n", actionName))
	}
	sb.WriteString("        try:\n")
	sb.WriteString("            while True:\n")
	if emitType != "" {
		sb.WriteString("                data = await websocket.receive_json()\n")
		sb.WriteString(fmt.Sprintf("                # data is expected to conform to %s\n", emitType))
		sb.WriteString(fmt.Sprintf("                # implement: await service.on_%s_message(websocket, data)\n", actionName))
	} else {
		sb.WriteString("                await websocket.receive_text()  # keep connection alive\n")
	}
	sb.WriteString("        except WebSocketDisconnect:\n")
	sb.WriteString(fmt.Sprintf("            # implement: await service.on_%s_close(websocket)\n", actionName))
	sb.WriteString("            pass\n")
	return sb.String()
}

// routes.go - HTTP route handler generation for Go backend
package gobackend

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	gostrategy "github.com/Adhamzineldin/Veld/internal/emitter/backend/go/strategy"
	"github.com/Adhamzineldin/Veld/internal/emitter/codegen"
	"github.com/Adhamzineldin/Veld/internal/emitter/lang"
)

// generateRoutesSetup writes internal/routes/routes.go with:
//   - SetupRoutes(r <RouterType>, ...) dispatcher
//   - Package-level writeJSON and readJSON helpers
func (e *GoEmitter) generateRoutesSetup(a ast.AST, outDir string, strat gostrategy.GoFrameworkStrategy) error {
	w := codegen.NewWriter("\t")
	w.Writeln(header)
	w.Writeln("package routes")
	w.BlankLine()

	im := codegen.NewImportManager()
	im.Add("encoding/json", codegen.GroupStdlib)
	im.Add("net/http", codegen.GroupStdlib)
	for _, imp := range strat.GoImports() {
		if imp != "net/http" {
			im.Add(imp, codegen.GroupThirdParty)
		}
	}

	for _, mod := range a.Modules {
		im.Add(goModuleName+"/internal/interfaces", codegen.GroupLocal)
		_ = mod // imports are per-module; the interfaces package covers all
		break
	}
	w.Write(im.Format("go"))
	w.BlankLine()

	// SetupRoutes — one parameter per module service.
	routerParamType := strat.RouterParamType()
	w.Writeln(fmt.Sprintf("// SetupRoutes registers all module routes with the given %s.", routerParamType))
	w.WriteBlock(fmt.Sprintf("func SetupRoutes(r %s, %s) {", routerParamType, buildServiceParams(e, a.Modules)))
	for _, mod := range a.Modules {
		setupName := e.adapter.NamingConvention("setup"+mod.Name+"Routes", lang.NamingContextPrivate)
		svcArg := e.adapter.NamingConvention(mod.Name+"Svc", lang.NamingContextPrivate)
		w.Writeln(fmt.Sprintf("%s(r, %s)", setupName, svcArg))
	}
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// writeJSON helper — avoids repeating header/WriteHeader/Encode in every handler.
	w.Writeln("// writeJSON writes v as JSON with the given HTTP status code.")
	w.WriteBlock("func writeJSON(w http.ResponseWriter, status int, v interface{}) {")
	w.Writeln("w.Header().Set(\"Content-Type\", \"application/json\")")
	w.Writeln("w.WriteHeader(status)")
	w.WriteBlock("if v != nil {")
	w.Writeln("json.NewEncoder(w).Encode(v) //nolint:errcheck")
	w.Dedent()
	w.Writeln("}")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// readJSON helper — decodes a JSON request body.
	w.Writeln("// readJSON decodes the request body into v.")
	w.WriteBlock("func readJSON(r *http.Request, v interface{}) error {")
	w.Writeln("return json.NewDecoder(r.Body).Decode(v)")
	w.Dedent()
	w.Writeln("}")

	dir := filepath.Join(outDir, "internal", "routes")
	return os.WriteFile(filepath.Join(dir, "routes.go"), w.Bytes(), 0644)
}

// buildServiceParams returns the parameter list for SetupRoutes.
// e.g. "authSvc interfaces.AuthService, foodSvc interfaces.FoodService"
func buildServiceParams(e *GoEmitter, modules []ast.Module) string {
	var parts []string
	for _, mod := range modules {
		argName := e.adapter.NamingConvention(mod.Name+"Svc", lang.NamingContextPrivate)
		ifaceType := "interfaces." + e.adapter.NamingConvention(mod.Name, lang.NamingContextExported) + "Service"
		parts = append(parts, argName+" "+ifaceType)
	}
	return strings.Join(parts, ", ")
}

// generateModuleRoutes writes internal/routes/{module}.go with:
//   - setup{Module}Routes registration function
//   - One handler closure per action
func (e *GoEmitter) generateModuleRoutes(a ast.AST, mod ast.Module, outDir string, strat gostrategy.GoFrameworkStrategy) error {
	w := codegen.NewWriter("\t")
	w.Writeln(header)
	w.Writeln("package routes")
	w.BlankLine()

	// Determine which imports are needed.
	hasInput := moduleHasInput(mod)

	im := codegen.NewImportManager()
	for _, imp := range strat.GoImports() {
		if imp == "net/http" {
			im.Add(imp, codegen.GroupStdlib)
		} else {
			im.Add(imp, codegen.GroupThirdParty)
		}
	}
	im.Add(goModuleName+"/internal/interfaces", codegen.GroupLocal)
	if hasInput {
		im.Add(goModuleName+"/internal/models", codegen.GroupLocal)
	}

	w.Write(im.Format("go"))
	w.BlankLine()

	// setup{Module}Routes — registers each action's handler.
	setupFuncName := e.adapter.NamingConvention("setup"+mod.Name+"Routes", lang.NamingContextPrivate)
	svcType := "interfaces." + e.adapter.NamingConvention(mod.Name, lang.NamingContextExported) + "Service"
	svcArg := e.adapter.NamingConvention(mod.Name+"Svc", lang.NamingContextPrivate)
	routerParamType := strat.RouterParamType()

	w.Writeln(fmt.Sprintf("// %s registers the %s module routes.", setupFuncName, mod.Name))
	w.WriteBlock(fmt.Sprintf("func %s(r %s, %s %s) {", setupFuncName, routerParamType, svcArg, svcType))

	for _, act := range mod.Actions {
		routePath := fullPath(mod, act)
		if act.Method == "WS" {
			actionName := e.adapter.NamingConvention(act.Name, lang.NamingContextExported)
			pathParams := emitter.ExtractPathParams(routePath)
			wsCode := strat.WSHandlerCode(actionName, routePath, act.Stream, act.Emit, pathParams, svcArg, svcType)
			for _, line := range strings.Split(strings.TrimRight(wsCode, "\n"), "\n") {
				// Strip leading tab since WriteBlock already indented once.
				w.Writeln(strings.TrimPrefix(line, "\t"))
			}
		} else {
			chiPath := emitter.ToChiPath(routePath)
			handlerName := e.adapter.NamingConvention(act.Name+"Handler", lang.NamingContextPrivate)
			w.Writeln(strat.RegisterRoute(act.Method, chiPath, handlerName+"("+svcArg+")"))
		}
	}

	w.Dedent()
	w.Writeln("}")

	// Handler function per action (skip WS actions — they are inline stubs).
	for _, act := range mod.Actions {
		if act.Method == "WS" {
			continue
		}
		w.BlankLine()
		if err := e.writeHandler(w, mod, act, svcType, strat); err != nil {
			return err
		}
	}

	dir := filepath.Join(outDir, "internal", "routes")
	fileName := strings.ToLower(mod.Name) + ".go"
	return os.WriteFile(filepath.Join(dir, fileName), w.Bytes(), 0644)
}

// writeHandler generates a single http.HandlerFunc closure for an action.
func (e *GoEmitter) writeHandler(w *codegen.Writer, mod ast.Module, act ast.Action, svcType string, strat gostrategy.GoFrameworkStrategy) error {
	handlerName := e.adapter.NamingConvention(act.Name+"Handler", lang.NamingContextPrivate)
	methodName := e.adapter.NamingConvention(act.Name, lang.NamingContextExported)
	routePath := fullPath(mod, act)
	pathParams := emitter.ExtractPathParams(routePath)

	hasInput := act.Input != ""
	hasOutput := act.Output != ""

	if act.Description != "" {
		w.Writeln(fmt.Sprintf("// %s — %s", handlerName, act.Description))
	} else {
		w.Writeln(fmt.Sprintf("// %s handles %s %s requests.", handlerName, strings.ToUpper(act.Method), routePath))
	}

	// Outer function: returns the handler closure bound to the service.
	w.WriteBlock(fmt.Sprintf("func %s(svc %s) http.HandlerFunc {", handlerName, svcType))
	w.WriteBlock("return func(w http.ResponseWriter, r *http.Request) {")

	// 1. Extract path parameters using the strategy (Chi vs plain net/http differ here).
	for _, p := range pathParams {
		varName := e.adapter.NamingConvention(p, lang.NamingContextPrivate)
		w.Writeln(strat.ExtractPathParam(varName, p))
	}
	if len(pathParams) > 0 {
		w.BlankLine()
	}

	// 2. Decode request body (if the action has input).
	if hasInput {
		w.Writeln(fmt.Sprintf("var req models.%s", act.Input))
		w.WriteBlock("if err := readJSON(r, &req); err != nil {")
		w.Writeln("writeJSON(w, http.StatusBadRequest, map[string]interface{}{\"error\": \"invalid request body\"})")
		w.Writeln("return")
		w.Dedent()
		w.Writeln("}")
		w.BlankLine()
	}

	// 3. Build service call arguments.
	callArgs := buildCallArgs(e, act, pathParams)

	// 4. Invoke service and handle error.
	if hasOutput {
		w.Writeln(fmt.Sprintf("resp, err := svc.%s(%s)", methodName, strings.Join(callArgs, ", ")))
	} else {
		w.Writeln(fmt.Sprintf("err := svc.%s(%s)", methodName, strings.Join(callArgs, ", ")))
	}

	w.WriteBlock("if err != nil {")
	w.Writeln("writeJSON(w, http.StatusInternalServerError, map[string]interface{}{\"error\": err.Error()})")
	w.Writeln("return")
	w.Dedent()
	w.Writeln("}")
	w.BlankLine()

	// 5. Write success response.
	if hasOutput {
		statusCode := successStatus(act)
		w.Writeln(fmt.Sprintf("writeJSON(w, %s, resp)", statusCode))
	} else if strings.ToUpper(act.Method) == "DELETE" {
		w.Writeln("w.WriteHeader(http.StatusNoContent)")
	} else {
		w.Writeln("writeJSON(w, http.StatusOK, nil)")
	}

	// Close inner function (return func body).
	w.Dedent()
	w.Writeln("}")
	// Close outer function.
	w.Dedent()
	w.Writeln("}")
	return nil
}

// ── small helpers ─────────────────────────────────────────────────────────────

// fullPath joins the module prefix and action path.
func fullPath(mod ast.Module, act ast.Action) string {
	if mod.Prefix != "" {
		return mod.Prefix + act.Path
	}
	return act.Path
}

// successStatus returns the HTTP status constant for a successful response.
func successStatus(act ast.Action) string {
	if strings.ToUpper(act.Method) == "POST" {
		return "http.StatusCreated"
	}
	return "http.StatusOK"
}

// buildCallArgs constructs the argument list for the service method call.
func buildCallArgs(e *GoEmitter, act ast.Action, pathParams []string) []string {
	var args []string
	args = append(args, "r.Context()")

	for _, p := range pathParams {
		args = append(args, e.adapter.NamingConvention(p, lang.NamingContextPrivate))
	}
	if act.Input != "" {
		args = append(args, "&req")
	}
	if act.Query != "" {
		// Query params are not automatically decoded; pass nil so the interface compiles.
		// Users should decode query params from r.URL.Query() in their service or wrap the handler.
		args = append(args, "nil")
	}
	if act.Headers != "" {
		// Headers are not automatically decoded; pass nil so the interface compiles.
		// Users should extract headers from r.Header in their service or wrap the handler.
		args = append(args, "nil")
	}
	return args
}

// moduleHasPathParams returns true if any action in the module has path parameters.
func moduleHasPathParams(mod ast.Module) bool {
	for _, act := range mod.Actions {
		if len(emitter.ExtractPathParams(fullPath(mod, act))) > 0 {
			return true
		}
	}
	return false
}

// moduleHasInput returns true if any action in the module has a request body.
func moduleHasInput(mod ast.Module) bool {
	for _, act := range mod.Actions {
		if act.Input != "" {
			return true
		}
	}
	return false
}

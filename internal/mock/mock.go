package mock

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// route holds a parsed route pattern and its handler.
type route struct {
	method  string
	parts   []string // split path segments; ":param" segments are wildcards
	handler http.HandlerFunc
}

// Run starts a mock HTTP server on the given port, serving fake responses
// derived from the .veld contract AST.
func Run(a ast.AST, port int) error {
	routes := buildRoutes(a)

	mux := http.NewServeMux()
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		// CORS headers for frontend development.
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}

		for _, rt := range routes {
			if matchRoute(rt, r.Method, r.URL.Path) {
				rt.handler(w, r)
				return
			}
		}

		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusNotFound)
		_ = json.NewEncoder(w).Encode(map[string]string{
			"error": "no matching route",
			"path":  r.URL.Path,
		})
	})

	printBanner(a, routes, port)

	addr := fmt.Sprintf(":%d", port)
	return http.ListenAndServe(addr, mux)
}

// buildRoutes creates route handlers for every action in the AST.
func buildRoutes(a ast.AST) []route {
	var routes []route
	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			if act.Method == "WS" {
				continue // skip WebSocket actions
			}
			fullPath := mod.Prefix + act.Path
			parts := splitPath(fullPath)
			handler := makeHandler(a, act)
			routes = append(routes, route{
				method:  act.Method,
				parts:   parts,
				handler: handler,
			})
		}
	}
	return routes
}

// makeHandler returns an http.HandlerFunc that serves a fake JSON response.
func makeHandler(a ast.AST, act ast.Action) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")

		// Determine status code.
		status := http.StatusOK
		if act.Method == "POST" {
			status = http.StatusCreated
		}
		if act.Method == "DELETE" && act.Output == "" {
			w.WriteHeader(http.StatusNoContent)
			return
		}

		// No output model → empty 200/201.
		if act.Output == "" {
			w.WriteHeader(status)
			return
		}

		// Build fake response body.
		body := exampleValue(a, act.Output, 0)
		if act.OutputArray {
			body = []interface{}{body}
		}

		w.WriteHeader(status)
		enc := json.NewEncoder(w)
		enc.SetIndent("", "  ")
		_ = enc.Encode(body)
	}
}

// exampleValue generates a fake value for the given type name.
// depth guards against infinite recursion on circular model references.
func exampleValue(a ast.AST, typeName string, depth int) interface{} {
	if depth > 3 {
		return nil
	}

	// Check if it's an enum.
	for _, e := range a.Enums {
		if e.Name == typeName {
			if len(e.Values) > 0 {
				return e.Values[0]
			}
			return "unknown"
		}
	}

	// Check if it's a model.
	for _, m := range a.Models {
		if m.Name == typeName {
			return exampleModel(a, m, depth)
		}
	}

	// Primitive type.
	return examplePrimitive(typeName, "value")
}

// exampleModel builds a fake JSON object for a model, including inherited fields.
func exampleModel(a ast.AST, m ast.Model, depth int) map[string]interface{} {
	obj := make(map[string]interface{})

	// Include parent fields first (inheritance).
	if m.Extends != "" {
		for _, parent := range a.Models {
			if parent.Name == m.Extends {
				parentObj := exampleModel(a, parent, depth+1)
				for k, v := range parentObj {
					obj[k] = v
				}
				break
			}
		}
	}

	// Own fields.
	for _, f := range m.Fields {
		val := exampleFieldValue(a, f, depth+1)
		obj[f.Name] = val
	}
	return obj
}

// exampleFieldValue generates a fake value for a single field.
func exampleFieldValue(a ast.AST, f ast.Field, depth int) interface{} {
	if f.IsMap {
		valType := f.MapValueType
		if valType == "" {
			valType = "string"
		}
		inner := exampleValue(a, valType, depth)
		return map[string]interface{}{"key": inner}
	}

	var val interface{}
	// Check if the type references a model or enum.
	val = exampleValue(a, f.Type, depth)

	if f.IsArray {
		return []interface{}{val}
	}
	return val
}

// examplePrimitive returns a fake value for a primitive Veld type.
func examplePrimitive(typeName string, fieldName string) interface{} {
	switch typeName {
	case "string":
		return "example-" + fieldName
	case "uuid":
		return "550e8400-e29b-41d4-a716-446655440000"
	case "int":
		return 42
	case "float":
		return 9.99
	case "bool":
		return true
	case "date":
		return "2026-03-08"
	case "datetime":
		return "2026-03-08T12:00:00Z"
	default:
		return "example-" + fieldName
	}
}

// splitPath splits a URL path into segments, filtering empty strings.
func splitPath(path string) []string {
	raw := strings.Split(path, "/")
	var parts []string
	for _, p := range raw {
		if p != "" {
			parts = append(parts, p)
		}
	}
	return parts
}

// matchRoute checks whether an HTTP method and request path match a route.
func matchRoute(rt route, method string, path string) bool {
	if rt.method != method {
		return false
	}
	reqParts := splitPath(path)
	if len(reqParts) != len(rt.parts) {
		return false
	}
	for i, p := range rt.parts {
		if strings.HasPrefix(p, ":") {
			continue // wildcard param — matches anything
		}
		if p != reqParts[i] {
			return false
		}
	}
	return true
}

// printBanner prints a startup message listing all registered routes.
func printBanner(a ast.AST, routes []route, port int) {
	fmt.Println()
	fmt.Printf("\033[1m  Veld Mock Server\033[0m\n")
	fmt.Printf("  Listening on \033[32mhttp://localhost:%d\033[0m\n", port)
	fmt.Println()

	if len(routes) == 0 {
		fmt.Println("  \033[33mNo routes registered.\033[0m")
		fmt.Println()
		return
	}

	fmt.Println("  \033[1mRoutes:\033[0m")
	for _, rt := range routes {
		fullPath := "/" + strings.Join(rt.parts, "/")
		methodColor := "\033[36m" // cyan default
		switch rt.method {
		case "GET":
			methodColor = "\033[32m" // green
		case "POST":
			methodColor = "\033[33m" // yellow
		case "PUT":
			methodColor = "\033[34m" // blue
		case "PATCH":
			methodColor = "\033[35m" // magenta
		case "DELETE":
			methodColor = "\033[31m" // red
		}
		fmt.Printf("    %s%-6s\033[0m %s\n", methodColor, rt.method, fullPath)
	}
	fmt.Println()
	fmt.Println("  \033[2mPress Ctrl+C to stop.\033[0m")
	fmt.Println()
}

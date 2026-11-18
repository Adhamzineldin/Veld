// Package emittertest holds fixtures and helpers shared by emitter tests.
// It is imported only from _test.go files.
package emittertest

import (
	"regexp"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// ShadowingAST has static GET routes declared after a parametric GET route on
// the same prefix, one of them guarded by middleware: the F-NODE-05 shape.
func ShadowingAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{{Name: "Item", Fields: []ast.Field{{Name: "id", Type: "string"}}}},
		Modules: []ast.Module{{
			Name:   "Items",
			Prefix: "/api",
			Actions: []ast.Action{
				{Name: "GetItem", Method: "GET", Path: "/items/:id", Output: "Item"},
				{Name: "GetMe", Method: "GET", Path: "/items/me", Output: "Item"},
				{Name: "GetGuarded", Method: "GET", Path: "/items/guarded", Output: "Item", Middleware: []string{"AuthGuard"}},
			},
		}},
	}
}

var registration = regexp.MustCompile(`router\.(get|post|put|patch|delete)\('([^']*)'`)

// RouteRegistrations lists "METHOD path" for every router.<method>('path', ...)
// call in generated source, in registration order.
func RouteRegistrations(src string) []string {
	var regs []string
	for _, m := range registration.FindAllStringSubmatch(src, -1) {
		regs = append(regs, m[1]+" "+m[2])
	}
	return regs
}

// RequestModelsAST has one action per way a model reaches a route from the
// request URL or headers: a query model, a headers model and a GET input
// (sent as query parameters). These are the F-NODE-06 strict-mode cases.
func RequestModelsAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{Name: "Item", Fields: []ast.Field{{Name: "id", Type: "string"}}},
			{Name: "SearchQuery", Fields: []ast.Field{{Name: "q", Type: "string"}}},
			{Name: "TraceHeaders", Fields: []ast.Field{{Name: "X-Trace-Id", Type: "string"}}},
		},
		Modules: []ast.Module{{
			Name:   "Items",
			Prefix: "/api",
			Actions: []ast.Action{
				{Name: "Search", Method: "GET", Path: "/search", Query: "SearchQuery", Output: "Item", OutputArray: true},
				{Name: "Lookup", Method: "GET", Path: "/lookup", Input: "SearchQuery", Output: "Item", OutputArray: true},
				{Name: "Secure", Method: "GET", Path: "/secure", Headers: "TraceHeaders", Output: "Item"},
			},
		}},
	}
}

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

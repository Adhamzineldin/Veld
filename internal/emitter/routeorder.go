package emitter

import (
	"sort"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// OrderBySpecificity returns actions in a safe registration order for routers
// that try routes in the order they were registered, such as Express
// (F-NODE-05). Paths are compared segment by segment: at the first segment
// where one path has a literal and the other a :param, the literal path comes
// first, so `GET /items/me` is registered before `GET /items/:id` and is not
// shadowed by it. Paths with the same literal/param shape keep their
// declaration order. The input slice is not modified.
//
// Only the shape of a path matters, so the result is deterministic and does
// not depend on the HTTP method or the module prefix (shared by all actions of
// a module).
func OrderBySpecificity(actions []ast.Action) []ast.Action {
	ordered := append([]ast.Action(nil), actions...)
	shapes := make(map[string][]bool, len(ordered))
	for _, a := range ordered {
		shapes[a.Path] = pathShape(a.Path)
	}
	sort.SliceStable(ordered, func(i, j int) bool {
		return shapeLess(shapes[ordered[i].Path], shapes[ordered[j].Path])
	})
	return ordered
}

// pathShape reports, per path segment, whether it is a :param.
func pathShape(path string) []bool {
	var shape []bool
	for _, seg := range strings.Split(strings.Trim(path, "/"), "/") {
		if seg == "" {
			continue
		}
		shape = append(shape, strings.HasPrefix(seg, ":"))
	}
	return shape
}

// shapeLess orders shapes lexicographically with literal before param and a
// shorter shape before its extensions.
func shapeLess(a, b []bool) bool {
	for k := 0; k < len(a) && k < len(b); k++ {
		if a[k] != b[k] {
			return !a[k]
		}
	}
	return len(a) < len(b)
}

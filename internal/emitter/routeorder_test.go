package emitter

import (
	"reflect"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func TestOrderBySpecificity(t *testing.T) {
	cases := []struct {
		name  string
		paths []string // declaration order
		want  []string // registration order
	}{
		{
			name:  "static declared after param moves ahead (F-NODE-05)",
			paths: []string{"/items/:id", "/items/me", "/items/guarded"},
			want:  []string{"/items/me", "/items/guarded", "/items/:id"},
		},
		{
			name:  "deeper static segment wins",
			paths: []string{"/a/:x/:y", "/a/:x/b"},
			want:  []string{"/a/:x/b", "/a/:x/:y"},
		},
		{
			name:  "same shape keeps declaration order",
			paths: []string{"/users", "/items", "/orders"},
			want:  []string{"/users", "/items", "/orders"},
		},
		{
			name:  "already safe order is unchanged",
			paths: []string{"/items/me", "/items/:id", "/items/:id/owner"},
			want:  []string{"/items/me", "/items/:id", "/items/:id/owner"},
		},
		{
			name:  "root and trailing slashes",
			paths: []string{"/:id", "/", "/health/"},
			want:  []string{"/", "/health/", "/:id"},
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			actions := make([]ast.Action, len(tc.paths))
			for i, p := range tc.paths {
				actions[i] = ast.Action{Name: p, Method: "GET", Path: p}
			}

			got := OrderBySpecificity(actions)

			paths := make([]string, len(got))
			for i, a := range got {
				paths[i] = a.Path
			}
			if !reflect.DeepEqual(paths, tc.want) {
				t.Errorf("order = %v, want %v", paths, tc.want)
			}
			if actions[0].Path != tc.paths[0] {
				t.Errorf("input slice was modified")
			}
		})
	}
}

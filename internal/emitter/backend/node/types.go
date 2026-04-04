package node

// types.go — emits types/{module}.ts + types/index.ts barrel.
//
// Delegates to tsshared.EmitTSTypes so the same TypeScript type output is
// produced whether the backend is node, java, csharp, go, or any other target.

import (
	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter/tsshared"
)

func (e *NodeEmitter) emitPerModuleTypes(a ast.AST, outDir string) error {
	return tsshared.EmitTSTypes(a, outDir)
}

// appendUnique is kept here for use by other node emitter files.
func appendUnique(slice []string, s string) []string {
	for _, v := range slice {
		if v == s {
			return slice
		}
	}
	return append(slice, s)
}

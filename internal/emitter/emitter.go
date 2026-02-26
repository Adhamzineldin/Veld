package emitter

import "github.com/veld-dev/veld/internal/ast"

// Emitter writes generated output files for a given AST.
type Emitter interface {
	Emit(a ast.AST, outDir string) error
}

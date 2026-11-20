package node

// errors.go — delegates error generation to tsshared.EmitTSErrors so the
// same errors/ directory is produced whether the frontend is typescript, react,
// vue, svelte, or any other TS-based emitter.

import (
	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter/tsshared"
)

func (e *NodeEmitter) emitErrorsBase(a ast.AST, outDir string) error {
	// Errors are now fully emitted by EmitTSErrors; this is a no-op so the
	// call site in main.go continues to compile without change.
	return nil
}

func (e *NodeEmitter) emitErrorsBarrel(a ast.AST, outDir string) error {
	// Delegated to EmitTSErrors — called once from Emit().
	return nil
}

func (e *NodeEmitter) emitErrors(mod ast.Module, outDir string) error {
	// Delegated to EmitTSErrors — called once from Emit().
	return nil
}

// emitAllErrors is called once from NodeEmitter.Emit() to generate the full
// errors/ directory via the shared implementation.
func (e *NodeEmitter) emitAllErrors(a ast.AST, outDir string) error {
	return tsshared.EmitTSErrors(a, outDir)
}

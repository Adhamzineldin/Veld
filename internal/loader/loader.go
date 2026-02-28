package loader

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/veld-dev/veld/internal/ast"
	"github.com/veld-dev/veld/internal/lexer"
	"github.com/veld-dev/veld/internal/parser"
)

// Parse loads a .veld entry point and recursively follows import statements.
// Returns the merged AST and the absolute paths of every .veld file that was
// loaded (for watch / incremental purposes).
func Parse(path string) (ast.AST, []string, error) {
	var files []string
	a, err := resolveFile(path, make(map[string]bool), &files)
	return a, files, err
}

func resolveFile(path string, seen map[string]bool, files *[]string) (ast.AST, error) {
	abs, err := filepath.Abs(path)
	if err != nil {
		return ast.AST{}, err
	}
	if seen[abs] {
		return ast.AST{ASTVersion: "1.0.0"}, nil // circular import guard
	}
	seen[abs] = true
	*files = append(*files, abs)

	content, err := os.ReadFile(path)
	if err != nil {
		return ast.AST{}, fmt.Errorf("reading %s: %w", path, err)
	}
	tokens, err := lexer.New(string(content)).Tokenize()
	if err != nil {
		return ast.AST{}, fmt.Errorf("lexing %s: %w", path, err)
	}
	a, err := parser.New(tokens).Parse()
	if err != nil {
		return ast.AST{}, fmt.Errorf("parsing %s: %w", path, err)
	}

	// Tag every definition with the file it came from (used by incremental gen).
	for i := range a.Models {
		a.Models[i].SourceFile = abs
	}
	for i := range a.Modules {
		a.Modules[i].SourceFile = abs
	}
	for i := range a.Enums {
		a.Enums[i].SourceFile = abs
	}

	// Resolve imports relative to this file's directory.
	dir := filepath.Dir(abs)
	merged := ast.AST{ASTVersion: "1.0.0"}
	for _, imp := range a.Imports {
		imported, err := resolveFile(filepath.Join(dir, imp), seen, files)
		if err != nil {
			return ast.AST{}, fmt.Errorf("import %q: %w", imp, err)
		}
		merged.Models = append(merged.Models, imported.Models...)
		merged.Modules = append(merged.Modules, imported.Modules...)
		merged.Enums = append(merged.Enums, imported.Enums...)
	}
	merged.Models = append(merged.Models, a.Models...)
	merged.Modules = append(merged.Modules, a.Modules...)
	merged.Enums = append(merged.Enums, a.Enums...)
	return merged, nil
}

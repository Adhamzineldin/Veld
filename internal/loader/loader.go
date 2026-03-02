package loader

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/lexer"
	"github.com/Adhamzineldin/Veld/internal/parser"
)

// Parse loads a .veld entry point and recursively follows import statements.
// Returns the merged AST and the absolute paths of every .veld file that was
// loaded (for watch / incremental purposes).
//
// aliases is an optional map of @alias → relative-dir-from-rootDir.
// If nil or missing, the alias name is used directly as the folder name
// (e.g. @models → {rootDir}/models/).
func Parse(path string, aliases ...map[string]string) (ast.AST, []string, error) {
	abs, err := filepath.Abs(path)
	if err != nil {
		return ast.AST{}, nil, err
	}
	rootDir := filepath.Dir(abs)

	var aliasMap map[string]string
	if len(aliases) > 0 && aliases[0] != nil {
		aliasMap = aliases[0]
	}

	var files []string
	fileImports := make(map[string][]string)
	a, err := resolveFile(path, rootDir, aliasMap, make(map[string]bool), &files, fileImports)
	a.FileImports = fileImports
	return a, files, err
}

// resolveFile parses a single .veld file and recursively resolves its imports.
// rootDir is the entry-point directory; aliasMap maps alias names to sub-paths.
func resolveFile(path, rootDir string, aliasMap map[string]string, seen map[string]bool, files *[]string, fileImports map[string][]string) (ast.AST, error) {
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

	// Tag every definition with the file it came from.
	for i := range a.Models {
		a.Models[i].SourceFile = abs
	}
	for i := range a.Modules {
		a.Modules[i].SourceFile = abs
	}
	for i := range a.Enums {
		a.Enums[i].SourceFile = abs
	}

	// Resolve imports.
	// @alias/name.veld or @alias/* — resolved from rootDir using alias mapping.
	// plain/path.veld             — resolved relative to this file's directory.
	dir := filepath.Dir(abs)
	merged := ast.AST{ASTVersion: "1.0.0"}
	for _, imp := range a.Imports {
		if len(imp) > 0 && imp[0] == '@' {
			// Alias-based: extract alias and the remainder
			rest := imp[1:] // e.g. "models/auth.veld" or "models/*"
			slashIdx := -1
			for i, c := range rest {
				if c == '/' {
					slashIdx = i
					break
				}
			}
			alias := rest
			name := ""
			if slashIdx >= 0 {
				alias = rest[:slashIdx]
				name = rest[slashIdx+1:]
			}

			// Resolve alias to directory
			aliasDir := resolveAliasDir(alias, rootDir, aliasMap)

			if name == "*" {
				// Wildcard: load all .veld files in aliasDir
				entries, err := os.ReadDir(aliasDir)
				if err != nil {
					continue // directory may not exist — skip silently
				}
				for _, entry := range entries {
					if !entry.IsDir() && filepath.Ext(entry.Name()) == ".veld" {
						importPath := filepath.Join(aliasDir, entry.Name())
						importAbs, _ := filepath.Abs(importPath)
						fileImports[abs] = append(fileImports[abs], importAbs)
						imported, err := resolveFile(importPath, rootDir, aliasMap, seen, files, fileImports)
						if err != nil {
							return ast.AST{}, fmt.Errorf("import %q: %w", imp, err)
						}
						merged.Models = append(merged.Models, imported.Models...)
						merged.Modules = append(merged.Modules, imported.Modules...)
						merged.Enums = append(merged.Enums, imported.Enums...)
					}
				}
			} else {
				// Single alias-based file
				importPath := filepath.Join(aliasDir, name)
				importAbs, _ := filepath.Abs(importPath)
				fileImports[abs] = append(fileImports[abs], importAbs)
				imported, err := resolveFile(importPath, rootDir, aliasMap, seen, files, fileImports)
				if err != nil {
					return ast.AST{}, fmt.Errorf("import %q: %w", imp, err)
				}
				merged.Models = append(merged.Models, imported.Models...)
				merged.Modules = append(merged.Modules, imported.Modules...)
				merged.Enums = append(merged.Enums, imported.Enums...)
			}
		} else {
			// Legacy relative import: resolve from this file's directory
			importPath := filepath.Join(dir, imp)
			importAbs, _ := filepath.Abs(importPath)
			fileImports[abs] = append(fileImports[abs], importAbs)
			imported, err := resolveFile(importPath, rootDir, aliasMap, seen, files, fileImports)
			if err != nil {
				return ast.AST{}, fmt.Errorf("import %q: %w", imp, err)
			}
			merged.Models = append(merged.Models, imported.Models...)
			merged.Modules = append(merged.Modules, imported.Modules...)
			merged.Enums = append(merged.Enums, imported.Enums...)
		}
	}
	merged.Models = append(merged.Models, a.Models...)
	merged.Modules = append(merged.Modules, a.Modules...)
	merged.Enums = append(merged.Enums, a.Enums...)
	// Propagate app-level prefix (first non-empty wins).
	if a.Prefix != "" && merged.Prefix == "" {
		merged.Prefix = a.Prefix
	}
	return merged, nil
}

// resolveAliasDir maps an alias name to its absolute directory.
// If aliasMap has an entry, that relative path (from rootDir) is used.
// Otherwise the alias name itself is used as a subdirectory of rootDir.
func resolveAliasDir(alias, rootDir string, aliasMap map[string]string) string {
	if aliasMap != nil {
		if mapped, ok := aliasMap[alias]; ok {
			return filepath.Join(rootDir, mapped)
		}
	}
	return filepath.Join(rootDir, alias) // default: alias == folder name
}

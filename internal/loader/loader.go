package loader

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/lexer"
	"github.com/Adhamzineldin/Veld/internal/parser"
)

// Parse loads a .veld entry point and recursively follows import statements.
// Returns the merged AST and the absolute paths of every .veld file loaded.
//
// aliases is an optional map of @alias → relative-dir-from-rootDir.
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

func resolveFile(path, rootDir string, aliasMap map[string]string, seen map[string]bool, files *[]string, fileImports map[string][]string) (ast.AST, error) {
	abs, err := filepath.Abs(path)
	if err != nil {
		return ast.AST{}, err
	}
	if seen[abs] {
		return ast.AST{ASTVersion: "1.0.0"}, nil
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

	for i := range a.Models {
		a.Models[i].SourceFile = abs
	}
	for i := range a.Modules {
		a.Modules[i].SourceFile = abs
	}
	for i := range a.Enums {
		a.Enums[i].SourceFile = abs
	}

	dir := filepath.Dir(abs)
	merged := ast.AST{ASTVersion: "1.0.0"}

	for _, imp := range a.Imports {
		if len(imp) > 0 && imp[0] == '@' {
			rest := imp[1:] // e.g. "models/user.veld", "org/pkg/modules/*"

			// ── Try registry package: @org/package[/subpath] ──────────────
			// Registry pulls land at rootDir/packages/@org/package/
			if aliasDir, subpath, ok := resolveRegistryImport(rest, rootDir); ok {
				imported, err := loadFromDir(aliasDir, subpath, imp, rootDir, aliasMap, seen, files, fileImports)
				if err != nil {
					return ast.AST{}, err
				}
				merged = mergeAST(merged, imported)
				continue
			}

			// ── Standard alias: @alias[/name] ─────────────────────────────
			slashIdx := strings.Index(rest, "/")
			alias, name := rest, ""
			if slashIdx >= 0 {
				alias = rest[:slashIdx]
				name = rest[slashIdx+1:]
			}
			aliasDir := resolveAliasDir(alias, rootDir, aliasMap)
			imported, err := loadFromDir(aliasDir, name, imp, rootDir, aliasMap, seen, files, fileImports)
			if err != nil {
				return ast.AST{}, err
			}
			merged = mergeAST(merged, imported)

		} else {
			// Legacy relative import
			importPath := filepath.Join(dir, imp)
			importAbs, _ := filepath.Abs(importPath)
			fileImports[abs] = append(fileImports[abs], importAbs)
			imported, err := resolveFile(importPath, rootDir, aliasMap, seen, files, fileImports)
			if err != nil {
				return ast.AST{}, fmt.Errorf("import %q: %w", imp, err)
			}
			merged = mergeAST(merged, imported)
		}
	}

	merged = mergeAST(merged, a)
	if a.Prefix != "" && merged.Prefix == "" {
		merged.Prefix = a.Prefix
	}
	return merged, nil
}

// resolveRegistryImport detects @org/package[/subpath] imports and maps them
// to rootDir/packages/@org/package. Returns (dir, subpath, true) on match.
func resolveRegistryImport(rest, rootDir string) (dir, subpath string, ok bool) {
	// Need at least "org/package"
	parts := strings.SplitN(rest, "/", 3)
	if len(parts) < 2 {
		return "", "", false
	}
	org, pkg := parts[0], parts[1]
	sub := ""
	if len(parts) == 3 {
		sub = parts[2]
	}

	candidate := filepath.Join(rootDir, "packages", "@"+org, pkg)
	if _, err := os.Stat(candidate); err != nil {
		return "", "", false
	}
	return candidate, sub, true
}

// loadFromDir loads .veld files from aliasDir according to the subpath pattern:
//
//	""        → load all .veld files recursively in aliasDir
//	"*"       → load all .veld files in aliasDir (non-recursive)
//	"sub/*"   → load all .veld files in aliasDir/sub/
//	"sub/**"  → load all .veld files in aliasDir/sub/ recursively
//	"file.veld" → load single file
func loadFromDir(aliasDir, subpath, origImport, rootDir string, aliasMap map[string]string, seen map[string]bool, files *[]string, fileImports map[string][]string) (ast.AST, error) {
	merged := ast.AST{ASTVersion: "1.0.0"}

	// Determine target directory and pattern
	targetDir := aliasDir
	pattern := subpath
	recursive := false

	if subpath == "" {
		// bare @org/package — load everything recursively
		recursive = true
		pattern = "**"
	} else if strings.HasSuffix(subpath, "/**") {
		targetDir = filepath.Join(aliasDir, strings.TrimSuffix(subpath, "/**"))
		pattern = "**"
		recursive = true
	} else if strings.HasSuffix(subpath, "/*") {
		targetDir = filepath.Join(aliasDir, strings.TrimSuffix(subpath, "/*"))
		pattern = "*"
	} else if subpath == "*" || subpath == "**" {
		recursive = subpath == "**"
		pattern = subpath
	}

	switch pattern {
	case "*":
		// Non-recursive glob of targetDir
		entries, err := os.ReadDir(targetDir)
		if err != nil {
			return merged, nil // directory may not exist — skip silently
		}
		for _, entry := range entries {
			if !entry.IsDir() && filepath.Ext(entry.Name()) == ".veld" {
				p := filepath.Join(targetDir, entry.Name())
				abs, _ := filepath.Abs(p)
				fileImports[abs] = append(fileImports[abs], abs)
				imp, err := resolveFile(p, rootDir, aliasMap, seen, files, fileImports)
				if err != nil {
					return ast.AST{}, fmt.Errorf("import %q: %w", origImport, err)
				}
				merged = mergeAST(merged, imp)
			}
		}

	case "**":
		// Recursive glob
		_ = recursive
		err := filepath.WalkDir(targetDir, func(p string, d os.DirEntry, walkErr error) error {
			if walkErr != nil || d.IsDir() || filepath.Ext(p) != ".veld" {
				return walkErr
			}
			abs, _ := filepath.Abs(p)
			fileImports[abs] = append(fileImports[abs], abs)
			imp, err := resolveFile(p, rootDir, aliasMap, seen, files, fileImports)
			if err != nil {
				return fmt.Errorf("import %q: %w", origImport, err)
			}
			merged = mergeAST(merged, imp)
			return nil
		})
		if err != nil {
			return ast.AST{}, err
		}

	default:
		// Single file
		p := filepath.Join(aliasDir, subpath)
		abs, _ := filepath.Abs(p)
		fileImports[abs] = append(fileImports[abs], abs)
		imp, err := resolveFile(p, rootDir, aliasMap, seen, files, fileImports)
		if err != nil {
			return ast.AST{}, fmt.Errorf("import %q: %w", origImport, err)
		}
		merged = mergeAST(merged, imp)
	}

	return merged, nil
}

func resolveAliasDir(alias, rootDir string, aliasMap map[string]string) string {
	if aliasMap != nil {
		if mapped, ok := aliasMap[alias]; ok {
			return filepath.Join(rootDir, mapped)
		}
	}
	return filepath.Join(rootDir, alias)
}

func mergeAST(dst, src ast.AST) ast.AST {
	dst.Models = append(dst.Models, src.Models...)
	dst.Modules = append(dst.Modules, src.Modules...)
	dst.Enums = append(dst.Enums, src.Enums...)
	return dst
}

package language

import (
	"fmt"
	"strings"
)

// ImportAlias defines a path alias like @models/, @modules/, @types/
type ImportAlias struct {
	Name string // "models", "modules", "types", etc.
	Path string // "./models", "./modules", etc.
}

// DefaultImportAliases provides standard project structure aliases
func DefaultImportAliases() map[string]ImportAlias {
	return map[string]ImportAlias{
		"models":   {Name: "models", Path: "./models"},
		"modules":  {Name: "modules", Path: "./modules"},
		"types":    {Name: "types", Path: "./types"},
		"enums":    {Name: "enums", Path: "./enums"},
		"schemas":  {Name: "schemas", Path: "./schemas"},
		"services": {Name: "services", Path: "./services"},
		"lib":      {Name: "lib", Path: "./lib"},
		"common":   {Name: "common", Path: "./common"},
	}
}

// ValidateImports checks that all imported symbols are used/exported
func (spec *VeldLanguageSpec) ValidateImports(file *VeldFile, aliases map[string]ImportAlias) []VeldError {
	var errors []VeldError

	// Check each import
	for importPath, resolvedPath := range file.Imports {
		// Track if this import is actually used
		used := false

		// Parse import path: @models/user -> "models", "user"
		parts := strings.Split(importPath, "/")
		if len(parts) < 2 {
			errors = append(errors, VeldError{
				File:    file.Path,
				Line:    1,
				Message: fmt.Sprintf("Invalid import path '%s'. Use format: @alias/name", importPath),
				Code:    "ERROR_INVALID_IMPORT_PATH",
			})
			continue
		}

		aliasName := strings.TrimPrefix(parts[0], "@")
		importedName := strings.TrimSuffix(parts[1], ".veld")

		// Check if import name is referenced in the file
		for _, model := range file.Models {
			// Check if type references this import
			for _, field := range model.Fields {
				if strings.HasPrefix(field.Type, aliasName+".") || field.Type == importedName {
					used = true
					break
				}
			}
		}

		// Check in actions
		for _, module := range file.Modules {
			for _, action := range module.Actions {
				if action.Input != "" && (strings.HasPrefix(action.Input, aliasName+".") || action.Input == importedName) {
					used = true
				}
				if action.Output != "" && (strings.HasPrefix(action.Output, aliasName+".") || action.Output == importedName) {
					used = true
				}
			}
		}

		// If import is declared but not used, warn
		if !used {
			errors = append(errors, VeldError{
				File:    file.Path,
				Line:    1, // TODO: track import line number
				Message: fmt.Sprintf("Import '@%s/%s' is not used", aliasName, importedName),
				Code:    "WARN_UNUSED_IMPORT",
			})
		}

		// Validate alias exists
		if _, hasAlias := aliases[aliasName]; !hasAlias {
			errors = append(errors, VeldError{
				File:    file.Path,
				Line:    1,
				Message: fmt.Sprintf("Unknown import alias '@%s'. Valid aliases: %v", aliasName, getAliasNames(aliases)),
				Code:    "ERROR_UNKNOWN_IMPORT_ALIAS",
			})
		}
	}

	// Check for undefined types that should be imported
	for _, model := range file.Models {
		for _, field := range model.Fields {
			typeName := extractBaseType(field.Type)
			if !spec.IsBuiltinType(typeName) && !file.Models[typeName] && !file.Enums[typeName] {
				// Check if it should come from an import
				if !strings.Contains(field.Type, ".") {
					errors = append(errors, VeldError{
						File:    file.Path,
						Line:    model.Line,
						Message: fmt.Sprintf("Type '%s' not found. Did you forget to import it? Use: import @alias/%s", typeName, strings.ToLower(typeName)),
						Code:    "ERROR_UNDEFINED_TYPE_MISSING_IMPORT",
					})
				}
			}
		}
	}

	return errors
}

// ResolveImportPath converts @models/user to ./models/user.veld
func ResolveImportPath(importPath string, aliases map[string]ImportAlias) (string, error) {
	parts := strings.Split(importPath, "/")
	if len(parts) < 2 {
		return "", fmt.Errorf("invalid import path '%s'. Use format: @alias/name", importPath)
	}

	aliasName := strings.TrimPrefix(parts[0], "@")
	name := parts[1]

	alias, hasAlias := aliases[aliasName]
	if !hasAlias {
		return "", fmt.Errorf("unknown alias '@%s'", aliasName)
	}

	return alias.Path + "/" + name + ".veld", nil
}

// extractBaseType gets the base type from List<T>, Map<K,V>, etc.
func extractBaseType(typeStr string) string {
	if strings.HasPrefix(typeStr, "List<") && strings.HasSuffix(typeStr, ">") {
		return typeStr[5 : len(typeStr)-1]
	}
	if strings.HasPrefix(typeStr, "Map<") && strings.HasSuffix(typeStr, ">") {
		parts := strings.Split(typeStr[4:len(typeStr)-1], ",")
		if len(parts) > 1 {
			return strings.TrimSpace(parts[1])
		}
	}
	return typeStr
}

// getAliasNames returns list of valid alias names
func getAliasNames(aliases map[string]ImportAlias) []string {
	var names []string
	for name := range aliases {
		names = append(names, "@"+name)
	}
	return names
}

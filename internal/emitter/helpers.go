package emitter

import (
	"regexp"
	"sort"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// paramRegex matches Express-style path params like :id, :userId etc.
var paramRegex = regexp.MustCompile(`:([a-zA-Z_][a-zA-Z0-9_]*)`)

// ExtractPathParams returns the parameter names from a route path like /users/:id.
func ExtractPathParams(path string) []string {
	matches := paramRegex.FindAllStringSubmatch(path, -1)
	params := make([]string, 0, len(matches))
	for _, m := range matches {
		params = append(params, m[1])
	}
	return params
}

// ToTemplateLiteral converts /users/:id to /users/${id} for JS template literals.
func ToTemplateLiteral(path string) string {
	return paramRegex.ReplaceAllString(path, `${$1}`)
}

// ToFlaskPath converts /users/:id to /users/<id> for Flask route registration.
func ToFlaskPath(path string) string {
	return paramRegex.ReplaceAllString(path, `<$1>`)
}

// ToOpenAPIPath converts /users/:id to /users/{id} for OpenAPI specs.
func ToOpenAPIPath(path string) string {
	return paramRegex.ReplaceAllString(path, `{$1}`)
}

// ToChiPath converts /users/:id to /users/{id} for Chi router.
func ToChiPath(path string) string {
	return paramRegex.ReplaceAllString(path, `{$1}`)
}

// CollectTransitiveModels returns all model names needed by a module, following
// model references in fields transitively.
func CollectTransitiveModels(a ast.AST, mod ast.Module) map[string]bool {
	byName := make(map[string]ast.Model, len(a.Models))
	for _, m := range a.Models {
		byName[m.Name] = m
	}

	used := make(map[string]bool)
	queue := []string{}
	for _, act := range mod.Actions {
		if act.Input != "" {
			queue = append(queue, act.Input)
		}
		if act.Output != "" {
			queue = append(queue, act.Output)
		}
		if act.Query != "" {
			queue = append(queue, act.Query)
		}
		if act.Stream != "" {
			queue = append(queue, act.Stream)
		}
	}
	for len(queue) > 0 {
		name := queue[0]
		queue = queue[1:]
		if used[name] {
			continue
		}
		used[name] = true
		if m, ok := byName[name]; ok {
			// Follow extends chain
			if m.Extends != "" && !used[m.Extends] {
				queue = append(queue, m.Extends)
			}
			for _, f := range m.Fields {
				// Regular model references
				if _, isModel := byName[f.Type]; isModel && !used[f.Type] {
					queue = append(queue, f.Type)
				}
				// Union type members that reference models
				for _, ut := range f.UnionTypes {
					if _, isModel := byName[ut]; isModel && !used[ut] {
						queue = append(queue, ut)
					}
				}
				// Map<string, V> value type references
				if f.IsMap {
					if _, isModel := byName[f.MapValueType]; isModel && !used[f.MapValueType] {
						queue = append(queue, f.MapValueType)
					}
				}
			}
		}
	}
	return used
}

// CollectUsedEnums returns the set of enum names referenced by models used in a module.
func CollectUsedEnums(a ast.AST, mod ast.Module) map[string]bool {
	enumNames := make(map[string]bool)
	for _, en := range a.Enums {
		enumNames[en.Name] = true
	}

	usedModels := CollectTransitiveModels(a, mod)
	usedEnums := make(map[string]bool)

	for _, m := range a.Models {
		if !usedModels[m.Name] {
			continue
		}
		for _, f := range m.Fields {
			if enumNames[f.Type] {
				usedEnums[f.Type] = true
			}
			if f.IsMap && enumNames[f.MapValueType] {
				usedEnums[f.MapValueType] = true
			}
			for _, ut := range f.UnionTypes {
				if enumNames[ut] {
					usedEnums[ut] = true
				}
			}
		}
	}
	return usedEnums
}

// CollectUsedTypes returns the unique type names (models + enums) referenced
// directly or transitively by a module, in a stable order.
func CollectUsedTypes(a ast.AST, mod ast.Module) []string {
	seen := make(map[string]bool)
	var result []string

	enumNames := make(map[string]bool)
	for _, en := range a.Enums {
		enumNames[en.Name] = true
	}

	// Direct action references.
	for _, act := range mod.Actions {
		for _, name := range []string{act.Input, act.Output, act.Query, act.Stream} {
			if name != "" && !seen[name] && !IsPrimitive(name) {
				seen[name] = true
				result = append(result, name)
			}
		}
	}

	// Transitive model references.
	usedModels := CollectTransitiveModels(a, mod)
	byName := make(map[string]ast.Model, len(a.Models))
	for _, m := range a.Models {
		byName[m.Name] = m
	}
	for _, m := range a.Models {
		if !usedModels[m.Name] {
			continue
		}
		if !seen[m.Name] {
			seen[m.Name] = true
			result = append(result, m.Name)
		}
		for _, f := range m.Fields {
			base := f.Type
			if _, isModel := byName[base]; isModel && !seen[base] {
				seen[base] = true
				result = append(result, base)
			}
			if enumNames[base] && !seen[base] {
				seen[base] = true
				result = append(result, base)
			}
			// Union type members that reference models or enums
			for _, ut := range f.UnionTypes {
				if _, isModel := byName[ut]; isModel && !seen[ut] {
					seen[ut] = true
					result = append(result, ut)
				}
				if enumNames[ut] && !seen[ut] {
					seen[ut] = true
					result = append(result, ut)
				}
			}
		}
	}

	// Enums.
	usedEnums := CollectUsedEnums(a, mod)
	for _, en := range a.Enums {
		if usedEnums[en.Name] && !seen[en.Name] {
			seen[en.Name] = true
			result = append(result, en.Name)
		}
	}

	return result
}

// IsPrimitive returns true for built-in Veld scalar types.
func IsPrimitive(t string) bool {
	switch t {
	case "string", "int", "float", "decimal", "bool", "date", "datetime", "uuid":
		return true
	}
	return false
}

// CollectModuleMiddleware returns all unique middleware names used in a module,
// in order of first appearance.
func CollectModuleMiddleware(mod ast.Module) []string {
	seen := make(map[string]bool)
	var result []string
	for _, act := range mod.Actions {
		for _, mw := range act.Middleware {
			if !seen[mw] {
				seen[mw] = true
				result = append(result, mw)
			}
		}
	}
	return result
}

// CollectAllMiddleware returns all unique middleware names used across ALL modules,
// in order of first appearance. This is used to emit a single shared middleware interface.
func CollectAllMiddleware(modules []ast.Module) []string {
	seen := make(map[string]bool)
	var result []string
	for _, mod := range modules {
		for _, act := range mod.Actions {
			for _, mw := range act.Middleware {
				if !seen[mw] {
					seen[mw] = true
					result = append(result, mw)
				}
			}
		}
	}
	return result
}

// ToPascalCase ensures the first letter is uppercase (PascalCase / UpperCamelCase).
func ToPascalCase(s string) string {
	if s == "" {
		return s
	}
	r := []rune(s)
	if r[0] >= 'a' && r[0] <= 'z' {
		r[0] -= 32
	}
	return string(r)
}

// ToCamelCase converts a PascalCase name to camelCase (lowercase first letter).
func ToCamelCase(s string) string {
	if s == "" {
		return s
	}
	// Find the boundary: lowercase everything up to (but not including) the
	// last uppercase letter in a leading-uppercase run.  E.g. "HTTPClient" → "httpClient".
	i := 0
	for i < len(s) && s[i] >= 'A' && s[i] <= 'Z' {
		i++
	}
	switch {
	case i == 0:
		return s // already starts lowercase
	case i == 1:
		return strings.ToLower(s[:1]) + s[1:]
	default:
		// e.g. i==4 for "HTTPClient" → "http" + "Client"
		return strings.ToLower(s[:i-1]) + s[i-1:]
	}
}

// ToSnakeCase converts a camelCase or PascalCase name to snake_case.
func ToSnakeCase(s string) string {
	var result strings.Builder
	for i, r := range s {
		if r >= 'A' && r <= 'Z' {
			if i > 0 {
				result.WriteByte('_')
			}
			result.WriteRune(r + 32) // to lower
		} else {
			result.WriteRune(r)
		}
	}
	return result.String()
}

// ResolveRoutePath computes the full route path by joining the app-level prefix,
// the module prefix, and the action path. Empty segments are skipped.
func ResolveRoutePath(appPrefix string, mod ast.Module, act ast.Action) string {
	return appPrefix + mod.Prefix + act.Path
}

// ToScreamingSnake converts a camelCase or PascalCase name to SCREAMING_SNAKE_CASE.
func ToScreamingSnake(s string) string {
	return strings.ToUpper(ToSnakeCase(s))
}

// ErrorCode builds a deterministic error code string from an action name and
// error name. Example: ("ListUsers", "NotFound") → "LIST_USERS_NOT_FOUND".
func ErrorCode(actionName, errorName string) string {
	return ToScreamingSnake(actionName) + "_" + ToScreamingSnake(errorName)
}

// ErrorHTTPStatus maps well-known error names to HTTP status codes.
// Unknown names default to 500.
var errorStatusMap = map[string]int{
	"NotFound":            404,
	"Unauthorized":        401,
	"Forbidden":           403,
	"Conflict":            409,
	"BadRequest":          400,
	"ValidationFailed":    422,
	"Gone":                410,
	"TooManyRequests":     429,
	"InternalServerError": 500,
	"ServiceUnavailable":  503,
	"NotImplemented":      501,
}

// ErrorHTTPStatus returns the HTTP status code for a well-known error name.
// Unknown names return 500.
func ErrorHTTPStatus(errorName string) int {
	if status, ok := errorStatusMap[errorName]; ok {
		return status
	}
	return 500
}

// ActionErrorStatus returns the HTTP status for an error on a specific action.
// It uses the explicit status from the contract (errors: [name:status]) when
// present, falling back to the well-known name map, then 500.
func ActionErrorStatus(act ast.Action, errName string) int {
	if act.ErrorStatuses != nil {
		if s, ok := act.ErrorStatuses[errName]; ok {
			return s
		}
	}
	return ErrorHTTPStatus(errName)
}

// HasErrors returns true if any action in the module defines error codes.
func HasErrors(mod ast.Module) bool {
	for _, act := range mod.Actions {
		if len(act.Errors) > 0 {
			return true
		}
	}
	return false
}

// CollectErrorExports returns the list of exported symbol names that a
// module's generated error file will contain (excluding the re-exported ApiError).
// This is used by barrel generators to detect and avoid duplicate export collisions.
func CollectErrorExports(mod ast.Module) []string {
	moduleLower := strings.ToLower(mod.Name)
	var names []string
	for _, act := range mod.Actions {
		if len(act.Errors) == 0 {
			continue
		}
		pascal := act.Name
		camelAction := ToCamelCase(act.Name)
		names = append(names, pascal+"ErrorCode", pascal+"Error", camelAction+"Errors")
	}
	names = append(names, moduleLower+"Errors")
	return names
}

// AssignModelsToModules maps each model and enum to the module whose actions
// reference it (transitively). Models/enums not referenced by any action are
// assigned to the "shared" group. Returns per-group slices (in AST order) plus
// ownership maps for looking up which group a type belongs to.
func AssignModelsToModules(a ast.AST) (modelGroups map[string][]ast.Model, enumGroups map[string][]ast.Enum, modelOwner map[string]string, enumOwner map[string]string) {
	modelGroups = make(map[string][]ast.Model)
	enumGroups = make(map[string][]ast.Enum)
	modelOwner = make(map[string]string)
	enumOwner = make(map[string]string)

	for _, mod := range a.Modules {
		modLower := strings.ToLower(mod.Name)
		for name := range CollectTransitiveModels(a, mod) {
			if _, already := modelOwner[name]; !already {
				modelOwner[name] = modLower
			}
		}
		for name := range CollectUsedEnums(a, mod) {
			if _, already := enumOwner[name]; !already {
				enumOwner[name] = modLower
			}
		}
	}

	for _, m := range a.Models {
		owner := modelOwner[m.Name]
		if owner == "" {
			owner = "shared"
			modelOwner[m.Name] = owner
		}
		modelGroups[owner] = append(modelGroups[owner], m)
	}
	for _, en := range a.Enums {
		owner := enumOwner[en.Name]
		if owner == "" {
			owner = "shared"
			enumOwner[en.Name] = owner
		}
		enumGroups[owner] = append(enumGroups[owner], en)
	}

	return
}

// SortedGroupKeys returns the unique keys across model and enum groups in
// alphabetical order, with "shared" always last (if present).
func SortedGroupKeys(modelGroups map[string][]ast.Model, enumGroups map[string][]ast.Enum) []string {
	seen := make(map[string]bool)
	var keys []string
	for k := range modelGroups {
		if !seen[k] && k != "shared" {
			seen[k] = true
			keys = append(keys, k)
		}
	}
	for k := range enumGroups {
		if !seen[k] && k != "shared" {
			seen[k] = true
			keys = append(keys, k)
		}
	}
	sort.Strings(keys)
	if modelGroups["shared"] != nil || enumGroups["shared"] != nil {
		keys = append(keys, "shared")
	}
	return keys
}

// CrossGroupTypeNames returns, for a given group's models, the set of type
// names that must be imported from other groups (due to extends chains and
// field references). Result maps source group name → list of type names.
func CrossGroupTypeNames(group string, models []ast.Model, modelOwner, enumOwner map[string]string) map[string][]string {
	result := make(map[string][]string)
	seen := make(map[string]bool)

	add := func(typeName string) {
		if seen[typeName] || typeName == "" || IsPrimitive(typeName) {
			return
		}
		owner := modelOwner[typeName]
		if owner == "" {
			owner = enumOwner[typeName]
		}
		if owner != "" && owner != group {
			seen[typeName] = true
			result[owner] = append(result[owner], typeName)
		}
	}

	for _, m := range models {
		if m.Extends != "" {
			add(m.Extends)
		}
		for _, f := range m.Fields {
			add(f.Type)
			if f.IsMap {
				add(f.MapValueType)
			}
			for _, ut := range f.UnionTypes {
				add(ut)
			}
		}
	}

	for k := range result {
		sort.Strings(result[k])
	}
	return result
}

// MergeASTs combines multiple service ASTs into one unified AST.
// Used when the frontend workspace entry consumes multiple backend services
// so the frontend SDK gets typed clients for every service in one import.
// Models and enums are deduplicated by name (first occurrence wins).
func MergeASTs(base ast.AST, consumed []ConsumedServiceInfo) ast.AST {
	if len(consumed) == 0 {
		return base
	}

	// Start with a copy of the base AST.
	merged := ast.AST{
		ASTVersion: base.ASTVersion,
		Prefix:     base.Prefix,
	}

	seenModels := make(map[string]bool)
	seenEnums := make(map[string]bool)
	seenModules := make(map[string]bool)

	addAST := func(a ast.AST) {
		for _, m := range a.Models {
			if !seenModels[m.Name] {
				seenModels[m.Name] = true
				merged.Models = append(merged.Models, m)
			}
		}
		for _, e := range a.Enums {
			if !seenEnums[e.Name] {
				seenEnums[e.Name] = true
				merged.Enums = append(merged.Enums, e)
			}
		}
		for _, mod := range a.Modules {
			if !seenModules[mod.Name] {
				seenModules[mod.Name] = true
				merged.Modules = append(merged.Modules, mod)
			}
		}
	}

	// Add the base AST first (the frontend's own .veld file, if any).
	addAST(base)

	// Then layer in each consumed service.
	for _, c := range consumed {
		addAST(c.AST)
	}

	return merged
}

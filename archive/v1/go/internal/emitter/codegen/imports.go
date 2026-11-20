package codegen

import (
	"fmt"
	"sort"
	"strings"
)

// ImportManager manages imports/dependencies for generated code.
// Deduplicates, groups, and formats imports based on language conventions.
type ImportManager struct {
	imports map[string]*ImportGroup
	order   []string // track insertion order
}

// ImportGroup represents a group of imports (e.g., stdlib, third-party, local).
type ImportGroup struct {
	Imports map[string]bool // import spec -> exists
	Group   ImportGroupType
}

// ImportGroupType categorizes import groups.
type ImportGroupType int

const (
	GroupStdlib ImportGroupType = iota
	GroupThirdParty
	GroupLocal
)

// NewImportManager creates a new import manager.
func NewImportManager() *ImportManager {
	return &ImportManager{
		imports: make(map[string]*ImportGroup),
		order:   []string{},
	}
}

// Add records an import. Returns true if newly added, false if already exists.
func (im *ImportManager) Add(importSpec string, group ImportGroupType) bool {
	key := importKey(importSpec)
	if im.imports[key] != nil {
		return false // already exists
	}
	im.imports[key] = &ImportGroup{
		Imports: map[string]bool{importSpec: true},
		Group:   group,
	}
	im.order = append(im.order, key)
	return true
}

// AddWithAlias records an import with an alias (for languages that support it).
// Format: "alias=importspec"
func (im *ImportManager) AddWithAlias(alias, importSpec string, group ImportGroupType) bool {
	key := importKey(alias + "=" + importSpec)
	if im.imports[key] != nil {
		return false
	}
	im.imports[key] = &ImportGroup{
		Imports: map[string]bool{alias + "=" + importSpec: true},
		Group:   group,
	}
	im.order = append(im.order, key)
	return true
}

// Format generates import statements based on language style.
// Syntax examples:
//
//	"go":     "import (" ... ")"
//	"rust":   "use module::Type;"
//	"java":   "import package.Class;"
//	"python": "from module import name"
func (im *ImportManager) Format(lang string) string {
	var buf strings.Builder

	switch lang {
	case "go":
		return im.formatGo()
	case "rust":
		return im.formatRust()
	case "java":
		return im.formatJava()
	case "python":
		return im.formatPython()
	case "csharp":
		return im.formatCSharp()
	case "php":
		return im.formatPHP()
	default:
		// Generic format: one import per line
		for _, key := range im.order {
			if group := im.imports[key]; group != nil {
				for imp := range group.Imports {
					buf.WriteString("import " + imp + "\n")
				}
			}
		}
	}

	return buf.String()
}

// formatGo generates Go import block.
// Handles both stdlib and third-party, deduplicates.
func (im *ImportManager) formatGo() string {
	if len(im.imports) == 0 {
		return ""
	}

	var buf strings.Builder

	// Collect imports by group
	stdlibImports := []string{}
	externalImports := []string{}

	for _, key := range im.order {
		if group := im.imports[key]; group != nil {
			for imp := range group.Imports {
				if group.Group == GroupStdlib {
					stdlibImports = append(stdlibImports, imp)
				} else {
					externalImports = append(externalImports, imp)
				}
			}
		}
	}

	sort.Strings(stdlibImports)
	sort.Strings(externalImports)

	// Single import
	if len(stdlibImports)+len(externalImports) == 1 {
		if len(stdlibImports) > 0 {
			return fmt.Sprintf("import \"%s\"\n", stdlibImports[0])
		}
		return fmt.Sprintf("import \"%s\"\n", externalImports[0])
	}

	// Multiple imports
	buf.WriteString("import (\n")
	for _, imp := range stdlibImports {
		buf.WriteString(fmt.Sprintf("\t\"%s\"\n", imp))
	}
	if len(stdlibImports) > 0 && len(externalImports) > 0 {
		buf.WriteString("\n")
	}
	for _, imp := range externalImports {
		buf.WriteString(fmt.Sprintf("\t\"%s\"\n", imp))
	}
	buf.WriteString(")\n")

	return buf.String()
}

// formatRust generates Rust use statements.
func (im *ImportManager) formatRust() string {
	var buf strings.Builder

	for _, key := range im.order {
		if group := im.imports[key]; group != nil {
			for imp := range group.Imports {
				if strings.Contains(imp, "=") {
					// Handle aliased imports: "alias=module::Type"
					parts := strings.Split(imp, "=")
					buf.WriteString(fmt.Sprintf("use %s as %s;\n", parts[1], parts[0]))
				} else {
					buf.WriteString(fmt.Sprintf("use %s;\n", imp))
				}
			}
		}
	}

	return buf.String()
}

// formatJava generates Java import statements.
func (im *ImportManager) formatJava() string {
	var buf strings.Builder

	for _, key := range im.order {
		if group := im.imports[key]; group != nil {
			for imp := range group.Imports {
				buf.WriteString(fmt.Sprintf("import %s;\n", imp))
			}
		}
	}

	return buf.String()
}

// formatPython generates Python import statements.
func (im *ImportManager) formatPython() string {
	var buf strings.Builder

	for _, key := range im.order {
		if group := im.imports[key]; group != nil {
			for imp := range group.Imports {
				if strings.Contains(imp, "=") {
					// Handle aliased imports
					parts := strings.Split(imp, "=")
					buf.WriteString(fmt.Sprintf("import %s as %s\n", parts[1], parts[0]))
				} else if strings.Contains(imp, ":") {
					// Handle "from X import Y" format
					parts := strings.Split(imp, ":")
					buf.WriteString(fmt.Sprintf("from %s import %s\n", parts[0], parts[1]))
				} else {
					buf.WriteString(fmt.Sprintf("import %s\n", imp))
				}
			}
		}
	}

	return buf.String()
}

// formatCSharp generates C# using statements.
func (im *ImportManager) formatCSharp() string {
	var buf strings.Builder

	for _, key := range im.order {
		if group := im.imports[key]; group != nil {
			for imp := range group.Imports {
				buf.WriteString(fmt.Sprintf("using %s;\n", imp))
			}
		}
	}

	return buf.String()
}

// formatPHP generates PHP use statements.
func (im *ImportManager) formatPHP() string {
	var buf strings.Builder

	for _, key := range im.order {
		if group := im.imports[key]; group != nil {
			for imp := range group.Imports {
				if strings.Contains(imp, "=") {
					// Handle aliased imports
					parts := strings.Split(imp, "=")
					buf.WriteString(fmt.Sprintf("use %s as %s;\n", parts[1], parts[0]))
				} else {
					buf.WriteString(fmt.Sprintf("use %s;\n", imp))
				}
			}
		}
	}

	return buf.String()
}

// importKey creates a unique key for deduplication.
func importKey(spec string) string {
	return spec
}

// All returns all imports as a slice.
func (im *ImportManager) All() []string {
	var result []string
	for _, key := range im.order {
		if group := im.imports[key]; group != nil {
			for imp := range group.Imports {
				result = append(result, imp)
			}
		}
	}
	return result
}

// Clear removes all imports.
func (im *ImportManager) Clear() {
	im.imports = make(map[string]*ImportGroup)
	im.order = []string{}
}

// Len returns the number of unique imports.
func (im *ImportManager) Len() int {
	return len(im.imports)
}

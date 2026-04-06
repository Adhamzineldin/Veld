package language

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

// VeldLanguage provides complete language definition and parsing
type VeldLanguage struct {
	Keywords     map[string]bool
	HttpMethods  map[string]bool
	BuiltinTypes map[string]bool
	Directives   map[string]bool
	SpecialTypes map[string]bool // List, Map, Optional
}

// VeldFile represents a parsed .veld file with all symbols
type VeldFile struct {
	Path    string
	Content string
	Models  map[string]*Model
	Enums   map[string]*Enum
	Modules map[string]*Module
	Imports map[string]string // "imported_name" -> "file_path"
	Errors  []VeldError
}

// Model represents a data structure
type Model struct {
	Name    string
	Line    int
	Fields  map[string]Field
	Extends string // if it extends another model
}

// Field represents a model field
type Field struct {
	Name     string
	Type     string
	Optional bool
}

// Enum represents an enumeration
type Enum struct {
	Name   string
	Line   int
	Values []string
}

// Module represents an API module
type Module struct {
	Name        string
	Line        int
	Description string
	Prefix      string
	Actions     map[string]*Action
}

// Action represents an API action
type Action struct {
	Name   string
	Line   int
	Method string
	Path   string
	Input  string
	Output string
}

// VeldError represents a validation error
type VeldError struct {
	File    string
	Line    int
	Column  int
	Message string
	Code    string // ERROR_UNDEFINED_TYPE, ERROR_INVALID_METHOD, etc.
}

// NewVeldLanguage creates the Veld language definition
func NewVeldLanguage() *VeldLanguage {
	return &VeldLanguage{
		Keywords: map[string]bool{
			"model":   true,
			"module":  true,
			"action":  true,
			"enum":    true,
			"import":  true,
			"extends": true,
		},
		HttpMethods: map[string]bool{
			"GET":     true,
			"POST":    true,
			"PUT":     true,
			"DELETE":  true,
			"PATCH":   true,
			"HEAD":    true,
			"OPTIONS": true,
		},
		BuiltinTypes: map[string]bool{
			"string":   true,
			"int":      true,
			"float":    true,
			"decimal":  true,
			"bool":     true,
			"date":     true,
			"datetime": true,
			"uuid":     true,
			"bytes":    true,
			"json":     true,
			"any":      true,
		},
		Directives: map[string]bool{
			"description": true,
			"prefix":      true,
			"method":      true,
			"path":        true,
			"input":       true,
			"output":      true,
			"default":     true,
		},
		SpecialTypes: map[string]bool{
			"List": true,
			"Map":  true,
		},
	}
}

// ParseFile parses a single .veld file
func (vl *VeldLanguage) ParseFile(path string) (*VeldFile, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	vf := &VeldFile{
		Path:    path,
		Content: string(content),
		Models:  make(map[string]*Model),
		Enums:   make(map[string]*Enum),
		Modules: make(map[string]*Module),
		Imports: make(map[string]string),
		Errors:  []VeldError{},
	}

	vl.parseContent(vf)
	return vf, nil
}

// parseContent parses the content of a Veld file
func (vl *VeldLanguage) parseContent(vf *VeldFile) {
	lines := strings.Split(vf.Content, "\n")

	for i := 0; i < len(lines); i++ {
		line := strings.TrimSpace(lines[i])
		lineNum := i + 1

		// Skip comments and empty lines
		if line == "" || strings.HasPrefix(line, "//") {
			continue
		}

		// Parse imports
		if strings.HasPrefix(line, "import") {
			vl.parseImport(vf, line, lineNum)
			continue
		}

		// Parse models
		if strings.HasPrefix(line, "model") {
			i = vl.parseModel(vf, lines, i, lineNum)
			continue
		}

		// Parse enums
		if strings.HasPrefix(line, "enum") {
			i = vl.parseEnum(vf, lines, i, lineNum)
			continue
		}

		// Parse modules
		if strings.HasPrefix(line, "module") {
			i = vl.parseModule(vf, lines, i, lineNum)
			continue
		}
	}
}

// parseImport parses an import statement
func (vl *VeldLanguage) parseImport(vf *VeldFile, line string, lineNum int) {
	// import "./models/user.veld"
	re := regexp.MustCompile(`import\s+"([^"]+)"`)
	matches := re.FindStringSubmatch(line)
	if len(matches) > 1 {
		importPath := matches[1]
		vf.Imports[filepath.Base(importPath)] = importPath
	}
}

// parseModel parses a model definition
func (vl *VeldLanguage) parseModel(vf *VeldFile, lines []string, startIdx int, lineNum int) int {
	line := strings.TrimSpace(lines[startIdx])

	// model User extends Base {
	re := regexp.MustCompile(`model\s+([A-Za-z_]\w*)(?:\s+extends\s+([A-Za-z_]\w*))?`)
	matches := re.FindStringSubmatch(line)
	if len(matches) < 2 {
		vf.Errors = append(vf.Errors, VeldError{
			File:    vf.Path,
			Line:    lineNum,
			Message: "Invalid model definition",
			Code:    "PARSE_ERROR",
		})
		return startIdx
	}

	model := &Model{
		Name:   matches[1],
		Line:   lineNum,
		Fields: make(map[string]Field),
	}
	if len(matches) > 2 {
		model.Extends = matches[2]
	}

	// Parse fields until closing brace
	i := startIdx + 1
	for i < len(lines) {
		fieldLine := strings.TrimSpace(lines[i])

		if fieldLine == "}" {
			vf.Models[model.Name] = model
			return i
		}

		if fieldLine == "" || strings.HasPrefix(fieldLine, "//") {
			i++
			continue
		}

		// Parse field: name: type
		fieldRe := regexp.MustCompile(`^([a-z_]\w*):\s*(.+?)(?:\s*\/\/.*)?$`)
		fieldMatches := fieldRe.FindStringSubmatch(fieldLine)
		if len(fieldMatches) > 2 {
			fieldName := fieldMatches[1]
			fieldType := strings.TrimSpace(fieldMatches[2])

			// Check for optional (?)
			optional := strings.HasSuffix(fieldType, "?")
			if optional {
				fieldType = strings.TrimSuffix(fieldType, "?")
			}

			model.Fields[fieldName] = Field{
				Name:     fieldName,
				Type:     fieldType,
				Optional: optional,
			}

			// Validate type
			vl.validateType(vf, fieldType, i+1)
		}

		i++
	}

	return i
}

// parseEnum parses an enum definition
func (vl *VeldLanguage) parseEnum(vf *VeldFile, lines []string, startIdx int, lineNum int) int {
	line := strings.TrimSpace(lines[startIdx])

	// enum Status {
	re := regexp.MustCompile(`enum\s+([A-Za-z_]\w*)`)
	matches := re.FindStringSubmatch(line)
	if len(matches) < 2 {
		return startIdx
	}

	enum := &Enum{
		Name:   matches[1],
		Line:   lineNum,
		Values: []string{},
	}

	i := startIdx + 1
	for i < len(lines) {
		enumLine := strings.TrimSpace(lines[i])

		if enumLine == "}" {
			vf.Enums[enum.Name] = enum
			return i
		}

		if enumLine == "" || strings.HasPrefix(enumLine, "//") {
			i++
			continue
		}

		// Parse enum value
		if match := regexp.MustCompile(`^([a-z_]\w*)`).FindStringSubmatch(enumLine); len(match) > 1 {
			enum.Values = append(enum.Values, match[1])
		}

		i++
	}

	return i
}

// parseModule parses a module definition
func (vl *VeldLanguage) parseModule(vf *VeldFile, lines []string, startIdx int, lineNum int) int {
	line := strings.TrimSpace(lines[startIdx])

	// module users {
	re := regexp.MustCompile(`module\s+([A-Za-z_]\w*)`)
	matches := re.FindStringSubmatch(line)
	if len(matches) < 2 {
		return startIdx
	}

	module := &Module{
		Name:    matches[1],
		Line:    lineNum,
		Actions: make(map[string]*Action),
	}

	i := startIdx + 1
	for i < len(lines) {
		modLine := strings.TrimSpace(lines[i])

		if modLine == "}" {
			vf.Modules[module.Name] = module
			return i
		}

		if modLine == "" || strings.HasPrefix(modLine, "//") {
			i++
			continue
		}

		// Parse module directives
		if strings.HasPrefix(modLine, "description:") {
			module.Description = strings.TrimPrefix(modLine, "description:")
			module.Description = strings.TrimSpace(module.Description)
			i++
			continue
		}

		if strings.HasPrefix(modLine, "prefix:") {
			module.Prefix = strings.TrimPrefix(modLine, "prefix:")
			module.Prefix = strings.TrimSpace(module.Prefix)
			i++
			continue
		}

		// Parse action
		if strings.HasPrefix(modLine, "action") {
			actionRe := regexp.MustCompile(`action\s+([A-Za-z_]\w*)`)
			actionMatches := actionRe.FindStringSubmatch(modLine)
			if len(actionMatches) > 1 {
				action := &Action{
					Name: actionMatches[1],
					Line: i + 1,
				}

				// Parse action properties
				i++
				for i < len(lines) {
					actionLine := strings.TrimSpace(lines[i])

					if actionLine == "}" {
						module.Actions[action.Name] = action
						break
					}

					if actionLine == "" || strings.HasPrefix(actionLine, "//") {
						i++
						continue
					}

					// Parse action directives
					if strings.HasPrefix(actionLine, "method:") {
						action.Method = strings.TrimPrefix(actionLine, "method:")
						action.Method = strings.TrimSpace(action.Method)
						vl.validateHttpMethod(vf, action.Method, i+1)
					} else if strings.HasPrefix(actionLine, "path:") {
						action.Path = strings.TrimPrefix(actionLine, "path:")
						action.Path = strings.TrimSpace(action.Path)
					} else if strings.HasPrefix(actionLine, "input:") {
						action.Input = strings.TrimPrefix(actionLine, "input:")
						action.Input = strings.TrimSpace(action.Input)
						vl.validateType(vf, action.Input, i+1)
					} else if strings.HasPrefix(actionLine, "output:") {
						action.Output = strings.TrimPrefix(actionLine, "output:")
						action.Output = strings.TrimSpace(action.Output)
						vl.validateType(vf, action.Output, i+1)
					}

					i++
				}
			}
		}

		i++
	}

	return i
}

// validateType checks if a type is valid
func (vl *VeldLanguage) validateType(vf *VeldFile, typeStr string, lineNum int) {
	// Remove List<> or Map<>
	baseType := typeStr
	if strings.HasPrefix(baseType, "List<") && strings.HasSuffix(baseType, ">") {
		baseType = baseType[5 : len(baseType)-1]
	} else if strings.HasPrefix(baseType, "Map<") && strings.HasSuffix(baseType, ">") {
		// Map<string, int> - get first type
		parts := strings.Split(baseType[4:len(baseType)-1], ",")
		if len(parts) > 0 {
			baseType = strings.TrimSpace(parts[0])
		}
	}

	// Check if it's a valid type
	spec := GetLanguageSpec()
	isValid := false
	for _, t := range spec.BuiltinTypes {
		if t == baseType {
			isValid = true
			break
		}
	}
	if !isValid {
		_, hasModel := vf.Models[baseType]
		_, hasEnum := vf.Enums[baseType]
		if !hasModel && !hasEnum {
			vf.Errors = append(vf.Errors, VeldError{
				File:    vf.Path,
				Line:    lineNum,
				Message: fmt.Sprintf("Type '%s' is not defined", baseType),
				Code:    "ERROR_UNDEFINED_TYPE",
			})
		}
	}
}

// validateHttpMethod checks if HTTP method is valid
func (vl *VeldLanguage) validateHttpMethod(vf *VeldFile, method string, lineNum int) {
	if !vl.HttpMethods[method] {
		vf.Errors = append(vf.Errors, VeldError{
			File:    vf.Path,
			Line:    lineNum,
			Message: fmt.Sprintf("Invalid HTTP method '%s'. Valid: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS", method),
			Code:    "ERROR_INVALID_HTTP_METHOD",
		})
	}
}

// ResolveImports loads all imported files
func (vl *VeldLanguage) ResolveImports(mainFile *VeldFile, basePath string) (map[string]*VeldFile, error) {
	files := make(map[string]*VeldFile)
	files[mainFile.Path] = mainFile

	for importName, importPath := range mainFile.Imports {
		fullPath := filepath.Join(basePath, importPath)
		importedFile, err := vl.ParseFile(fullPath)
		if err != nil {
			return nil, err
		}
		files[importName] = importedFile
	}

	return files, nil
}

// GetAllSymbols returns all symbols across all files
func GetAllSymbols(files map[string]*VeldFile) (models map[string]*Model, enums map[string]*Enum, modules map[string]*Module) {
	models = make(map[string]*Model)
	enums = make(map[string]*Enum)
	modules = make(map[string]*Module)

	for _, file := range files {
		for k, v := range file.Models {
			models[k] = v
		}
		for k, v := range file.Enums {
			enums[k] = v
		}
		for k, v := range file.Modules {
			modules[k] = v
		}
	}

	return
}

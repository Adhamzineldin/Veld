package ast

// AST is the root of a parsed .veld contract.
type AST struct {
	ASTVersion string   `json:"astVersion"`
	Imports    []string `json:"-"` // resolved by the file loader, not serialised
	Models     []Model  `json:"models"`
	Modules    []Module `json:"modules"`
}

// Model is a named data type with typed fields.
type Model struct {
	Name       string  `json:"name"`
	Fields     []Field `json:"fields"`
	SourceFile string  `json:"-"` // absolute path; set by file loader, not serialised
}

// Field is a single property of a Model.
type Field struct {
	Name string `json:"name"`
	Type string `json:"type"`
}

// Module groups related Actions.
type Module struct {
	Name       string   `json:"name"`
	Actions    []Action `json:"actions"`
	SourceFile string   `json:"-"` // absolute path; set by file loader, not serialised
}

// Action is a single API endpoint inside a Module.
type Action struct {
	Name       string   `json:"name"`
	Method     string   `json:"method"`
	Path       string   `json:"path"`
	Input      string   `json:"input"`
	Output     string   `json:"output"`
	Middleware []string `json:"middleware"`
}

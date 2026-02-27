package ast

// AST is the root of a parsed .veld contract.
type AST struct {
	ASTVersion string   `json:"astVersion"`
	Imports    []string `json:"-"` // resolved by the file loader, not serialised
	Models     []Model  `json:"models"`
	Modules    []Module `json:"modules"`
	Enums      []Enum   `json:"enums,omitempty"`
}

// Enum is a named set of string constants.
type Enum struct {
	Name        string   `json:"name"`
	Description string   `json:"description,omitempty"`
	Values      []string `json:"values"`
	SourceFile  string   `json:"-"`
}

// Model is a named data type with typed fields.
type Model struct {
	Name        string  `json:"name"`
	Description string  `json:"description,omitempty"`
	Fields      []Field `json:"fields"`
	SourceFile  string  `json:"-"` // absolute path; set by file loader, not serialised
}

// Field is a single property of a Model.
type Field struct {
	Name     string `json:"name"`
	Type     string `json:"type"`
	Optional bool   `json:"optional,omitempty"`
	IsArray  bool   `json:"isArray,omitempty"`
	Default  string `json:"default,omitempty"` // @default(value)
}

// Module groups related Actions.
type Module struct {
	Name        string   `json:"name"`
	Description string   `json:"description,omitempty"`
	Prefix      string   `json:"prefix,omitempty"` // route path prefix
	Actions     []Action `json:"actions"`
	SourceFile  string   `json:"-"` // absolute path; set by file loader, not serialised
}

// Action is a single API endpoint inside a Module.
type Action struct {
	Name        string   `json:"name"`
	Description string   `json:"description,omitempty"`
	Method      string   `json:"method"`
	Path        string   `json:"path"`
	Input       string   `json:"input"`
	Output      string   `json:"output"`
	OutputArray bool     `json:"outputArray,omitempty"` // output User[] → true
	Query       string   `json:"query,omitempty"`       // query param model
	Middleware  []string `json:"middleware"`
}

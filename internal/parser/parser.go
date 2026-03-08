package parser

import (
	"fmt"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/lexer"
)

// Parser builds an AST from a token slice produced by the Lexer.
type Parser struct {
	tokens []lexer.Token
	pos    int
}

// New creates a Parser over the given token slice.
func New(tokens []lexer.Token) *Parser {
	return &Parser{tokens: tokens}
}

// Parse runs the recursive descent parser and returns the AST.
func (p *Parser) Parse() (ast.AST, error) {
	result := ast.AST{ASTVersion: "1.0.0"}

	for p.peek().Type != lexer.TEOF {
		switch p.peek().Type {
		case lexer.TImport:
			imp, err := p.parseImport()
			if err != nil {
				return result, err
			}
			result.Imports = append(result.Imports, imp)
		case lexer.TFrom:
			imp, err := p.parseFromImport()
			if err != nil {
				return result, err
			}
			result.Imports = append(result.Imports, imp)
		case lexer.TModel:
			m, err := p.parseModel()
			if err != nil {
				return result, err
			}
			result.Models = append(result.Models, m)
		case lexer.TModule:
			mod, err := p.parseModule()
			if err != nil {
				return result, err
			}
			result.Modules = append(result.Modules, mod)
		case lexer.TEnum:
			en, err := p.parseEnum()
			if err != nil {
				return result, err
			}
			result.Enums = append(result.Enums, en)
		case lexer.TIdent:
			// Contextual keyword: top-level "prefix: /api/v1"
			if p.peekIdent("prefix") {
				p.consume()
				if _, err := p.expect(lexer.TColon); err != nil {
					return result, fmt.Errorf("prefix: %w", err)
				}
				pathTok, err := p.expect(lexer.TPath)
				if err != nil {
					return result, fmt.Errorf("prefix path: %w", err)
				}
				result.Prefix = pathTok.Value
			} else {
				tok := p.peek()
				return result, fmt.Errorf("line %d: unexpected token %q", tok.Line, tok.Value)
			}
		default:
			tok := p.peek()
			return result, fmt.Errorf("line %d: unexpected token %q", tok.Line, tok.Value)
		}
	}

	return result, nil
}

// --- helpers ---

func (p *Parser) peek() lexer.Token {
	if p.pos >= len(p.tokens) {
		return lexer.Token{Type: lexer.TEOF}
	}
	return p.tokens[p.pos]
}

// peekAt returns the token at the given offset from the current position.
func (p *Parser) peekAt(offset int) lexer.Token {
	idx := p.pos + offset
	if idx >= len(p.tokens) {
		return lexer.Token{Type: lexer.TEOF}
	}
	return p.tokens[idx]
}

func (p *Parser) consume() lexer.Token {
	tok := p.peek()
	p.pos++
	return tok
}

func (p *Parser) expect(t lexer.TokenType) (lexer.Token, error) {
	tok := p.consume()
	if tok.Type != t {
		return tok, fmt.Errorf("line %d: expected %s, got %q", tok.Line, t, tok.Value)
	}
	return tok, nil
}

// peekIdent returns true if the next token is TIdent with the given value.
// Used for contextual keywords like "description", "prefix", "method", etc.
func (p *Parser) peekIdent(value string) bool {
	return p.peek().Type == lexer.TIdent && p.peek().Value == value
}

// --- import parsing ---

// resolveImportPath normalises a raw folder + suffix into the canonical
// loader format: "@folder/*" (wildcard) or "@folder/name.veld" (single file).
func resolveImportPath(folder, suffix string) string {
	if suffix == "*" || suffix == "" {
		return "@" + folder + "/*"
	}
	return "@" + folder + "/" + suffix + ".veld"
}

// parseBarePath handles a TPath like "/models/*" or "/models/user" and returns
// the loader-canonical import string.
func parseBarePath(raw string) string {
	raw = raw[1:] // strip leading /
	slashIdx := -1
	for i, c := range raw {
		if c == '/' {
			slashIdx = i
			break
		}
	}
	if slashIdx < 0 {
		// Single segment: /models → wildcard
		return "@" + raw + "/*"
	}
	return resolveImportPath(raw[:slashIdx], raw[slashIdx+1:])
}

// parseImport handles:
//
//	import @models/*           →  @models/*
//	import @models/user        →  @models/user.veld
//	import models/*            →  @models/*
//	import models/user         →  @models/user.veld
//	import /models/*           →  @models/*
//	import /models/user        →  @models/user.veld
//	import /models             →  @models/*
//	import "models/user.veld"  →  models/user.veld  (legacy)
func (p *Parser) parseImport() (string, error) {
	p.consume() // consume 'import'

	switch p.peek().Type {
	case lexer.TAt:
		// import @alias/...
		p.consume() // @
		aliasTok, err := p.expect(lexer.TIdent)
		if err != nil {
			return "", fmt.Errorf("import alias: %w", err)
		}
		if p.peek().Type == lexer.TPath {
			pathTok := p.consume()
			return resolveImportPath(aliasTok.Value, pathTok.Value[1:]), nil
		}
		// import @models  (no path) → wildcard
		return "@" + aliasTok.Value + "/*", nil

	case lexer.TPath:
		// import /models/* or /models/user or /models
		pathTok := p.consume()
		return parseBarePath(pathTok.Value), nil

	case lexer.TIdent:
		// import models/user or import models (no path after)
		aliasTok := p.consume()
		if p.peek().Type == lexer.TPath {
			pathTok := p.consume()
			return resolveImportPath(aliasTok.Value, pathTok.Value[1:]), nil
		}
		// import models → wildcard
		return "@" + aliasTok.Value + "/*", nil

	case lexer.TString:
		// import "models/user.veld" (legacy)
		pathTok := p.consume()
		return pathTok.Value, nil

	default:
		tok := p.peek()
		return "", fmt.Errorf("line %d: expected import path, got %q", tok.Line, tok.Value)
	}
}

// parseFromImport handles:
//
//	from @models import *        →  @models/*
//	from @models import user     →  @models/user.veld
//	from /models import *        →  @models/*
//	from /models import user     →  @models/user.veld
//	from models import *         →  @models/*
//	from models import user      →  @models/user.veld
func (p *Parser) parseFromImport() (string, error) {
	p.consume() // consume 'from'

	var folder string
	switch p.peek().Type {
	case lexer.TAt:
		p.consume() // @
		aliasTok, err := p.expect(lexer.TIdent)
		if err != nil {
			return "", fmt.Errorf("from alias: %w", err)
		}
		folder = aliasTok.Value
	case lexer.TPath:
		pathTok := p.consume()
		folder = pathTok.Value[1:] // strip leading /
	case lexer.TIdent:
		aliasTok := p.consume()
		folder = aliasTok.Value
	default:
		tok := p.peek()
		return "", fmt.Errorf("line %d: expected folder path after 'from', got %q", tok.Line, tok.Value)
	}

	// Expect 'import'
	if _, err := p.expect(lexer.TImport); err != nil {
		return "", fmt.Errorf("from import: %w", err)
	}

	// Expect * or identifier
	switch p.peek().Type {
	case lexer.TStar:
		p.consume()
		return "@" + folder + "/*", nil
	case lexer.TIdent:
		nameTok := p.consume()
		return "@" + folder + "/" + nameTok.Value + ".veld", nil
	default:
		tok := p.peek()
		return "", fmt.Errorf("line %d: expected '*' or name after 'import', got %q", tok.Line, tok.Value)
	}
}

// --- grammar rules ---

func (p *Parser) parseEnum() (ast.Enum, error) {
	startTok := p.consume() // 'enum'
	nameTok, err := p.expect(lexer.TIdent)
	if err != nil {
		return ast.Enum{}, fmt.Errorf("enum name: %w", err)
	}
	if _, err := p.expect(lexer.TLBrace); err != nil {
		return ast.Enum{}, err
	}

	en := ast.Enum{Name: nameTok.Value, Line: startTok.Line}

	// optional description: "..."
	if p.peekIdent("description") && p.peekAt(1).Type == lexer.TColon && p.peekAt(2).Type == lexer.TString {
		p.consume()            // description
		p.consume()            // :
		descTok := p.consume() // "..."
		en.Description = descTok.Value
	}

	for p.peek().Type != lexer.TRBrace && p.peek().Type != lexer.TEOF {
		valTok, err := p.expect(lexer.TIdent)
		if err != nil {
			return en, fmt.Errorf("enum value: %w", err)
		}
		en.Values = append(en.Values, valTok.Value)
	}

	if _, err := p.expect(lexer.TRBrace); err != nil {
		return en, err
	}
	return en, nil
}

func (p *Parser) parseModel() (ast.Model, error) {
	startTok := p.consume() // 'model'
	nameTok, err := p.expect(lexer.TIdent)
	if err != nil {
		return ast.Model{}, fmt.Errorf("model name: %w", err)
	}

	m := ast.Model{Name: nameTok.Value, Line: startTok.Line}

	// optional: model Child extends Parent {
	if p.peek().Type == lexer.TIdent && p.peek().Value == "extends" {
		p.consume() // 'extends'
		parentTok, err := p.expect(lexer.TIdent)
		if err != nil {
			return ast.Model{}, fmt.Errorf("model extends: %w", err)
		}
		m.Extends = parentTok.Value
	}

	if _, err := p.expect(lexer.TLBrace); err != nil {
		return ast.Model{}, err
	}

	// optional description: "..."
	// Disambiguate from a field named "description" by checking if token after ':' is a string literal.
	if p.peekIdent("description") && p.peekAt(1).Type == lexer.TColon && p.peekAt(2).Type == lexer.TString {
		p.consume()            // description
		p.consume()            // :
		descTok := p.consume() // "..."
		m.Description = descTok.Value
	}

	for p.peek().Type != lexer.TRBrace && p.peek().Type != lexer.TEOF {
		f, err := p.parseField()
		if err != nil {
			return m, err
		}
		m.Fields = append(m.Fields, f)
	}

	if _, err := p.expect(lexer.TRBrace); err != nil {
		return m, err
	}
	return m, nil
}

func (p *Parser) parseField() (ast.Field, error) {
	nameTok, err := p.expect(lexer.TIdent)
	if err != nil {
		return ast.Field{}, fmt.Errorf("field name: %w", err)
	}

	// Check for optional marker: name? : type  OR  name?: type
	optional := false
	if p.peek().Type == lexer.TQuestion {
		p.consume()
		optional = true
	}

	if _, err := p.expect(lexer.TColon); err != nil {
		return ast.Field{}, err
	}

	typeTok := p.consume()
	isValidType := isTypeToken(typeTok.Type) || typeTok.Type == lexer.TIdent
	if !isValidType {
		return ast.Field{}, fmt.Errorf("line %d: expected type (string, int, float, bool, date, datetime, uuid, or model name), got %q", typeTok.Line, typeTok.Value)
	}

	typeName := typeTok.Value
	isArray := false
	isMap := false
	mapValueType := ""

	// List<T> syntax — equivalent to T[] (isArray = true)
	if typeName == "List" && p.peek().Type == lexer.TLAngle {
		p.consume() // <
		elemTok := p.consume()
		if !isTypeToken(elemTok.Type) && elemTok.Type != lexer.TIdent {
			return ast.Field{}, fmt.Errorf("line %d: expected element type in List<T>, got %q", elemTok.Line, elemTok.Value)
		}
		if _, err := p.expect(lexer.TRAngle); err != nil {
			return ast.Field{}, err
		}
		isArray = true
		typeName = elemTok.Value
	}

	// Map<string, ValueType> syntax
	if typeName == "Map" && p.peek().Type == lexer.TLAngle {
		p.consume() // <
		keyTok := p.consume()
		if keyTok.Value != "string" {
			return ast.Field{}, fmt.Errorf("line %d: Map key type must be \"string\", got %q", keyTok.Line, keyTok.Value)
		}
		if _, err := p.expect(lexer.TComma); err != nil {
			return ast.Field{}, fmt.Errorf("line %d: expected ',' in Map<string, V>", keyTok.Line)
		}
		valTok := p.consume()
		if !isTypeToken(valTok.Type) && valTok.Type != lexer.TIdent {
			return ast.Field{}, fmt.Errorf("line %d: expected value type in Map<string, V>, got %q", valTok.Line, valTok.Value)
		}
		if _, err := p.expect(lexer.TRAngle); err != nil {
			return ast.Field{}, err
		}
		isMap = true
		mapValueType = valTok.Value
		typeName = "Map"
	}

	// Array suffix: name: string[] or name: User[]
	if p.peek().Type == lexer.TLBracket {
		p.consume() // [
		if _, err := p.expect(lexer.TRBracket); err != nil {
			return ast.Field{}, err
		}
		isArray = true
	}

	// Union type: name: "DRAFT" | "PENDING" | "APPROVED"
	// or name: string | int
	var unionTypes []string
	if p.peek().Type == lexer.TPipe {
		// The first type is already in typeName; collect all alternatives
		unionTypes = append(unionTypes, typeName)
		for p.peek().Type == lexer.TPipe {
			p.consume() // |
			nextTok := p.consume()
			nextValid := isTypeToken(nextTok.Type) || nextTok.Type == lexer.TIdent || nextTok.Type == lexer.TString
			if !nextValid {
				return ast.Field{}, fmt.Errorf("line %d: expected type after '|', got %q", nextTok.Line, nextTok.Value)
			}
			unionTypes = append(unionTypes, nextTok.Value)
		}
	}

	f := ast.Field{
		Name:         nameTok.Value,
		Type:         typeName,
		UnionTypes:   unionTypes,
		Optional:     optional,
		IsArray:      isArray,
		IsMap:        isMap,
		MapValueType: mapValueType,
		Line:         nameTok.Line,
	}

	// Handle field annotations: @default(value), @deprecated "message"
	// Multiple annotations are allowed in any order.
	for p.peek().Type == lexer.TAt {
		p.consume() // @
		kwTok := p.consume()
		switch kwTok.Value {
		case "default":
			if _, err := p.expect(lexer.TLParen); err != nil {
				return f, err
			}
			// The default value can be a string, number, identifier (true/false/enum value)
			valTok := p.consume()
			switch valTok.Type {
			case lexer.TString:
				f.Default = "\"" + valTok.Value + "\""
			case lexer.TNumber:
				f.Default = valTok.Value
			case lexer.TIdent:
				f.Default = valTok.Value
			case lexer.TTypeBool:
				f.Default = valTok.Value // true/false parsed as keyword
			default:
				return f, fmt.Errorf("line %d: expected default value, got %q", valTok.Line, valTok.Value)
			}
			if _, err := p.expect(lexer.TRParen); err != nil {
				return f, err
			}
		case "deprecated":
			msgTok, err := p.expect(lexer.TString)
			if err != nil {
				return f, fmt.Errorf("line %d: @deprecated expects a quoted message, e.g. @deprecated \"use newField instead\"", kwTok.Line)
			}
			f.Deprecated = msgTok.Value
		case "example":
			if _, err := p.expect(lexer.TLParen); err != nil {
				return f, err
			}
			valTok := p.consume()
			switch valTok.Type {
			case lexer.TString:
				f.Example = valTok.Value
			case lexer.TNumber:
				f.Example = valTok.Value
			case lexer.TIdent, lexer.TTypeBool:
				f.Example = valTok.Value
			default:
				return f, fmt.Errorf("line %d: expected example value, got %q", valTok.Line, valTok.Value)
			}
			if _, err := p.expect(lexer.TRParen); err != nil {
				return f, err
			}
		case "unique":
			f.Unique = true
		case "index":
			f.Index = true
		case "relation":
			if _, err := p.expect(lexer.TLParen); err != nil {
				return f, err
			}
			relTok, err := p.expect(lexer.TIdent)
			if err != nil {
				return f, fmt.Errorf("line %d: @relation expects a model name", kwTok.Line)
			}
			f.Relation = relTok.Value
			if _, err := p.expect(lexer.TRParen); err != nil {
				return f, err
			}
		case "serverSet":
			f.ServerSet = true
		default:
			return f, fmt.Errorf("line %d: unknown field annotation @%s", kwTok.Line, kwTok.Value)
		}
	}

	return f, nil
}

func (p *Parser) parseModule() (ast.Module, error) {
	startTok := p.consume() // 'module'
	nameTok, err := p.expect(lexer.TIdent)
	if err != nil {
		return ast.Module{}, fmt.Errorf("module name: %w", err)
	}
	if _, err := p.expect(lexer.TLBrace); err != nil {
		return ast.Module{}, err
	}

	mod := ast.Module{Name: nameTok.Value, Line: startTok.Line}

	// optional description: "..."
	if p.peekIdent("description") && p.peekAt(1).Type == lexer.TColon && p.peekAt(2).Type == lexer.TString {
		p.consume()            // description
		p.consume()            // :
		descTok := p.consume() // "..."
		mod.Description = descTok.Value
	}

	// optional prefix: /path
	if p.peekIdent("prefix") {
		p.consume()
		if _, err := p.expect(lexer.TColon); err != nil {
			return mod, err
		}
		prefixTok, err := p.expect(lexer.TPath)
		if err != nil {
			return mod, fmt.Errorf("module prefix: %w", err)
		}
		mod.Prefix = prefixTok.Value
	}

	for p.peek().Type != lexer.TRBrace && p.peek().Type != lexer.TEOF {
		act, err := p.parseAction()
		if err != nil {
			return mod, err
		}
		mod.Actions = append(mod.Actions, act)
	}

	if _, err := p.expect(lexer.TRBrace); err != nil {
		return mod, err
	}
	return mod, nil
}

func (p *Parser) parseAction() (ast.Action, error) {
	startTok, err := p.expect(lexer.TAction)
	if err != nil {
		return ast.Action{}, err
	}
	nameTok, err := p.expect(lexer.TIdent)
	if err != nil {
		return ast.Action{}, fmt.Errorf("action name: %w", err)
	}
	if _, err := p.expect(lexer.TLBrace); err != nil {
		return ast.Action{}, err
	}

	act := ast.Action{
		Name:       nameTok.Value,
		Middleware: []string{},
		Line:       startTok.Line,
	}

	for p.peek().Type != lexer.TRBrace && p.peek().Type != lexer.TEOF {
		switch {
		case p.peekIdent("method"):
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			methodTok := p.consume()
			if !isHTTPMethod(methodTok.Type) && methodTok.Type != lexer.TWS {
				return act, fmt.Errorf("line %d: expected HTTP method (GET, POST, PUT, DELETE, PATCH, WS), got %q", methodTok.Line, methodTok.Value)
			}
			act.Method = methodTok.Value
		case p.peekIdent("path"):
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			pathTok, err := p.expect(lexer.TPath)
			if err != nil {
				return act, fmt.Errorf("action path: %w", err)
			}
			act.Path = pathTok.Value
		case p.peekIdent("description"):
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			descTok, err := p.expect(lexer.TString)
			if err != nil {
				return act, err
			}
			act.Description = descTok.Value
		case p.peekIdent("input"):
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			tok, err := p.expect(lexer.TIdent)
			if err != nil {
				return act, err
			}
			act.Input = tok.Value
		case p.peekIdent("output"):
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			tok, err := p.expectTypeOrIdent()
			if err != nil {
				return act, err
			}
			act.Output = tok.Value
			// Check for array suffix: output: User[]
			if p.peek().Type == lexer.TLBracket {
				p.consume()
				if _, err := p.expect(lexer.TRBracket); err != nil {
					return act, err
				}
				act.OutputArray = true
			}
		case p.peekIdent("query"):
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			tok, err := p.expect(lexer.TIdent)
			if err != nil {
				return act, err
			}
			act.Query = tok.Value
		case p.peekIdent("stream"):
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			tok, err := p.expect(lexer.TIdent)
			if err != nil {
				return act, err
			}
			act.Stream = tok.Value
		case p.peekIdent("middleware"):
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			if p.peek().Type == lexer.TLBracket {
				// Bracket list: middleware: [Guard1, Guard2]
				p.consume() // consume '['
				for p.peek().Type != lexer.TRBracket && p.peek().Type != lexer.TEOF {
					tok, err := p.expect(lexer.TIdent)
					if err != nil {
						return act, fmt.Errorf("middleware name: %w", err)
					}
					act.Middleware = append(act.Middleware, tok.Value)
					if p.peek().Type == lexer.TComma {
						p.consume()
					}
				}
				if _, err := p.expect(lexer.TRBracket); err != nil {
					return act, fmt.Errorf("middleware list: %w", err)
				}
			} else {
				// Single value: middleware: Guard
				tok, err := p.expect(lexer.TIdent)
				if err != nil {
					return act, err
				}
				act.Middleware = append(act.Middleware, tok.Value)
			}
		case p.peekIdent("errors"):
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			if _, err := p.expect(lexer.TLBracket); err != nil {
				return act, fmt.Errorf("errors list: %w", err)
			}
			for p.peek().Type != lexer.TRBracket && p.peek().Type != lexer.TEOF {
				nameTok, err := p.expect(lexer.TIdent)
				if err != nil {
					return act, fmt.Errorf("error name: %w", err)
				}
				act.Errors = append(act.Errors, nameTok.Value)
				if p.peek().Type == lexer.TComma {
					p.consume()
				}
			}
			if _, err := p.expect(lexer.TRBracket); err != nil {
				return act, fmt.Errorf("errors list: %w", err)
			}
		case p.peek().Type == lexer.TAt:
			p.consume() // @
			kwTok := p.consume()
			switch kwTok.Value {
			case "deprecated":
				msgTok, err := p.expect(lexer.TString)
				if err != nil {
					return act, fmt.Errorf("line %d: @deprecated expects a quoted message, e.g. @deprecated \"use newAction instead\"", kwTok.Line)
				}
				act.Deprecated = msgTok.Value
			default:
				return act, fmt.Errorf("line %d: unknown action annotation @%s", kwTok.Line, kwTok.Value)
			}
		default:
			tok := p.peek()
			return act, fmt.Errorf("line %d: unexpected token %q in action body", tok.Line, tok.Value)
		}
	}

	if _, err := p.expect(lexer.TRBrace); err != nil {
		return act, err
	}

	// Validate that required fields were provided.
	if act.Method == "" {
		return act, fmt.Errorf("action %q: missing required field \"method\"", act.Name)
	}
	if act.Path == "" {
		return act, fmt.Errorf("action %q: missing required field \"path\"", act.Name)
	}

	return act, nil
}

// expectTypeOrIdent consumes a token that is either a type keyword or an identifier.
func (p *Parser) expectTypeOrIdent() (lexer.Token, error) {
	tok := p.consume()
	if isTypeToken(tok.Type) || tok.Type == lexer.TIdent {
		return tok, nil
	}
	return tok, fmt.Errorf("line %d: expected type or identifier, got %q", tok.Line, tok.Value)
}

func isHTTPMethod(t lexer.TokenType) bool {
	return t == lexer.TGET || t == lexer.TPOST || t == lexer.TPUT ||
		t == lexer.TDELETE || t == lexer.TPATCH
}

func isTypeToken(t lexer.TokenType) bool {
	return t == lexer.TTypeString || t == lexer.TTypeInt || t == lexer.TTypeFloat ||
		t == lexer.TTypeBool || t == lexer.TTypeDate || t == lexer.TTypeDatetime ||
		t == lexer.TTypeUUID
}

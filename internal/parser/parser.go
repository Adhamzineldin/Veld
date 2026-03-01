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
			p.consume()
			if p.peek().Type == lexer.TAt {
				// @alias/name  or  @alias/*  format
				p.consume() // consume @
				aliasTok, err := p.expect(lexer.TIdent)
				if err != nil {
					return result, fmt.Errorf("import alias: %w", err)
				}
				pathTok, err := p.expect(lexer.TPath)
				if err != nil {
					return result, fmt.Errorf("import path: %w", err)
				}
				// pathTok.Value starts with "/" — strip the leading slash.
				// Keep the @ prefix so the loader can distinguish alias-based
				// imports (resolved from project root) from relative imports.
				suffix := pathTok.Value[1:]
				if suffix == "*" {
					// Wildcard folder import: @alias/* — loader will glob the directory
					result.Imports = append(result.Imports, "@"+aliasTok.Value+"/*")
				} else {
					// Single file: @alias/name — loader resolves from project root
					result.Imports = append(result.Imports, "@"+aliasTok.Value+"/"+suffix+".veld")
				}
			} else if p.peek().Type == lexer.TIdent &&
				p.pos+1 < len(p.tokens) && p.tokens[p.pos+1].Type == lexer.TPath {
				// Root-relative import without @: models/auth → same as @models/auth
				// Tokens: TIdent("models") + TPath("/auth")
				aliasTok := p.consume()
				pathTok := p.consume() // TPath guaranteed by lookahead
				suffix := pathTok.Value[1:]
				if suffix == "*" {
					result.Imports = append(result.Imports, "@"+aliasTok.Value+"/*")
				} else {
					result.Imports = append(result.Imports, "@"+aliasTok.Value+"/"+suffix+".veld")
				}
			} else {
				// Legacy quoted string format: import "models/auth.veld"
				pathTok, err := p.expect(lexer.TString)
				if err != nil {
					return result, fmt.Errorf("import path: %w", err)
				}
				result.Imports = append(result.Imports, pathTok.Value)
			}
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
	if p.peek().Type == lexer.TDescription {
		p.consume()
		if _, err := p.expect(lexer.TColon); err != nil {
			return en, err
		}
		descTok, err := p.expect(lexer.TString)
		if err != nil {
			return en, fmt.Errorf("enum description: %w", err)
		}
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
	if p.peek().Type == lexer.TDescription {
		p.consume()
		if _, err := p.expect(lexer.TColon); err != nil {
			return m, err
		}
		descTok, err := p.expect(lexer.TString)
		if err != nil {
			return m, fmt.Errorf("model description: %w", err)
		}
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

	f := ast.Field{
		Name:         nameTok.Value,
		Type:         typeName,
		Optional:     optional,
		IsArray:      isArray,
		IsMap:        isMap,
		MapValueType: mapValueType,
		Line:         nameTok.Line,
	}

	// Check for @default(value)
	if p.peek().Type == lexer.TAt {
		p.consume() // @
		kwTok := p.consume()
		if kwTok.Value != "default" {
			return f, fmt.Errorf("line %d: expected \"default\" after @, got %q", kwTok.Line, kwTok.Value)
		}
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
	if p.peek().Type == lexer.TDescription {
		p.consume()
		if _, err := p.expect(lexer.TColon); err != nil {
			return mod, err
		}
		descTok, err := p.expect(lexer.TString)
		if err != nil {
			return mod, fmt.Errorf("module description: %w", err)
		}
		mod.Description = descTok.Value
	}

	// optional prefix: /path
	if p.peek().Type == lexer.TPrefix {
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
		switch p.peek().Type {
		case lexer.TMethod:
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			methodTok := p.consume()
			if !isHTTPMethod(methodTok.Type) {
				return act, fmt.Errorf("line %d: expected HTTP method (GET, POST, PUT, DELETE, PATCH), got %q", methodTok.Line, methodTok.Value)
			}
			act.Method = methodTok.Value
		case lexer.TKeyPath:
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			pathTok, err := p.expect(lexer.TPath)
			if err != nil {
				return act, fmt.Errorf("action path: %w", err)
			}
			act.Path = pathTok.Value
		case lexer.TDescription:
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			descTok, err := p.expect(lexer.TString)
			if err != nil {
				return act, err
			}
			act.Description = descTok.Value
		case lexer.TInput:
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			tok, err := p.expect(lexer.TIdent)
			if err != nil {
				return act, err
			}
			act.Input = tok.Value
		case lexer.TOutput:
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
		case lexer.TQuery:
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			tok, err := p.expect(lexer.TIdent)
			if err != nil {
				return act, err
			}
			act.Query = tok.Value
		case lexer.TMiddleware:
			p.consume()
			if _, err := p.expect(lexer.TColon); err != nil {
				return act, err
			}
			tok, err := p.expect(lexer.TIdent)
			if err != nil {
				return act, err
			}
			act.Middleware = append(act.Middleware, tok.Value)
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

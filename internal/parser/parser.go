package parser

import (
	"fmt"

	"github.com/veld-dev/veld/internal/ast"
	"github.com/veld-dev/veld/internal/lexer"
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
			pathTok, err := p.expect(lexer.TString)
			if err != nil {
				return result, fmt.Errorf("import path: %w", err)
			}
			result.Imports = append(result.Imports, pathTok.Value)
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

func (p *Parser) parseModel() (ast.Model, error) {
	p.consume() // 'model'
	nameTok, err := p.expect(lexer.TIdent)
	if err != nil {
		return ast.Model{}, fmt.Errorf("model name: %w", err)
	}
	if _, err := p.expect(lexer.TLBrace); err != nil {
		return ast.Model{}, err
	}

	m := ast.Model{Name: nameTok.Value}
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
	if _, err := p.expect(lexer.TColon); err != nil {
		return ast.Field{}, err
	}
	typeTok := p.consume()
	if typeTok.Type != lexer.TTypeString && typeTok.Type != lexer.TTypeInt && typeTok.Type != lexer.TTypeBool {
		return ast.Field{}, fmt.Errorf("line %d: expected type (string, int, bool), got %q", typeTok.Line, typeTok.Value)
	}
	return ast.Field{Name: nameTok.Value, Type: typeTok.Value}, nil
}

func (p *Parser) parseModule() (ast.Module, error) {
	p.consume() // 'module'
	nameTok, err := p.expect(lexer.TIdent)
	if err != nil {
		return ast.Module{}, fmt.Errorf("module name: %w", err)
	}
	if _, err := p.expect(lexer.TLBrace); err != nil {
		return ast.Module{}, err
	}

	mod := ast.Module{Name: nameTok.Value}
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
	if _, err := p.expect(lexer.TAction); err != nil {
		return ast.Action{}, err
	}
	nameTok, err := p.expect(lexer.TIdent)
	if err != nil {
		return ast.Action{}, fmt.Errorf("action name: %w", err)
	}
	methodTok := p.consume()
	if !isHTTPMethod(methodTok.Type) {
		return ast.Action{}, fmt.Errorf("line %d: expected HTTP method, got %q", methodTok.Line, methodTok.Value)
	}
	pathTok, err := p.expect(lexer.TPath)
	if err != nil {
		return ast.Action{}, fmt.Errorf("action path: %w", err)
	}
	if _, err := p.expect(lexer.TLBrace); err != nil {
		return ast.Action{}, err
	}

	act := ast.Action{
		Name:       nameTok.Value,
		Method:     methodTok.Value,
		Path:       pathTok.Value,
		Middleware: []string{},
	}

	for p.peek().Type != lexer.TRBrace && p.peek().Type != lexer.TEOF {
		switch p.peek().Type {
		case lexer.TInput:
			p.consume()
			tok, err := p.expect(lexer.TIdent)
			if err != nil {
				return act, err
			}
			act.Input = tok.Value
		case lexer.TOutput:
			p.consume()
			tok, err := p.expect(lexer.TIdent)
			if err != nil {
				return act, err
			}
			act.Output = tok.Value
		case lexer.TMiddleware:
			p.consume()
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
	return act, nil
}

func isHTTPMethod(t lexer.TokenType) bool {
	return t == lexer.TGET || t == lexer.TPOST || t == lexer.TPUT ||
		t == lexer.TDELETE || t == lexer.TPATCH
}

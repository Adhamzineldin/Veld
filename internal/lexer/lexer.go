package lexer

import (
	"fmt"
	"unicode"
)

// TokenType identifies what kind of token was scanned.
type TokenType int

const (
	// Keywords
	TModel TokenType = iota
	TModule
	TAction
	TInput
	TOutput
	TMiddleware
	TImport
	TEnum
	TDescription
	TQuery
	TDefault
	TPrefix
	TMethod  // keyword "method"
	TKeyPath // keyword "path" (not the /path token)

	// HTTP methods
	TGET
	TPOST
	TPUT
	TDELETE
	TPATCH
	TWS

	// WebSocket
	TStream // keyword "stream"
	TEmit   // keyword "emit"

	// Error linking
	TErrors // keyword "errors"

	// Import
	TFrom // keyword "from"

	// Primitive types
	TTypeString
	TTypeInt
	TTypeFloat
	TTypeBool
	TTypeDecimal
	TTypeDate
	TTypeDatetime
	TTypeUUID

	// Punctuation
	TLBrace
	TRBrace
	TColon
	TLBracket // [
	TRBracket // ]
	TQuestion // ?
	TAt       // @
	TLParen   // (
	TRParen   // )
	TLAngle   // <
	TRAngle   // >
	TComma    // ,
	TPipe     // |
	TStar     // *

	// Other
	TIdent
	TPath
	TString // quoted string literal, e.g. "models/auth.veld"
	TNumber // numeric literal for @default(123)
	TEOF
)

// String returns the human-readable symbol for a TokenType, used in error messages.
func (t TokenType) String() string {
	switch t {
	case TModel:
		return "\"model\""
	case TModule:
		return "\"module\""
	case TAction:
		return "\"action\""
	case TInput:
		return "\"input\""
	case TOutput:
		return "\"output\""
	case TMiddleware:
		return "\"middleware\""
	case TImport:
		return "\"import\""
	case TEnum:
		return "\"enum\""
	case TDescription:
		return "\"description\""
	case TQuery:
		return "\"query\""
	case TDefault:
		return "\"default\""
	case TPrefix:
		return "\"prefix\""
	case TMethod:
		return "\"method\""
	case TKeyPath:
		return "\"path\""
	case TGET:
		return "\"GET\""
	case TPOST:
		return "\"POST\""
	case TPUT:
		return "\"PUT\""
	case TDELETE:
		return "\"DELETE\""
	case TPATCH:
		return "\"PATCH\""
	case TWS:
		return "\"WS\""
	case TStream:
		return "\"stream\""
	case TErrors:
		return "\"errors\""
	case TFrom:
		return "\"from\""
	case TStar:
		return "\"*\""
	case TTypeString:
		return "\"string\""
	case TTypeInt:
		return "\"int\""
	case TTypeFloat:
		return "\"float\""
	case TTypeDecimal:
		return "\"decimal\""
	case TTypeBool:
		return "\"bool\""
	case TTypeDate:
		return "\"date\""
	case TTypeDatetime:
		return "\"datetime\""
	case TTypeUUID:
		return "\"uuid\""
	case TLBrace:
		return "\"{\""
	case TRBrace:
		return "\"}\""
	case TColon:
		return "\":\""
	case TLBracket:
		return "\"[\""
	case TRBracket:
		return "\"]\""
	case TQuestion:
		return "\"?\""
	case TAt:
		return "\"@\""
	case TLParen:
		return "\"(\""
	case TRParen:
		return "\")\""
	case TLAngle:
		return "\"<\""
	case TRAngle:
		return "\">\""
	case TComma:
		return "\",\""
	case TPipe:
		return "\"|\""
	case TIdent:
		return "identifier"
	case TPath:
		return "path (e.g. /auth/login)"
	case TString:
		return "string literal (e.g. \"models/auth.veld\")"
	case TNumber:
		return "number literal"
	case TEOF:
		return "end of file"
	default:
		return "unknown token"
	}
}

// Token is a single lexical unit.
type Token struct {
	Type  TokenType
	Value string
	Line  int
}

// IsKeyword reports whether t is a reserved keyword token (not a punctuation,
// type primitive, or identifier). Used by the parser to allow keywords as field names.
func IsKeyword(t TokenType) bool {
	switch t {
	case TModel, TModule, TAction, TInput, TOutput, TMiddleware, TImport, TEnum,
		TDescription, TQuery, TDefault, TPrefix, TMethod, TKeyPath,
		TGET, TPOST, TPUT, TDELETE, TPATCH, TWS, TStream, TEmit, TErrors, TFrom:
		return true
	}
	return false
}

// Lexer converts raw .veld source text into a flat token slice.
type Lexer struct {
	source []rune
	pos    int
	line   int
	errors []error // collected errors for recovery mode
}

// New creates a Lexer from the given source string.
func New(source string) *Lexer {
	return &Lexer{source: []rune(source), pos: 0, line: 1}
}

// Tokenize scans the entire source and returns all tokens.
// The lexer recovers from unexpected characters — it collects all errors
// and continues scanning so the caller gets a complete error list.
func (l *Lexer) Tokenize() ([]Token, error) {
	var tokens []Token

	for l.pos < len(l.source) {
		ch := l.source[l.pos]

		if ch == '\n' {
			l.line++
			l.pos++
		} else if ch == '\r' || ch == '\t' || ch == ' ' {
			l.pos++
		} else if ch == '/' && l.pos+1 < len(l.source) && l.source[l.pos+1] == '*' {
			// Block comment /* ... */ — skip everything, track newlines.
			l.pos += 2 // skip /*
			for l.pos < len(l.source) {
				if l.source[l.pos] == '*' && l.pos+1 < len(l.source) && l.source[l.pos+1] == '/' {
					l.pos += 2 // skip */
					break
				}
				if l.source[l.pos] == '\n' {
					l.line++
				}
				l.pos++
			}
		} else if ch == '/' && l.pos+1 < len(l.source) && l.source[l.pos+1] == '/' {
			// Line comment — skip to end of line.
			for l.pos < len(l.source) && l.source[l.pos] != '\n' {
				l.pos++
			}
		} else if ch == '"' {
			// Quoted string literal — used by import statements and descriptions.
			l.pos++ // skip opening quote
			start := l.pos
			for l.pos < len(l.source) && l.source[l.pos] != '"' && l.source[l.pos] != '\n' {
				l.pos++
			}
			value := string(l.source[start:l.pos])
			if l.pos < len(l.source) && l.source[l.pos] == '"' {
				l.pos++ // skip closing quote
			}
			tokens = append(tokens, Token{TString, value, l.line})
		} else if ch == '{' {
			tokens = append(tokens, Token{TLBrace, "{", l.line})
			l.pos++
		} else if ch == '}' {
			tokens = append(tokens, Token{TRBrace, "}", l.line})
			l.pos++
		} else if ch == '[' {
			tokens = append(tokens, Token{TLBracket, "[", l.line})
			l.pos++
		} else if ch == ']' {
			tokens = append(tokens, Token{TRBracket, "]", l.line})
			l.pos++
		} else if ch == '(' {
			tokens = append(tokens, Token{TLParen, "(", l.line})
			l.pos++
		} else if ch == ')' {
			tokens = append(tokens, Token{TRParen, ")", l.line})
			l.pos++
		} else if ch == ':' {
			tokens = append(tokens, Token{TColon, ":", l.line})
			l.pos++
		} else if ch == '?' {
			tokens = append(tokens, Token{TQuestion, "?", l.line})
			l.pos++
		} else if ch == '@' {
			tokens = append(tokens, Token{TAt, "@", l.line})
			l.pos++
		} else if ch == '<' {
			tokens = append(tokens, Token{TLAngle, "<", l.line})
			l.pos++
		} else if ch == '>' {
			tokens = append(tokens, Token{TRAngle, ">", l.line})
			l.pos++
		} else if ch == ',' {
			tokens = append(tokens, Token{TComma, ",", l.line})
			l.pos++
		} else if ch == '|' {
			tokens = append(tokens, Token{TPipe, "|", l.line})
			l.pos++
		} else if ch == '*' {
			tokens = append(tokens, Token{TStar, "*", l.line})
			l.pos++
		} else if ch == '/' {
			// Path token: reads until whitespace or brace.
			start := l.pos
			for l.pos < len(l.source) &&
				!unicode.IsSpace(l.source[l.pos]) &&
				l.source[l.pos] != '{' &&
				l.source[l.pos] != '}' {
				l.pos++
			}
			tokens = append(tokens, Token{TPath, string(l.source[start:l.pos]), l.line})
		} else if unicode.IsDigit(ch) || (ch == '-' && l.pos+1 < len(l.source) && unicode.IsDigit(l.source[l.pos+1])) {
			// Number literal (for @default)
			start := l.pos
			if ch == '-' {
				l.pos++
			}
			for l.pos < len(l.source) && (unicode.IsDigit(l.source[l.pos]) || l.source[l.pos] == '.') {
				l.pos++
			}
			tokens = append(tokens, Token{TNumber, string(l.source[start:l.pos]), l.line})
		} else if unicode.IsLetter(ch) || ch == '_' {
			start := l.pos
			for l.pos < len(l.source) &&
				(unicode.IsLetter(l.source[l.pos]) || unicode.IsDigit(l.source[l.pos]) || l.source[l.pos] == '_') {
				l.pos++
			}
			tokens = append(tokens, classifyWord(string(l.source[start:l.pos]), l.line))
		} else {
			// Error recovery: record the error and skip the bad character.
			l.errors = append(l.errors, fmt.Errorf("line %d: unexpected character %q", l.line, ch))
			l.pos++
		}
	}

	tokens = append(tokens, Token{TEOF, "", l.line})
	if len(l.errors) > 0 {
		return tokens, l.errors[0] // return first error for backward compat, tokens are still usable
	}
	return tokens, nil
}

// Errors returns all lexer errors collected during tokenization.
// When error recovery is active, this may contain multiple entries.
func (l *Lexer) Errors() []error {
	return l.errors
}

func classifyWord(word string, line int) Token {
	switch word {
	case "model":
		return Token{TModel, word, line}
	case "module":
		return Token{TModule, word, line}
	case "action":
		return Token{TAction, word, line}
	case "input":
		return Token{TIdent, word, line} // contextual keyword
	case "output":
		return Token{TIdent, word, line} // contextual keyword
	case "middleware":
		return Token{TIdent, word, line} // contextual keyword
	case "import":
		return Token{TImport, word, line}
	case "enum":
		return Token{TEnum, word, line}
	case "description":
		return Token{TIdent, word, line} // contextual keyword — parsed by value in parser
	case "query":
		return Token{TIdent, word, line} // contextual keyword
	case "default":
		return Token{TIdent, word, line} // contextual keyword
	case "prefix":
		return Token{TIdent, word, line} // contextual keyword
	case "method":
		return Token{TIdent, word, line} // contextual keyword
	case "path":
		return Token{TIdent, word, line} // contextual keyword
	case "extends":
		return Token{TIdent, word, line} // parsed contextually by parser
	case "Map":
		return Token{TIdent, word, line} // parsed contextually by parser
	case "GET":
		return Token{TGET, word, line}
	case "POST":
		return Token{TPOST, word, line}
	case "PUT":
		return Token{TPUT, word, line}
	case "DELETE":
		return Token{TDELETE, word, line}
	case "PATCH":
		return Token{TPATCH, word, line}
	case "WS":
		return Token{TWS, word, line}
	case "stream":
		return Token{TIdent, word, line} // contextual keyword
	case "errors":
		return Token{TIdent, word, line} // contextual keyword
	case "from":
		return Token{TFrom, word, line}
	case "string":
		return Token{TTypeString, word, line}
	case "int":
		return Token{TTypeInt, word, line}
	case "float":
		return Token{TTypeFloat, word, line}
	case "decimal":
		return Token{TTypeDecimal, word, line}
	case "bool":
		return Token{TTypeBool, word, line}
	case "date":
		return Token{TTypeDate, word, line}
	case "datetime":
		return Token{TTypeDatetime, word, line}
	case "uuid":
		return Token{TTypeUUID, word, line}
	default:
		return Token{TIdent, word, line}
	}
}

// Package format provides canonical formatting for .veld contract files.
//
// The formatter reads a .veld file, re-lexes it, and outputs the canonical
// format: consistent 2-space indentation, aligned field types, blank lines
// between blocks, and sorted imports.
package format

import (
	"os"
	"sort"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/lexer"
)

// File formats a single .veld file in-place and returns the formatted output.
// If the file cannot be read or lexed, it returns an error.
func File(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	return Format(string(data))
}

// Format takes raw .veld source text and returns its canonical formatting.
func Format(source string) (string, error) {
	tokens, err := lexer.New(source).Tokenize()
	if err != nil {
		return "", err
	}
	return formatTokens(tokens), nil
}

func formatTokens(tokens []lexer.Token) string {
	var sb strings.Builder
	var imports []string
	indent := 0
	prevType := lexer.TEOF
	inImportBlock := false
	i := 0

	for i < len(tokens) {
		tok := tokens[i]

		if tok.Type == lexer.TEOF {
			break
		}

		// Collect imports
		if tok.Type == lexer.TImport || tok.Type == lexer.TFrom {
			imp := collectImportLine(tokens, &i)
			imports = append(imports, imp)
			inImportBlock = true
			continue
		}

		// Flush sorted imports before first non-import
		if inImportBlock {
			sort.Strings(imports)
			for _, imp := range imports {
				sb.WriteString(imp + "\n")
			}
			sb.WriteString("\n")
			imports = nil
			inImportBlock = false
		}

		switch tok.Type {
		case lexer.TModel, lexer.TModule, lexer.TEnum:
			if prevType != lexer.TEOF && prevType != lexer.TRBrace {
				sb.WriteString("\n")
			}
			line := collectBlock(tokens, &i, indent)
			sb.WriteString(line)
		case lexer.TPrefix:
			sb.WriteString(writeIndent(indent) + "prefix: ")
			i++
			if i < len(tokens) && tokens[i].Type == lexer.TColon {
				i++
			}
			if i < len(tokens) && tokens[i].Type == lexer.TPath {
				sb.WriteString(tokens[i].Value + "\n")
				i++
			}
		default:
			i++
		}
		prevType = tok.Type
	}

	// Flush remaining imports
	if len(imports) > 0 {
		sort.Strings(imports)
		for _, imp := range imports {
			sb.WriteString(imp + "\n")
		}
		sb.WriteString("\n")
	}

	result := sb.String()
	// Ensure single trailing newline
	result = strings.TrimRight(result, "\n") + "\n"
	return result
}

func collectImportLine(tokens []lexer.Token, i *int) string {
	var parts []string
	for *i < len(tokens) {
		tok := tokens[*i]
		if tok.Type == lexer.TEOF {
			break
		}
		parts = append(parts, tok.Value)
		*i++
		// End of import: next token is on a new line or is a keyword
		if *i < len(tokens) {
			next := tokens[*i]
			if next.Type == lexer.TImport || next.Type == lexer.TFrom ||
				next.Type == lexer.TModel || next.Type == lexer.TModule ||
				next.Type == lexer.TEnum || next.Type == lexer.TPrefix ||
				next.Type == lexer.TEOF {
				break
			}
			if next.Line > tok.Line {
				break
			}
		}
	}
	return strings.Join(parts, "")
}

func collectBlock(tokens []lexer.Token, i *int, baseIndent int) string {
	var sb strings.Builder
	depth := 0

	for *i < len(tokens) {
		tok := tokens[*i]
		if tok.Type == lexer.TEOF {
			break
		}

		if tok.Type == lexer.TLBrace {
			sb.WriteString(" {\n")
			depth++
			*i++
			continue
		}

		if tok.Type == lexer.TRBrace {
			depth--
			sb.WriteString(writeIndent(baseIndent+depth) + "}\n")
			*i++
			if depth <= 0 {
				break
			}
			continue
		}

		indent := writeIndent(baseIndent + depth)

		// Handle different line types
		switch tok.Type {
		case lexer.TModel, lexer.TModule, lexer.TEnum:
			sb.WriteString(indent + tok.Value + " ")
			*i++
		case lexer.TAction:
			sb.WriteString("\n" + indent + tok.Value + " ")
			*i++
		case lexer.TDescription, lexer.TPrefix, lexer.TMethod, lexer.TKeyPath,
			lexer.TInput, lexer.TOutput, lexer.TQuery, lexer.TMiddleware,
			lexer.TStream, lexer.TErrors:
			line := indent + collectFieldLine(tokens, i)
			sb.WriteString(line + "\n")
		case lexer.TIdent:
			// Could be "extends", a field name, or enum value
			if tok.Value == "extends" {
				sb.WriteString("extends ")
				*i++
			} else if depth > 0 {
				line := indent + collectFieldLine(tokens, i)
				sb.WriteString(line + "\n")
			} else {
				sb.WriteString(tok.Value + " ")
				*i++
			}
		case lexer.TAt:
			line := indent + collectAnnotation(tokens, i)
			sb.WriteString(line + "\n")
		default:
			sb.WriteString(tok.Value)
			*i++
		}
	}

	return sb.String()
}

func collectFieldLine(tokens []lexer.Token, i *int) string {
	startLine := tokens[*i].Line
	var parts []string
	for *i < len(tokens) {
		tok := tokens[*i]
		if tok.Type == lexer.TEOF || tok.Type == lexer.TRBrace || tok.Type == lexer.TLBrace {
			break
		}
		if tok.Line > startLine && tok.Type != lexer.TAt {
			break
		}
		if tok.Type == lexer.TAt {
			// Annotations on the same line
			parts = append(parts, " "+collectAnnotation(tokens, i))
			continue
		}
		parts = append(parts, tok.Value)
		*i++
	}
	return formatFieldParts(parts)
}

func collectAnnotation(tokens []lexer.Token, i *int) string {
	var parts []string
	parts = append(parts, "@")
	*i++ // consume @
	depth := 0
	for *i < len(tokens) {
		tok := tokens[*i]
		if tok.Type == lexer.TEOF {
			break
		}
		parts = append(parts, tok.Value)
		*i++
		if tok.Type == lexer.TLParen {
			depth++
		}
		if tok.Type == lexer.TRParen {
			depth--
			if depth <= 0 {
				break
			}
		}
		// String after @deprecated
		if tok.Type == lexer.TString && depth == 0 {
			break
		}
		// @unique, @index — no parens
		if tok.Type == lexer.TIdent && depth == 0 {
			next := tokens[*i]
			if next.Type != lexer.TLParen {
				break
			}
		}
	}
	return strings.Join(parts, "")
}

func formatFieldParts(parts []string) string {
	// Join with appropriate spacing
	var result strings.Builder
	for i, p := range parts {
		if i > 0 && p != ":" && p != "?" && p != "[" && p != "]" &&
			p != "<" && p != ">" && p != "," && parts[i-1] != ":" &&
			parts[i-1] != "<" && parts[i-1] != "[" {
			result.WriteString(" ")
		}
		result.WriteString(p)
	}
	return result.String()
}

func writeIndent(level int) string {
	return strings.Repeat("  ", level)
}

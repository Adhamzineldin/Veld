package lsp

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func computeHover(text string, pos Position, a ast.AST) *Hover {
	word := wordAtPosition(text, pos)
	if word == "" {
		return nil
	}

	// Check models
	for _, m := range a.Models {
		if m.Name == word {
			var sb strings.Builder
			sb.WriteString(fmt.Sprintf("**model %s**\n\n", m.Name))
			if m.Description != "" {
				sb.WriteString(m.Description + "\n\n")
			}
			if m.Extends != "" {
				sb.WriteString(fmt.Sprintf("extends `%s`\n\n", m.Extends))
			}
			sb.WriteString("Fields:\n")
			for _, f := range m.Fields {
				opt := ""
				if f.Optional {
					opt = "?"
				}
				sb.WriteString(fmt.Sprintf("- `%s%s`: %s\n", f.Name, opt, f.Type))
			}
			return &Hover{
				Contents: MarkupContent{Kind: "markdown", Value: sb.String()},
			}
		}
	}

	// Check enums
	for _, en := range a.Enums {
		if en.Name == word {
			var sb strings.Builder
			sb.WriteString(fmt.Sprintf("**enum %s**\n\n", en.Name))
			if en.Description != "" {
				sb.WriteString(en.Description + "\n\n")
			}
			sb.WriteString("Values: `" + strings.Join(en.Values, "`, `") + "`")
			return &Hover{
				Contents: MarkupContent{Kind: "markdown", Value: sb.String()},
			}
		}
	}

	// Check modules
	for _, mod := range a.Modules {
		if mod.Name == word {
			var sb strings.Builder
			sb.WriteString(fmt.Sprintf("**module %s**\n\n", mod.Name))
			if mod.Description != "" {
				sb.WriteString(mod.Description + "\n\n")
			}
			if mod.Prefix != "" {
				sb.WriteString(fmt.Sprintf("prefix: `%s`\n\n", mod.Prefix))
			}
			sb.WriteString(fmt.Sprintf("%d action(s)\n", len(mod.Actions)))
			for _, act := range mod.Actions {
				routePath := act.Path
				if mod.Prefix != "" {
					routePath = mod.Prefix + act.Path
				}
				sb.WriteString(fmt.Sprintf("- `%s %s` — %s\n", act.Method, routePath, act.Name))
			}
			return &Hover{
				Contents: MarkupContent{Kind: "markdown", Value: sb.String()},
			}
		}

		// Check actions
		for _, act := range mod.Actions {
			if act.Name == word {
				routePath := act.Path
				if mod.Prefix != "" {
					routePath = mod.Prefix + act.Path
				}
				var sb strings.Builder
				sb.WriteString(fmt.Sprintf("**action %s**\n\n", act.Name))
				if act.Description != "" {
					sb.WriteString(act.Description + "\n\n")
				}
				sb.WriteString(fmt.Sprintf("`%s %s`\n\n", act.Method, routePath))
				if act.Input != "" {
					sb.WriteString(fmt.Sprintf("- input: `%s`\n", act.Input))
				}
				if act.Output != "" {
					out := act.Output
					if act.OutputArray {
						out += "[]"
					}
					sb.WriteString(fmt.Sprintf("- output: `%s`\n", out))
				}
				if act.Stream != "" {
					sb.WriteString(fmt.Sprintf("- stream: `%s`\n", act.Stream))
				}
				return &Hover{
					Contents: MarkupContent{Kind: "markdown", Value: sb.String()},
				}
			}
		}
	}

	return nil
}

func wordAtPosition(text string, pos Position) string {
	lines := strings.Split(text, "\n")
	if pos.Line >= len(lines) {
		return ""
	}
	line := lines[pos.Line]
	if pos.Character >= len(line) {
		return ""
	}

	// Find word boundaries
	start := pos.Character
	for start > 0 && isIdentChar(line[start-1]) {
		start--
	}
	end := pos.Character
	for end < len(line) && isIdentChar(line[end]) {
		end++
	}

	if start == end {
		return ""
	}
	return line[start:end]
}

func isIdentChar(ch byte) bool {
	return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') ||
		(ch >= '0' && ch <= '9') || ch == '_'
}

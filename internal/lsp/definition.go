package lsp

import (
	"github.com/Adhamzineldin/Veld/internal/ast"
)

func computeDefinition(text string, pos Position, a ast.AST, uri string) *Location {
	word := wordAtPosition(text, pos)
	if word == "" {
		return nil
	}

	// Check if word matches a model name — jump to its definition
	for _, m := range a.Models {
		if m.Name == word && m.Line > 0 {
			return &Location{
				URI: uri,
				Range: Range{
					Start: Position{Line: m.Line - 1, Character: 0},
					End:   Position{Line: m.Line - 1, Character: len("model " + m.Name)},
				},
			}
		}
	}

	// Check enums
	for _, en := range a.Enums {
		if en.Name == word && en.Line > 0 {
			return &Location{
				URI: uri,
				Range: Range{
					Start: Position{Line: en.Line - 1, Character: 0},
					End:   Position{Line: en.Line - 1, Character: len("enum " + en.Name)},
				},
			}
		}
	}

	// Check modules
	for _, mod := range a.Modules {
		if mod.Name == word && mod.Line > 0 {
			return &Location{
				URI: uri,
				Range: Range{
					Start: Position{Line: mod.Line - 1, Character: 0},
					End:   Position{Line: mod.Line - 1, Character: len("module " + mod.Name)},
				},
			}
		}

		// Check actions
		for _, act := range mod.Actions {
			if act.Name == word && act.Line > 0 {
				return &Location{
					URI: uri,
					Range: Range{
						Start: Position{Line: act.Line - 1, Character: 0},
						End:   Position{Line: act.Line - 1, Character: len("action " + act.Name)},
					},
				}
			}
		}
	}

	// Enum values — find the containing enum
	for _, en := range a.Enums {
		for _, v := range en.Values {
			if v == word && en.Line > 0 {
				return &Location{
					URI: uri,
					Range: Range{
						Start: Position{Line: en.Line - 1, Character: 0},
						End:   Position{Line: en.Line - 1, Character: len("enum " + en.Name)},
					},
				}
			}
		}
	}

	return nil
}

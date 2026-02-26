package validator

import (
	"fmt"

	"github.com/veld-dev/veld/internal/ast"
)

// Validate performs semantic checks on a parsed AST and returns all errors found.
func Validate(a ast.AST) []error {
	var errs []error

	// Collect model names and check for duplicates.
	modelNames := make(map[string]bool)
	for _, m := range a.Models {
		if modelNames[m.Name] {
			errs = append(errs, fmt.Errorf("duplicate model name: %q", m.Name))
		}
		modelNames[m.Name] = true
	}

	// Check modules for duplicate names and validate action type references.
	moduleNames := make(map[string]bool)
	for _, mod := range a.Modules {
		if moduleNames[mod.Name] {
			errs = append(errs, fmt.Errorf("duplicate module name: %q", mod.Name))
		}
		moduleNames[mod.Name] = true

		actionNames := make(map[string]bool)
		for _, act := range mod.Actions {
			if actionNames[act.Name] {
				errs = append(errs, fmt.Errorf("module %q: duplicate action name: %q", mod.Name, act.Name))
			}
			actionNames[act.Name] = true

			if act.Input != "" && !modelNames[act.Input] {
				errs = append(errs, fmt.Errorf("module %q, action %q: undefined input type %q", mod.Name, act.Name, act.Input))
			}
			if act.Output != "" && !modelNames[act.Output] {
				errs = append(errs, fmt.Errorf("module %q, action %q: undefined output type %q", mod.Name, act.Name, act.Output))
			}
		}
	}

	return errs
}

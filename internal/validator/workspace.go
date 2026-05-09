package validator

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/config"
)

// ValidateWorkspaceConsumes checks workspace-level consumes declarations for errors:
// - Unknown consumed service name
// - Self-consumption (A consumes A)
// Returns a list of errors and warnings. Errors are fatal; warnings are informational.
func ValidateWorkspaceConsumes(entries []config.WorkspaceEntry) (errs []error, warnings []string) {
	// Build lookup of valid workspace entry names.
	nameSet := make(map[string]bool, len(entries))
	for _, e := range entries {
		nameSet[e.Name] = true
	}

	// Check each entry's consumes list.
	for _, e := range entries {
		for _, consumed := range e.Consumes {
			// Self-consumption check.
			if consumed == e.Name {
				errs = append(errs, fmt.Errorf(
					"workspace %q: cannot consume itself",
					e.Name,
				))
				continue
			}
			// Unknown service check.
			if !nameSet[consumed] {
				available := make([]string, 0, len(entries))
				for _, other := range entries {
					if other.Name != e.Name {
						available = append(available, other.Name)
					}
				}
				errs = append(errs, fmt.Errorf(
					"workspace %q: consumes unknown service %q (available: %s)",
					e.Name, consumed, strings.Join(available, ", "),
				))
			}
		}

		// Warn if consumed service has no baseUrl.
		for _, consumed := range e.Consumes {
			if !nameSet[consumed] {
				continue // already reported as unknown
			}
			for _, other := range entries {
				if other.Name == consumed && other.BaseUrl == "" {
					warnings = append(warnings, fmt.Sprintf(
						"consumed service %q has no baseUrl — clients must provide it at runtime or via VELD_%s_URL",
						consumed, strings.ToUpper(strings.ReplaceAll(consumed, "-", "_")),
					))
				}
			}
		}
	}

	return errs, warnings
}


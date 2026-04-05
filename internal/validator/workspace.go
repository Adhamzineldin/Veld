package validator

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/config"
)

// ValidateWorkspaceConsumes checks workspace-level consumes declarations for errors:
// - Unknown consumed service name
// - Self-consumption (A consumes A)
// - Circular dependency chains (A→B→A)
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

	// Circular dependency detection using DFS.
	if circErr := detectCircularConsumes(entries); circErr != nil {
		errs = append(errs, circErr)
	}

	return errs, warnings
}

// detectCircularConsumes uses iterative DFS to find cycles in the consumes graph.
func detectCircularConsumes(entries []config.WorkspaceEntry) error {
	// Build adjacency map.
	graph := make(map[string][]string, len(entries))
	for _, e := range entries {
		graph[e.Name] = e.Consumes
	}

	const (
		white = 0 // not visited
		gray  = 1 // in current path
		black = 2 // fully processed
	)
	color := make(map[string]int, len(entries))
	parent := make(map[string]string) // for reconstructing the cycle path

	for _, e := range entries {
		if color[e.Name] != white {
			continue
		}

		// Iterative DFS with explicit stack.
		type frame struct {
			node string
			idx  int // index into graph[node] for next child
		}
		stack := []frame{{node: e.Name}}
		color[e.Name] = gray

		for len(stack) > 0 {
			top := &stack[len(stack)-1]
			children := graph[top.node]

			if top.idx >= len(children) {
				// Done with all children — mark black.
				color[top.node] = black
				stack = stack[:len(stack)-1]
				continue
			}

			child := children[top.idx]
			top.idx++

			switch color[child] {
			case gray:
				// Found a cycle — reconstruct path.
				cycle := []string{child}
				for i := len(stack) - 1; i >= 0; i-- {
					cycle = append([]string{stack[i].node}, cycle...)
					if stack[i].node == child {
						break
					}
				}
				return fmt.Errorf("circular service dependency: %s", strings.Join(cycle, " → "))
			case white:
				parent[child] = top.node
				color[child] = gray
				stack = append(stack, frame{node: child})
			}
			// black = already fully processed, skip
		}
	}

	return nil
}

package main_test

import (
	"fmt"
	"path/filepath"
	"testing"
)

// determinismContract extends testContract with a second module that shares
// User with Users and owns a model and an enum of its own. That makes the
// TypeScript types barrel list Orders' own types explicitly, the path where
// F-LEAD-01 put them in map iteration order.
const determinismContract = testContract + `
enum OrderStatus { open closed }

model Order {
  id: uuid
  owner: User
  status: OrderStatus
  tags: Map<string, string>
}

module Orders {
  prefix: /api/v1

  action ListOrders {
    method: GET
    path: /orders
    output: Order[]
  }

  action GetOrderOwner {
    method: GET
    path: /orders/:id/owner
    output: User
  }
}
`

// B-012 / F-LEAD-01: six generations of the same contract are byte-identical,
// for every backend, frontend and tool target.
func TestDeterministicOutput(t *testing.T) {
	targets := []struct{ backend, frontend string }{
		{"node-ts", "typescript"},
		{"node-ts", "react"},
		{"node-js", "javascript"},
		{"python", "vue"},
		{"go", "angular"},
		{"rust", "svelte"},
		{"java", "dart"},
		{"csharp", "kotlin"},
		{"php", "swift"},
		{"node-ts", "types-only"},
		{"openapi", "none"},
		{"database", "none"},
		{"dockerfile", "none"},
		{"cicd", "none"},
		{"env", "none"},
		{"scaffold-tests", "none"},
	}
	const runs = 6
	for _, tg := range targets {
		t.Run(tg.backend+"+"+tg.frontend, func(t *testing.T) {
			t.Parallel()
			var first map[string]string
			for i := 1; i <= runs; i++ {
				dir := setupTestProject(t)
				writeFile(t, filepath.Join(dir, "veld", "app.veld"), determinismContract)
				_, stderr, code := runVeld(t, dir, "generate", "--force",
					"--backend="+tg.backend, "--frontend="+tg.frontend)
				if code != 0 {
					t.Fatalf("run %d: exit %d; stderr: %s", i, code, stderr)
				}
				got := treeHashes(t, filepath.Join(dir, "generated"))
				if i == 1 {
					if len(got) == 0 {
						t.Fatalf("run 1 generated nothing")
					}
					first = got
					continue
				}
				requireSameTree(t, fmt.Sprintf("run %d vs run 1", i), first, got)
			}
		})
	}
}

//go:build outputsafety

// Output-safety acceptance tests for ADR-007 (backlog B-006 to B-009).
//
// They describe the required behaviour, so at 008bad2 most of them fail on
// purpose: they reproduce the findings listed in
// docs/notes/2026-10-24-unsafe-write-paths.md. The build tag keeps them out of
// the default `go test ./...` until the staged writer lands; remove the tag in
// the B-006 pull request. Run them with:
//
//	go test -tags outputsafety -run OutputSafety ./cmd/veld/
//
// Every case runs inside t.TempDir(). Never add a case whose --out points at
// the user's home or a filesystem root: today's binary would really delete it.
//
// Assertions are behavioural (exit status, file contents) and deliberately do
// not pin details still being decided: the manifest format (writer API note,
// due 2026-10-29) and exit-code numbers (B-046).

package main_test

import (
	"bufio"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"io"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// treeHashes returns the SHA-256 of every regular file under root, keyed by
// slash-separated relative path. A missing root yields an empty map.
func treeHashes(t *testing.T, root string) map[string]string {
	t.Helper()
	hashes := map[string]string{}
	err := filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			if os.IsNotExist(err) && path == root {
				return filepath.SkipDir
			}
			return err
		}
		if d.IsDir() {
			return nil
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		sum := sha256.Sum256(data)
		hashes[filepath.ToSlash(rel)] = hex.EncodeToString(sum[:])
		return nil
	})
	if err != nil {
		t.Fatalf("hash tree %s: %v", root, err)
	}
	return hashes
}

// requireSameTree fails with every added, removed or changed path.
func requireSameTree(t *testing.T, label string, before, after map[string]string) {
	t.Helper()
	for path, sum := range before {
		got, ok := after[path]
		switch {
		case !ok:
			t.Errorf("%s: %s was deleted", label, path)
		case got != sum:
			t.Errorf("%s: %s was modified", label, path)
		}
	}
	for path := range after {
		if _, ok := before[path]; !ok {
			t.Errorf("%s: %s was created", label, path)
		}
	}
}

func writeFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("mkdir %s: %v", filepath.Dir(path), err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("write %s: %v", path, err)
	}
}

func readFile(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return string(data)
}

// mustGenerate runs `veld generate --force` and fails the test on error.
func mustGenerate(t *testing.T, dir string) {
	t.Helper()
	if _, stderr, code := runVeld(t, dir, "generate", "--force"); code != 0 {
		t.Fatalf("baseline generate: exit %d; stderr: %s", code, stderr)
	}
}

// withoutAction returns contract with the named action block removed.
func withoutAction(t *testing.T, contract, action string) string {
	t.Helper()
	start := strings.Index(contract, "  action "+action+" {")
	if start < 0 {
		t.Fatalf("action %s not found in contract", action)
	}
	end := strings.Index(contract[start:], "\n  }\n")
	if end < 0 {
		t.Fatalf("end of action %s not found", action)
	}
	return contract[:start] + contract[start+end+len("\n  }\n"):]
}

// F-NODE-01, F-CLI-03: files Veld did not write survive generate.
func TestOutputSafetyUserFileSurvivesGenerate(t *testing.T) {
	dir := setupTestProject(t)
	mustGenerate(t, dir)
	userFile := filepath.Join(dir, "generated", "my-notes.ts")
	writeFile(t, userFile, "// hand-written\n")

	mustGenerate(t, dir)
	mustGenerate(t, dir)

	if got := readFile(t, userFile); got != "// hand-written\n" {
		t.Fatalf("user file changed: %q", got)
	}
}

// Guard for B-006: the manifest must still remove files of a deleted module.
// Passes today (via RemoveAll) and must keep passing with the staged writer.
func TestOutputSafetyStaleModuleFileRemoved(t *testing.T) {
	dir := setupTestProject(t)
	contractPath := filepath.Join(dir, "veld", "app.veld")
	writeFile(t, contractPath, testContract+`
module Orders {
  prefix: /api/v1

  action ListOrders {
    method: GET
    path: /orders
  }
}
`)
	mustGenerate(t, dir)
	stale := filepath.Join(dir, "generated", "routes", "orders.routes.ts")
	if _, err := os.Stat(stale); err != nil {
		t.Fatalf("precondition: %s not generated: %v", stale, err)
	}

	writeFile(t, contractPath, testContract)
	mustGenerate(t, dir)

	if _, err := os.Stat(stale); !os.IsNotExist(err) {
		t.Fatalf("stale file of removed module still present: %s", stale)
	}
}

// F-NODE-02, F-CLI-02: an output path that would cover the contract sources
// is refused before anything is deleted.
func TestOutputSafetyRefusesOutCoveringSources(t *testing.T) {
	cases := []struct {
		name   string
		args   func(dir string) []string
		config string // replaces veld.config.json when non-empty
	}{
		{
			name: "out flag is the config dir",
			args: func(string) []string { return []string{"generate", "--force", "--out=../veld"} },
		},
		{
			name: "out flag is the project root containing the config dir",
			args: func(dir string) []string { return []string{"generate", "--force", "--out=" + dir} },
		},
		{
			name: "workspace entry out is the config dir",
			args: func(string) []string { return []string{"generate", "--force"} },
			config: `{
  "workspace": [
    { "name": "api", "backendConfig": { "target": "node-ts" }, "input": "app.veld", "out": "." }
  ]
}`,
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := setupTestProject(t)
			if tc.config != "" {
				writeFile(t, filepath.Join(dir, "veld", "veld.config.json"), tc.config)
			}
			writeFile(t, filepath.Join(dir, "notes.txt"), "user notes\n")
			before := treeHashes(t, dir)

			_, _, code := runVeld(t, dir, tc.args(dir)...)

			if code == 0 {
				t.Errorf("expected generate to refuse the output path, got exit 0")
			}
			requireSameTree(t, "project", before, treeHashes(t, dir))
		})
	}
}

// F-NODE-03, F-CLI-01: a contract that fails to parse leaves the previous
// output and the breaking-change lock untouched.
func TestOutputSafetyFailedGenerateKeepsPreviousOutput(t *testing.T) {
	dir := setupTestProject(t)
	mustGenerate(t, dir)
	outBefore := treeHashes(t, filepath.Join(dir, "generated"))
	lockPath := filepath.Join(dir, "veld", ".veld.lock.json")
	lockBefore := readFile(t, lockPath)

	writeFile(t, filepath.Join(dir, "veld", "app.veld"), testContract+"\ngarbage {{{\n")
	if _, _, code := runVeld(t, dir, "generate", "--force"); code == 0 {
		t.Fatalf("precondition: generate of a broken contract should fail")
	}

	requireSameTree(t, "generated", outBefore, treeHashes(t, filepath.Join(dir, "generated")))
	if _, err := os.Stat(lockPath); err != nil {
		t.Fatalf("lock deleted by a failed run: %v", err)
	}
	if readFile(t, lockPath) != lockBefore {
		t.Fatalf("lock rewritten by a failed run")
	}
}

// F-NODE-03: saving a broken contract during `veld watch` keeps the output.
func TestOutputSafetyWatchKeepsOutputOnBrokenSave(t *testing.T) {
	dir := setupTestProject(t)
	outDir := filepath.Join(dir, "generated")

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, veldBinary(t), "watch")
	cmd.Dir = dir
	pr, pw := io.Pipe()
	cmd.Stdout = pw
	cmd.Stderr = pw
	if err := cmd.Start(); err != nil {
		t.Fatalf("start watch: %v", err)
	}
	defer func() {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		_ = pw.Close()
	}()

	lines := make(chan string)
	go func() {
		defer close(lines)
		sc := bufio.NewScanner(pr)
		for sc.Scan() {
			lines <- sc.Text()
		}
	}()
	// waitFor synchronises on watch's own log lines instead of sleeping.
	waitFor := func(marker string) {
		t.Helper()
		for {
			select {
			case line, ok := <-lines:
				if !ok {
					t.Fatalf("watch exited before printing %q", marker)
				}
				if strings.Contains(line, marker) {
					return
				}
			case <-ctx.Done():
				t.Fatalf("timed out waiting for %q", marker)
			}
		}
	}

	waitFor("Ready")
	before := treeHashes(t, outDir)
	if len(before) == 0 {
		t.Fatalf("precondition: watch produced no output")
	}

	writeFile(t, filepath.Join(dir, "veld", "app.veld"), testContract+"\ngarbage {{{\n")
	waitFor("save to retry")

	requireSameTree(t, "generated", before, treeHashes(t, outDir))
}

// F-NODE-04, F-CLI-01: removing an action is a breaking change that
// `generate --strict` blocks, and the lock baseline survives.
func TestOutputSafetyStrictBlocksBreakingChange(t *testing.T) {
	dir := setupTestProject(t)
	mustGenerate(t, dir)
	lockPath := filepath.Join(dir, "veld", ".veld.lock.json")
	lockBefore := readFile(t, lockPath)
	outBefore := treeHashes(t, filepath.Join(dir, "generated"))

	writeFile(t, filepath.Join(dir, "veld", "app.veld"), withoutAction(t, testContract, "DeleteUser"))
	_, _, code := runVeld(t, dir, "generate", "--strict")

	if code == 0 {
		t.Errorf("expected --strict to block removal of DeleteUser, got exit 0")
	}
	if _, err := os.Stat(lockPath); err != nil {
		t.Fatalf("lock deleted: %v", err)
	}
	if readFile(t, lockPath) != lockBefore {
		t.Errorf("lock rewritten although generation was blocked")
	}
	requireSameTree(t, "generated", outBefore, treeHashes(t, filepath.Join(dir, "generated")))
}

// F-CLI-06: --dry-run changes nothing on disk.
func TestOutputSafetyDryRunWritesNothing(t *testing.T) {
	cases := []struct {
		name     string
		baseline bool // generate once before the dry run
	}{
		{name: "fresh project"},
		{name: "after a real generate", baseline: true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := setupTestProject(t)
			if tc.baseline {
				mustGenerate(t, dir)
			}
			before := treeHashes(t, dir)

			if _, stderr, code := runVeld(t, dir, "generate", "--dry-run"); code != 0 {
				t.Fatalf("dry-run: exit %d; stderr: %s", code, stderr)
			}

			requireSameTree(t, "project", before, treeHashes(t, dir))
		})
	}
}

// F-CLI-03: `veld clean` never deletes files Veld did not write.
func TestOutputSafetyCleanKeepsUnmanagedFiles(t *testing.T) {
	cases := []struct {
		name     string
		baseline bool // generate once so the dir also holds managed files
	}{
		{name: "out dir holds only user files"},
		{name: "out dir holds user and generated files", baseline: true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := setupTestProject(t)
			if tc.baseline {
				mustGenerate(t, dir)
			}
			userFile := filepath.Join(dir, "generated", "business.ts")
			writeFile(t, userFile, "export const rule = 1;\n")

			runVeld(t, dir, "clean")

			if _, err := os.Stat(userFile); err != nil {
				t.Fatalf("clean deleted a user file: %v", err)
			}
		})
	}
}

// F-CLI-05: `veld init` never overwrites existing files.
func TestOutputSafetyInitNeverOverwrites(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, filepath.Join(dir, "README.md"), "MY README\n")
	writeFile(t, filepath.Join(dir, "veld", "app.veld"), "// mine\n")
	before := treeHashes(t, dir)

	_, _, code := runVeld(t, dir, "init")

	if code == 0 {
		t.Errorf("expected init to refuse existing files, got exit 0")
	}
	after := treeHashes(t, dir)
	for path, sum := range before {
		if after[path] != sum {
			t.Errorf("init overwrote or deleted %s", path)
		}
	}
}

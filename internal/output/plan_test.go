package output

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"
)

// generate stages files (slash path -> content) for out, plans with o and,
// unless the plan fails, applies it. It stands in for one `veld generate`.
func generate(t *testing.T, out string, files map[string]string, o Options) (*Plan, error) {
	t.Helper()
	sess, err := NewSession(Policy{})
	if err != nil {
		t.Fatalf("NewSession: %v", err)
	}
	defer func() {
		if err := sess.Close(); err != nil {
			t.Errorf("Close: %v", err)
		}
	}()
	staged, err := sess.Stage(out)
	if err != nil {
		return nil, err
	}
	for rel, content := range files {
		put(t, filepath.Join(staged, filepath.FromSlash(rel)), content)
	}
	plan, err := sess.Plan(o)
	if err != nil {
		return nil, err
	}
	if err := plan.Apply(); err != nil {
		t.Fatalf("Apply: %v", err)
	}
	return plan, nil
}

func mustGenerate(t *testing.T, out string, files map[string]string, o Options) *Plan {
	t.Helper()
	plan, err := generate(t, out, files, o)
	if err != nil {
		t.Fatalf("generate: %v", err)
	}
	return plan
}

func read(t *testing.T, p string) string {
	t.Helper()
	data, err := os.ReadFile(p)
	if err != nil {
		t.Fatalf("read %s: %v", p, err)
	}
	return string(data)
}

func exists(p string) bool {
	_, err := os.Lstat(p)
	return err == nil
}

func manifestPaths(t *testing.T, dir string) []string {
	t.Helper()
	m, found, err := ReadManifest(dir)
	if err != nil || !found {
		t.Fatalf("ReadManifest: found=%v err=%v", found, err)
	}
	var paths []string
	for _, e := range m.Files {
		paths = append(paths, e.Path)
	}
	return paths
}

var baseFiles = map[string]string{
	"index.ts":               "export * from './routes/users.routes';\n",
	"routes/users.routes.ts": "// users\n",
}

func TestFirstGenerateCreatesFilesAndManifest(t *testing.T) {
	out := filepath.Join(t.TempDir(), "generated")
	plan := mustGenerate(t, out, baseFiles, Options{VeldVersion: "1.2.3", Emitters: []string{"node-ts"}})

	if got := plan.Dirs[0].Create; !reflect.DeepEqual(got, []string{"index.ts", "routes/users.routes.ts"}) {
		t.Errorf("Create = %v", got)
	}
	if got := read(t, filepath.Join(out, "routes", "users.routes.ts")); got != "// users\n" {
		t.Errorf("content = %q", got)
	}
	if got := manifestPaths(t, out); !reflect.DeepEqual(got, []string{"index.ts", "routes/users.routes.ts"}) {
		t.Errorf("manifest paths = %v", got)
	}
	m, _, _ := ReadManifest(out)
	if m.VeldVersion != "1.2.3" || !reflect.DeepEqual(m.Emitters, []string{"node-ts"}) {
		t.Errorf("manifest header = %+v", m)
	}
}

func TestRegenerateLeavesUnchangedFilesAlone(t *testing.T) {
	out := filepath.Join(t.TempDir(), "generated")
	mustGenerate(t, out, baseFiles, Options{})
	old := time.Now().Add(-time.Hour).Truncate(time.Second)
	for _, p := range []string{filepath.Join(out, "index.ts"), filepath.Join(out, ManifestName)} {
		if err := os.Chtimes(p, old, old); err != nil {
			t.Fatalf("chtimes: %v", err)
		}
	}

	plan := mustGenerate(t, out, baseFiles, Options{})

	if plan.Changed() {
		t.Errorf("identical run reported changes: %+v", plan.Dirs[0])
	}
	for _, p := range []string{filepath.Join(out, "index.ts"), filepath.Join(out, ManifestName)} {
		info, err := os.Stat(p)
		if err != nil {
			t.Fatalf("stat: %v", err)
		}
		if !info.ModTime().Equal(old) {
			t.Errorf("%s was rewritten (mtime %v, want %v)", filepath.Base(p), info.ModTime(), old)
		}
	}
}

func TestUserFilesAreNeverTouched(t *testing.T) {
	out := filepath.Join(t.TempDir(), "generated")
	mustGenerate(t, out, baseFiles, Options{})
	user := filepath.Join(out, "routes", "my-notes.ts")
	put(t, user, "// mine\n")

	// A later run that drops every file must still keep the user's file and its directory.
	mustGenerate(t, out, map[string]string{"index.ts": "x"}, Options{})

	if got := read(t, user); got != "// mine\n" {
		t.Fatalf("user file changed: %q", got)
	}
	if exists(filepath.Join(out, "routes", "users.routes.ts")) {
		t.Errorf("stale generated file not removed")
	}
}

func TestUnmanagedCollision(t *testing.T) {
	cases := []struct {
		name         string
		existing     string
		overwrite    bool
		wantConflict bool
		wantList     string // which plan list the file lands in
	}{
		{name: "different content is refused", existing: "// mine\n", wantConflict: true},
		{name: "different content with overwrite", existing: "// mine\n", overwrite: true, wantList: "update"},
		{name: "identical content is adopted", existing: baseFiles["index.ts"], wantList: "unchanged"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			out := filepath.Join(t.TempDir(), "generated")
			put(t, filepath.Join(out, "index.ts"), tc.existing)

			plan, err := generate(t, out, baseFiles, Options{OverwriteUnmanaged: tc.overwrite})

			if tc.wantConflict {
				var ce *ConflictError
				if !errors.As(err, &ce) || len(ce.Paths) != 1 || !strings.Contains(ce.Paths[0], "index.ts") {
					t.Fatalf("expected a conflict on index.ts, got %v", err)
				}
				if got := read(t, filepath.Join(out, "index.ts")); got != tc.existing {
					t.Errorf("conflicting file modified: %q", got)
				}
				if exists(filepath.Join(out, "routes")) || exists(filepath.Join(out, ManifestName)) {
					t.Errorf("a refused plan wrote files")
				}
				return
			}
			if err != nil {
				t.Fatalf("generate: %v", err)
			}
			lists := map[string][]string{"update": plan.Dirs[0].Update, "unchanged": plan.Dirs[0].Unchanged}
			if !reflect.DeepEqual(lists[tc.wantList], []string{"index.ts"}) {
				t.Errorf("%s = %v, want [index.ts]", tc.wantList, lists[tc.wantList])
			}
			if got := manifestPaths(t, out); len(got) != 2 {
				t.Errorf("manifest should now manage both files, got %v", got)
			}
		})
	}
}

func TestStaleManagedFiles(t *testing.T) {
	out := filepath.Join(t.TempDir(), "generated")
	files := map[string]string{
		"index.ts":                 "i",
		"routes/orders.routes.ts":  "orders",
		"types/orders/order.ts":    "order",
		"routes/edited.routes.ts":  "edited",
		"routes/removed.routes.ts": "removed",
	}
	mustGenerate(t, out, files, Options{})
	put(t, filepath.Join(out, "routes", "edited.routes.ts"), "hand edit\n")
	if err := os.Remove(filepath.Join(out, "routes", "removed.routes.ts")); err != nil {
		t.Fatal(err)
	}

	plan := mustGenerate(t, out, map[string]string{"index.ts": "i"}, Options{})

	dp := plan.Dirs[0]
	if want := []string{"routes/orders.routes.ts", "types/orders/order.ts"}; !reflect.DeepEqual(dp.Delete, want) {
		t.Errorf("Delete = %v, want %v", dp.Delete, want)
	}
	if want := []string{"routes/edited.routes.ts"}; !reflect.DeepEqual(dp.Keep, want) {
		t.Errorf("Keep = %v, want %v", dp.Keep, want)
	}
	if exists(filepath.Join(out, "types")) {
		t.Errorf("empty directory types/ left behind")
	}
	if got := read(t, filepath.Join(out, "routes", "edited.routes.ts")); got != "hand edit\n" {
		t.Errorf("hand-edited stale file changed: %q", got)
	}
	if got := manifestPaths(t, out); !reflect.DeepEqual(got, []string{"index.ts"}) {
		t.Errorf("manifest = %v, want only index.ts", got)
	}
}

func TestPartialKeepsFilesNotProduced(t *testing.T) {
	out := filepath.Join(t.TempDir(), "generated")
	mustGenerate(t, out, baseFiles, Options{Emitters: []string{"node-ts"}})

	plan := mustGenerate(t, out, map[string]string{"index.ts": "new"}, Options{Partial: true, Emitters: []string{"typescript"}})

	if len(plan.Dirs[0].Delete) != 0 {
		t.Errorf("partial run deleted %v", plan.Dirs[0].Delete)
	}
	if !exists(filepath.Join(out, "routes", "users.routes.ts")) {
		t.Errorf("partial run removed a file it did not produce")
	}
	if got := manifestPaths(t, out); !reflect.DeepEqual(got, []string{"index.ts", "routes/users.routes.ts"}) {
		t.Errorf("manifest = %v", got)
	}
	m, _, _ := ReadManifest(out)
	if !reflect.DeepEqual(m.Emitters, []string{"node-ts", "typescript"}) {
		t.Errorf("emitters = %v", m.Emitters)
	}
}

func TestSessionStage(t *testing.T) {
	root := t.TempDir()
	configDir := filepath.Join(root, "veld")
	mkdirs(t, configDir)
	sess, err := NewSession(Policy{Protected: []Protected{{Label: "config directory", Path: configDir}}})
	if err != nil {
		t.Fatal(err)
	}
	stagingRoot := sess.root

	out := filepath.Join(root, "generated")
	a, err := sess.Stage(out)
	if err != nil {
		t.Fatalf("Stage: %v", err)
	}
	b, err := sess.Stage(filepath.Join(root, "x", "..", "generated"))
	if err != nil || a != b {
		t.Errorf("same directory staged twice: %q vs %q (err %v)", a, b, err)
	}
	if filepath.Base(a) != "generated" {
		t.Errorf("staging dir %q does not keep the base name", a)
	}
	if _, err := sess.Stage(configDir); err == nil {
		t.Errorf("Stage accepted the config directory")
	}
	if got := sess.Dirs(); len(got) != 1 {
		t.Errorf("Dirs = %v, want one entry", got)
	}

	if err := sess.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}
	if exists(stagingRoot) {
		t.Errorf("Close left %s behind", stagingRoot)
	}
	if exists(out) {
		t.Errorf("staging touched the real output directory")
	}
}

func TestPlanRejectsReservedFile(t *testing.T) {
	out := filepath.Join(t.TempDir(), "generated")
	_, err := generate(t, out, map[string]string{ManifestName: "{}"}, Options{})
	if err == nil || !strings.Contains(err.Error(), "reserved") {
		t.Fatalf("expected reserved-file error, got %v", err)
	}
}

func TestPlanRefusesCorruptManifest(t *testing.T) {
	out := filepath.Join(t.TempDir(), "generated")
	put(t, filepath.Join(out, ManifestName), "{")
	if _, err := generate(t, out, baseFiles, Options{}); err == nil {
		t.Fatalf("expected an error for a corrupt manifest")
	}
}

func TestRecoveryManifestKeepsEverythingVeldWrote(t *testing.T) {
	old := Manifest{VeldVersion: "1", Files: []Entry{{Path: "a.ts", SHA256: "old"}, {Path: "b.ts", SHA256: "b"}}}
	rec := recoveryManifest(old, map[string]string{"a.ts": "new", "c.ts": "c"})
	got := map[string]string{}
	for _, e := range rec.Files {
		got[e.Path] = e.SHA256
	}
	want := map[string]string{"a.ts": "new", "b.ts": "b", "c.ts": "c"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("recovery manifest = %v, want %v", got, want)
	}
}

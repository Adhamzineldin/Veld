package output

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

var testHash = hashBytes([]byte("x"))

func TestManifestRoundTripIsSortedAndDeterministic(t *testing.T) {
	dir := t.TempDir()
	m := Manifest{
		VeldVersion: "0.1.0",
		Emitters:    []string{"typescript", "node-ts", "typescript"},
		Files: []Entry{
			{Path: "routes/users.routes.ts", SHA256: testHash},
			{Path: "index.ts", SHA256: testHash},
		},
	}
	if err := writeManifest(dir, m); err != nil {
		t.Fatalf("write: %v", err)
	}
	first, err := os.ReadFile(filepath.Join(dir, ManifestName))
	if err != nil {
		t.Fatalf("read: %v", err)
	}

	got, found, err := ReadManifest(dir)
	if err != nil || !found {
		t.Fatalf("ReadManifest: found=%v err=%v", found, err)
	}
	if got.Files[0].Path != "index.ts" || got.Files[1].Path != "routes/users.routes.ts" {
		t.Errorf("files not sorted: %+v", got.Files)
	}
	if strings.Join(got.Emitters, ",") != "node-ts,typescript" {
		t.Errorf("emitters not sorted and unique: %v", got.Emitters)
	}

	if err := writeManifest(dir, got); err != nil {
		t.Fatalf("rewrite: %v", err)
	}
	second, err := os.ReadFile(filepath.Join(dir, ManifestName))
	if err != nil {
		t.Fatalf("read: %v", err)
	}
	if string(first) != string(second) {
		t.Errorf("manifest bytes changed on rewrite:\n%s\n---\n%s", first, second)
	}
}

func TestReadManifestMissing(t *testing.T) {
	_, found, err := ReadManifest(t.TempDir())
	if err != nil || found {
		t.Fatalf("expected not found without error, got found=%v err=%v", found, err)
	}
}

func TestReadManifestRejectsUnsafeContent(t *testing.T) {
	entry := func(p, h string) string {
		return `{"manifestVersion":1,"veldVersion":"x","emitters":[],"files":[{"path":"` + p + `","sha256":"` + h + `"}]}`
	}
	cases := []struct {
		name    string
		content string
		wantErr string
	}{
		{"parent escape", entry("../outside.ts", testHash), "escapes"},
		{"nested parent escape", entry("a/../../outside.ts", testHash), "clean"},
		{"absolute unix path", entry("/etc/passwd", testHash), "escapes"},
		{"backslash path", entry(`a\\b.ts`, testHash), "clean"},
		{"empty path", entry("", testHash), "clean"},
		{"manifest itself", entry(ManifestName, testHash), "reserved"},
		{"bad hash", entry("a.ts", "zz"), "sha256"},
		{"upper-case hash", entry("a.ts", strings.ToUpper(testHash)), "sha256"},
		{"unknown version", `{"manifestVersion":2,"files":[]}`, "version 2"},
		{"not json", `{`, "parse"},
		{"duplicate path", `{"manifestVersion":1,"files":[{"path":"a.ts","sha256":"` + testHash +
			`"},{"path":"a.ts","sha256":"` + testHash + `"}]}`, "duplicate"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			put(t, filepath.Join(dir, ManifestName), tc.content)
			_, _, err := ReadManifest(dir)
			if err == nil || !strings.Contains(err.Error(), tc.wantErr) {
				t.Fatalf("expected error containing %q, got %v", tc.wantErr, err)
			}
		})
	}
}

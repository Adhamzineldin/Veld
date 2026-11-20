package output

import (
	"path/filepath"
	"reflect"
	"testing"
)

func TestClean(t *testing.T) {
	cases := []struct {
		name        string
		generate    bool              // run a generation first
		userFiles   map[string]string // files written by the user afterwards
		edit        string            // managed file to hand-edit before cleaning
		wantManaged bool
		wantRemoved []string
		wantKept    []string
		wantDirGone bool
	}{
		{
			name:        "only managed files: directory removed",
			generate:    true,
			wantManaged: true,
			wantRemoved: []string{"index.ts", "routes/users.routes.ts"},
			wantDirGone: true,
		},
		{
			name:        "user file survives with its directory",
			generate:    true,
			userFiles:   map[string]string{"routes/mine.ts": "x"},
			wantManaged: true,
			wantRemoved: []string{"index.ts", "routes/users.routes.ts"},
		},
		{
			name:        "hand-edited managed file is kept",
			generate:    true,
			edit:        "index.ts",
			wantManaged: true,
			wantRemoved: []string{"routes/users.routes.ts"},
			wantKept:    []string{"index.ts"},
		},
		{
			name:      "no manifest: nothing removed",
			userFiles: map[string]string{"business.ts": "x"},
		},
		{
			name: "missing directory",
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			out := filepath.Join(t.TempDir(), "generated")
			if tc.generate {
				mustGenerate(t, out, baseFiles, Options{})
			}
			for rel, content := range tc.userFiles {
				put(t, filepath.Join(out, filepath.FromSlash(rel)), content)
			}
			if tc.edit != "" {
				put(t, filepath.Join(out, tc.edit), "edited\n")
			}

			res, err := Clean(out)
			if err != nil {
				t.Fatalf("Clean: %v", err)
			}

			if res.Managed != tc.wantManaged {
				t.Errorf("Managed = %v, want %v", res.Managed, tc.wantManaged)
			}
			if !reflect.DeepEqual(res.Removed, tc.wantRemoved) {
				t.Errorf("Removed = %v, want %v", res.Removed, tc.wantRemoved)
			}
			if !reflect.DeepEqual(res.Kept, tc.wantKept) {
				t.Errorf("Kept = %v, want %v", res.Kept, tc.wantKept)
			}
			for rel := range tc.userFiles {
				if !exists(filepath.Join(out, filepath.FromSlash(rel))) {
					t.Errorf("user file %s deleted", rel)
				}
			}
			if exists(filepath.Join(out, ManifestName)) {
				t.Errorf("manifest left behind")
			}
			if tc.wantDirGone && exists(out) {
				t.Errorf("empty output directory left behind")
			}
		})
	}
}

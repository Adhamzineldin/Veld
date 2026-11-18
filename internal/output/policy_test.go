package output

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func mkdirs(t *testing.T, paths ...string) {
	t.Helper()
	for _, p := range paths {
		if err := os.MkdirAll(p, 0o755); err != nil {
			t.Fatalf("mkdir %s: %v", p, err)
		}
	}
}

func put(t *testing.T, p, content string) {
	t.Helper()
	mkdirs(t, filepath.Dir(p))
	if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
		t.Fatalf("write %s: %v", p, err)
	}
}

func TestPolicyCheck(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "project")
	configDir := filepath.Join(project, "veld")
	mkdirs(t, configDir)
	put(t, filepath.Join(project, "loose", "old.veld"), "model A {}\n")

	policy := Policy{Protected: []Protected{
		{Label: "config directory", Path: configDir},
		{Label: "working directory", Path: project},
	}}

	cases := []struct {
		name    string
		dir     string
		wantErr string // substring; empty means allowed
	}{
		{name: "sibling of the config dir", dir: filepath.Join(project, "generated")},
		{name: "inside the config dir", dir: filepath.Join(configDir, "generated")},
		{name: "not yet existing nested dir", dir: filepath.Join(project, "a", "b", "c")},
		{name: "equals the config dir", dir: configDir, wantErr: "config directory"},
		{name: "config dir via dot-dot", dir: filepath.Join(configDir, "..", "veld"), wantErr: "config directory"},
		{name: "contains the config dir", dir: project, wantErr: "config directory"},
		{name: "parent of everything protected", dir: root, wantErr: "would contain"},
		{name: "filesystem root", dir: filepath.VolumeName(root) + string(filepath.Separator), wantErr: "filesystem root"},
		{name: "holds contract sources", dir: filepath.Join(project, "loose"), wantErr: "contract source"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			err := policy.Check(tc.dir)
			if tc.wantErr == "" {
				if err != nil {
					t.Fatalf("expected %s to be allowed, got %v", tc.dir, err)
				}
				return
			}
			if err == nil || !strings.Contains(err.Error(), tc.wantErr) {
				t.Fatalf("expected error containing %q for %s, got %v", tc.wantErr, tc.dir, err)
			}
		})
	}
}

func TestPolicyCheckRefusesHome(t *testing.T) {
	home, err := os.UserHomeDir()
	if err != nil {
		t.Skipf("no home directory: %v", err)
	}
	for _, dir := range []string{home, filepath.Dir(home)} {
		if err := (Policy{}).Check(dir); err == nil {
			t.Errorf("expected %s to be refused", dir)
		}
	}
}

func TestWithin(t *testing.T) {
	sep := string(filepath.Separator)
	base := filepath.VolumeName(os.TempDir()) + sep + "x"
	cases := []struct {
		name          string
		parent, child string
		want          bool
	}{
		{"same", base, base, true},
		{"child", base, base + sep + "y", true},
		{"sibling with shared prefix", base, base + "y", false},
		{"parent of base", base + sep + "y", base, false},
		{"root contains everything", filepath.VolumeName(base) + sep, base, true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := within(tc.parent, tc.child); got != tc.want {
				t.Fatalf("within(%q, %q) = %v, want %v", tc.parent, tc.child, got, tc.want)
			}
		})
	}
}

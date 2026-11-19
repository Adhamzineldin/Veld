package output

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
)

// Session collects one generation run in a private staging directory.
//
// Callers ask Stage for the staging path of each real output directory and
// let emitters write there. Plan then compares the staged files with what is
// on disk, and Apply moves them into place. Close always removes the staging
// directory; it never touches a real output directory.
type Session struct {
	policy Policy
	root   string
	dirs   []stagedDir
}

type stagedDir struct {
	real   string // canonical real output directory
	staged string
}

// NewSession creates the staging directory under the system temp directory,
// so that staging never adds anything to the user's project.
func NewSession(policy Policy) (*Session, error) {
	root, err := os.MkdirTemp("", "veld-staging-*")
	if err != nil {
		return nil, fmt.Errorf("create staging directory: %w", err)
	}
	return &Session{policy: policy, root: root}, nil
}

// Stage checks dir against the policy and returns the staging directory that
// stands in for it. Calling Stage again with the same directory returns the
// same staging directory. The staging directory keeps dir's base name, so
// emitters that derive names from it behave as they would on dir.
func (s *Session) Stage(dir string) (string, error) {
	real, err := canonical(dir)
	if err != nil {
		return "", fmt.Errorf("resolve output directory %s: %w", dir, err)
	}
	for _, d := range s.dirs {
		if sameDir(d.real, real) {
			return d.staged, nil
		}
	}
	if err := s.policy.Check(real); err != nil {
		return "", err
	}
	staged := filepath.Join(s.root, strconv.Itoa(len(s.dirs)), filepath.Base(real))
	if err := os.MkdirAll(staged, 0o755); err != nil {
		return "", fmt.Errorf("create staging directory for %s: %w", real, err)
	}
	s.dirs = append(s.dirs, stagedDir{real: real, staged: staged})
	return staged, nil
}

// Dirs returns the real output directories staged so far, in Stage order.
func (s *Session) Dirs() []string {
	out := make([]string, len(s.dirs))
	for i, d := range s.dirs {
		out[i] = d.real
	}
	return out
}

// Close removes the staging directory.
func (s *Session) Close() error {
	if err := os.RemoveAll(s.root); err != nil {
		return fmt.Errorf("remove staging directory %s: %w", s.root, err)
	}
	return nil
}

func sameDir(a, b string) bool {
	return within(a, b) && within(b, a)
}

// Package output writes generated files without ever destroying files Veld
// did not write (ADR-007).
//
// Emitters write into a private staging directory obtained from a Session.
// Nothing touches the real output directory until Plan has checked every
// staged file against the previous .veld-manifest.json and Apply swaps the
// files in. Only files listed in a manifest, with unchanged content, are ever
// deleted.
package output

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"strings"
)

// ContractExt is the file extension of contract sources. An output directory
// that already holds a contract source is refused.
const ContractExt = ".veld"

// Protected is a directory that an output directory may neither equal nor
// contain, for example the config directory.
type Protected struct {
	Label string // shown in the error, e.g. "config directory"
	Path  string
}

// Policy decides which output directories are safe to write to (ADR-007 §5).
// The filesystem root and the user's home directory are always protected.
type Policy struct {
	Protected []Protected
}

// Check returns an error when dir equals or contains a protected directory,
// is a filesystem root, or already contains contract sources.
func (p Policy) Check(dir string) error {
	out, err := canonical(dir)
	if err != nil {
		return fmt.Errorf("resolve output directory %s: %w", dir, err)
	}
	if isRoot(out) {
		return fmt.Errorf("refusing output directory %s: it is a filesystem root", out)
	}
	for _, prot := range p.withDefaults() {
		if prot.Path == "" {
			continue
		}
		pp, err := canonical(prot.Path)
		if err != nil {
			return fmt.Errorf("resolve %s %s: %w", prot.Label, prot.Path, err)
		}
		if within(out, pp) {
			return fmt.Errorf("refusing output directory %s: it would contain the %s %s", out, prot.Label, pp)
		}
	}
	return checkNoContracts(out)
}

func (p Policy) withDefaults() []Protected {
	all := append([]Protected(nil), p.Protected...)
	if home, err := os.UserHomeDir(); err == nil {
		all = append(all, Protected{Label: "home directory", Path: home})
	}
	return all
}

// checkNoContracts refuses an existing directory that holds .veld files.
func checkNoContracts(dir string) error {
	var found string
	err := filepath.WalkDir(dir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			if errors.Is(err, fs.ErrNotExist) && path == dir {
				return filepath.SkipAll
			}
			return err
		}
		if !d.IsDir() && strings.EqualFold(filepath.Ext(path), ContractExt) {
			found = path
			return filepath.SkipAll
		}
		return nil
	})
	if err != nil {
		return fmt.Errorf("scan output directory %s: %w", dir, err)
	}
	if found != "" {
		return fmt.Errorf("refusing output directory %s: it contains the contract source %s", dir, found)
	}
	return nil
}

// canonical returns the cleaned absolute form of p with symlinks resolved on
// the longest existing prefix, so that equal directories compare equal.
func canonical(p string) (string, error) {
	abs, err := filepath.Abs(p)
	if err != nil {
		return "", err
	}
	existing, rest := abs, ""
	for {
		resolved, err := filepath.EvalSymlinks(existing)
		if err == nil {
			return filepath.Join(resolved, rest), nil
		}
		parent := filepath.Dir(existing)
		if parent == existing {
			return abs, nil
		}
		rest = filepath.Join(filepath.Base(existing), rest)
		existing = parent
	}
}

func isRoot(p string) bool {
	return filepath.Dir(p) == p
}

// within reports whether child equals parent or lies inside it. Both must be
// canonical. It uses a trailing-separator prefix test (AGENTS.md §6).
func within(parent, child string) bool {
	if foldCase {
		parent, child = strings.ToLower(parent), strings.ToLower(child)
	}
	if parent == child {
		return true
	}
	prefix := parent
	if !strings.HasSuffix(prefix, string(filepath.Separator)) {
		prefix += string(filepath.Separator)
	}
	return strings.HasPrefix(child, prefix)
}

// foldCase is true where the default filesystem is case-insensitive.
var foldCase = runtime.GOOS == "windows" || runtime.GOOS == "darwin"

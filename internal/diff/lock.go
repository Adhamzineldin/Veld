package diff

// lock.go — reads and writes the .veld.lock.json snapshot file.
//
// The lock file captures a minimal, deterministic representation of the
// compiled AST so that subsequent `veld generate` / `veld watch` runs can
// compare the new contract against the last-known-good version and surface
// breaking changes before any code is written.
//
// Format (JSON):
//
//	{
//	  "version": 1,
//	  "modules": [ { "name": "...", "prefix": "...", "actions": [...] } ],
//	  "models":  [ { "name": "...", "extends": "...", "fields": [...] } ]
//	}
//
// The lock file is written alongside veld.config.json and MUST be committed
// to version control so that CI can catch breaking changes in pull requests.

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

const (
	lockFileName = ".veld.lock.json"
	lockVersion  = 1
)

// lockFile is the on-disk representation of a locked AST snapshot.
type lockFile struct {
	Version int          `json:"version"`
	Modules []ast.Module `json:"modules"`
	Models  []ast.Model  `json:"models"`
}

// LockPath returns the canonical path of the lock file relative to the
// directory that contains the veld config file (configDir).
func LockPath(configDir string) string {
	return filepath.Join(configDir, lockFileName)
}

// LoadLock reads the lock file from disk and reconstructs an ast.AST.
// Returns (zero AST, nil) if the lock file does not exist yet — callers
// treat that as "no previous version, skip diff".
// Returns a non-nil error only on I/O or JSON parse failures.
func LoadLock(configDir string) (ast.AST, bool, error) {
	data, err := os.ReadFile(LockPath(configDir))
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return ast.AST{}, false, nil
		}
		return ast.AST{}, false, err
	}

	var lf lockFile
	if err := json.Unmarshal(data, &lf); err != nil {
		return ast.AST{}, false, err
	}

	return ast.AST{Modules: lf.Modules, Models: lf.Models}, true, nil
}

// SaveLock writes a new lock file capturing the current AST snapshot.
// It overwrites any existing lock file atomically via a temp-file rename.
func SaveLock(configDir string, a ast.AST) error {
	lf := lockFile{
		Version: lockVersion,
		Modules: a.Modules,
		Models:  a.Models,
	}

	data, err := json.MarshalIndent(lf, "", "  ")
	if err != nil {
		return err
	}

	// Atomic write: write to a temp file then rename.
	tmp := LockPath(configDir) + ".tmp"
	if err := os.WriteFile(tmp, data, 0644); err != nil {
		return err
	}
	return os.Rename(tmp, LockPath(configDir))
}

// DeleteLock removes the lock file. Called by `veld clean` so that a fresh
// generate starts without a stale baseline.
func DeleteLock(configDir string) error {
	err := os.Remove(LockPath(configDir))
	if errors.Is(err, os.ErrNotExist) {
		return nil // already gone — not an error
	}
	return err
}

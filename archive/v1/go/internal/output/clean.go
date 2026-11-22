package output

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
)

// CleanResult reports what Clean did. Paths are slash-separated and relative
// to the cleaned directory.
type CleanResult struct {
	Managed bool     // the directory had a manifest
	Removed []string // managed files deleted
	Kept    []string // managed files edited since generation, left in place
}

// Clean deletes exactly the files listed in dir's manifest whose content is
// still what Veld wrote, then the directories that leaves empty, then the
// manifest (ADR-007 §3). Without a manifest it deletes nothing.
func Clean(dir string) (CleanResult, error) {
	root, err := canonical(dir)
	if err != nil {
		return CleanResult{}, fmt.Errorf("resolve %s: %w", dir, err)
	}
	m, found, err := ReadManifest(root)
	if err != nil || !found {
		return CleanResult{}, err
	}
	res := CleanResult{Managed: true}
	for _, e := range m.Files {
		p := filepath.Join(root, filepath.FromSlash(e.Path))
		sum, isFile, err := hashFile(p)
		if err != nil {
			return res, fmt.Errorf("hash %s: %w", p, err)
		}
		switch {
		case !isFile:
			continue
		case sum != e.SHA256:
			res.Kept = append(res.Kept, e.Path)
			continue
		}
		if err := os.Remove(p); err != nil && !errors.Is(err, fs.ErrNotExist) {
			return res, fmt.Errorf("remove %s: %w", p, err)
		}
		res.Removed = append(res.Removed, e.Path)
		removeEmptyParents(root, p)
	}
	if err := os.Remove(filepath.Join(root, ManifestName)); err != nil && !errors.Is(err, fs.ErrNotExist) {
		return res, fmt.Errorf("remove manifest: %w", err)
	}
	left, err := os.ReadDir(root)
	if err != nil {
		return res, fmt.Errorf("list %s: %w", root, err)
	}
	if len(left) == 0 {
		if err := os.Remove(root); err != nil {
			return res, fmt.Errorf("remove empty output directory %s: %w", root, err)
		}
	}
	return res, nil
}

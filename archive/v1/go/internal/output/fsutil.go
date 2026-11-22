package output

import (
	"fmt"
	"os"
	"path/filepath"
)

// writeAtomic writes data to a temporary file beside dst and renames it over
// dst, so a reader never sees a half-written file.
func writeAtomic(dst string, data []byte, perm os.FileMode) error {
	dir := filepath.Dir(dst)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return fmt.Errorf("create %s: %w", dir, err)
	}
	tmp, err := os.CreateTemp(dir, ".veld-tmp-*")
	if err != nil {
		return fmt.Errorf("create temporary file in %s: %w", dir, err)
	}
	tmpName := tmp.Name()
	fail := func(err error) error {
		tmp.Close()
		os.Remove(tmpName)
		return err
	}
	if _, err := tmp.Write(data); err != nil {
		return fail(fmt.Errorf("write %s: %w", dst, err))
	}
	if err := tmp.Close(); err != nil {
		return fail(fmt.Errorf("close %s: %w", dst, err))
	}
	if err := os.Chmod(tmpName, perm); err != nil {
		return fail(fmt.Errorf("chmod %s: %w", dst, err))
	}
	if err := os.Rename(tmpName, dst); err != nil {
		return fail(fmt.Errorf("replace %s: %w", dst, err))
	}
	return nil
}

// removeEmptyParents removes the now-empty directories between file and root,
// never root itself.
func removeEmptyParents(root, file string) {
	for d := filepath.Dir(file); d != root && within(root, d); d = filepath.Dir(d) {
		if os.Remove(d) != nil {
			return // not empty, or not removable: stop climbing
		}
	}
}

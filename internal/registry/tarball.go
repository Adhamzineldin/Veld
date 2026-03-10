package registry

import (
	"archive/tar"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

// Pack creates a .tar.gz of all .veld files + veld.config.json rooted at dir.
// Returns the path of the temporary tarball file and its SHA-256 hex digest.
func Pack(dir string) (tarPath string, sha string, err error) {
	// Collect files
	var files []string
	patterns := []string{"*.veld", "**/*.veld", "veld.config.json", "veld/*.veld", "veld/**/*.veld", "veld/veld.config.json"}

	err = filepath.WalkDir(dir, func(path string, d os.DirEntry, walkErr error) error {
		if walkErr != nil || d.IsDir() {
			return walkErr
		}
		rel, _ := filepath.Rel(dir, path)
		ext := filepath.Ext(path)
		base := filepath.Base(path)
		if ext == ".veld" || base == "veld.config.json" {
			files = append(files, rel)
		}
		_ = patterns
		return nil
	})
	if err != nil {
		return
	}
	if len(files) == 0 {
		err = fmt.Errorf("no .veld files or veld.config.json found in %s", dir)
		return
	}

	// Write temp file
	tmp, createErr := os.CreateTemp("", "veld-*.tar.gz")
	if createErr != nil {
		err = createErr
		return
	}
	tarPath = tmp.Name()

	hasher := sha256.New()
	mw := io.MultiWriter(tmp, hasher)

	gw := gzip.NewWriter(mw)
	tw := tar.NewWriter(gw)

	for _, rel := range files {
		absPath := filepath.Join(dir, rel)
		fi, statErr := os.Stat(absPath)
		if statErr != nil {
			continue
		}
		f, openErr := os.Open(absPath)
		if openErr != nil {
			continue
		}
		hdr := &tar.Header{
			Name:    "package/" + filepath.ToSlash(rel),
			Mode:    0644,
			Size:    fi.Size(),
			ModTime: fi.ModTime(),
		}
		tw.WriteHeader(hdr)
		io.Copy(tw, f)
		f.Close()
	}

	tw.Close()
	gw.Close()
	tmp.Close()

	sha = hex.EncodeToString(hasher.Sum(nil))
	return
}

// Unpack extracts a .tar.gz tarball into destDir, expecting "package/" prefix entries.
func Unpack(tarPath, destDir string) error {
	f, err := os.Open(tarPath)
	if err != nil {
		return err
	}
	defer f.Close()

	return UnpackReader(f, destDir)
}

// UnpackReader extracts from an io.Reader.
func UnpackReader(r io.Reader, destDir string) error {
	gr, err := gzip.NewReader(r)
	if err != nil {
		return fmt.Errorf("not a valid gzip archive: %w", err)
	}
	defer gr.Close()

	tr := tar.NewReader(gr)
	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}

		// Strip "package/" prefix
		name := hdr.Name
		if strings.HasPrefix(name, "package/") {
			name = strings.TrimPrefix(name, "package/")
		}

		// Security: prevent path traversal
		name = filepath.Clean(name)
		if strings.HasPrefix(name, "..") {
			continue
		}

		dest := filepath.Join(destDir, name)

		if hdr.Typeflag == tar.TypeDir {
			os.MkdirAll(dest, 0755)
			continue
		}

		os.MkdirAll(filepath.Dir(dest), 0755)
		out, err := os.Create(dest)
		if err != nil {
			return err
		}
		io.Copy(out, tr)
		out.Close()
	}
	return nil
}

// VerifySHA checks a file's SHA-256 against expected hex.
func VerifySHA(filePath, expected string) error {
	f, err := os.Open(filePath)
	if err != nil {
		return err
	}
	defer f.Close()
	h := sha256.New()
	io.Copy(h, f)
	got := hex.EncodeToString(h.Sum(nil))
	if got != expected {
		return fmt.Errorf("SHA-256 mismatch: expected %s got %s", expected, got)
	}
	return nil
}

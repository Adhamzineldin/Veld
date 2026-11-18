package output

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"
)

// ManifestName is the file in every output directory that records which
// files Veld wrote there and their SHA-256.
const ManifestName = ".veld-manifest.json"

// ManifestVersion is the manifest format version this build reads and writes.
// Changing the format means bumping it (AGENTS.md §1.9, ADR-009).
const ManifestVersion = 1

// maxManifestSize bounds how much of a manifest is read (AGENTS.md §6).
const maxManifestSize = 16 << 20

// Manifest is the content of .veld-manifest.json.
type Manifest struct {
	ManifestVersion int      `json:"manifestVersion"`
	VeldVersion     string   `json:"veldVersion"`
	Emitters        []string `json:"emitters"`
	Files           []Entry  `json:"files"`
}

// Entry is one managed file.
type Entry struct {
	Path   string `json:"path"` // slash-separated, relative to the output directory
	SHA256 string `json:"sha256"`
}

// ReadManifest loads the manifest of dir. found is false when there is none.
// Every entry is validated, so a tampered manifest can never name a path
// outside dir.
func ReadManifest(dir string) (m Manifest, found bool, err error) {
	f, err := os.Open(filepath.Join(dir, ManifestName))
	if errors.Is(err, fs.ErrNotExist) {
		return Manifest{}, false, nil
	}
	if err != nil {
		return Manifest{}, false, fmt.Errorf("open manifest: %w", err)
	}
	defer f.Close()

	data, err := io.ReadAll(io.LimitReader(f, maxManifestSize+1))
	if err != nil {
		return Manifest{}, false, fmt.Errorf("read manifest: %w", err)
	}
	if len(data) > maxManifestSize {
		return Manifest{}, false, fmt.Errorf("manifest %s exceeds %d bytes", f.Name(), maxManifestSize)
	}
	if err := json.Unmarshal(data, &m); err != nil {
		return Manifest{}, false, fmt.Errorf("parse manifest %s: %w", f.Name(), err)
	}
	if m.ManifestVersion != ManifestVersion {
		return Manifest{}, false, fmt.Errorf("manifest %s has version %d; this veld reads version %d",
			f.Name(), m.ManifestVersion, ManifestVersion)
	}
	seen := make(map[string]bool, len(m.Files))
	for _, e := range m.Files {
		if err := validEntry(e); err != nil {
			return Manifest{}, false, fmt.Errorf("manifest %s: %w", f.Name(), err)
		}
		if seen[e.Path] {
			return Manifest{}, false, fmt.Errorf("manifest %s: duplicate path %q", f.Name(), e.Path)
		}
		seen[e.Path] = true
	}
	return m, true, nil
}

// writeManifest stores m in dir atomically, entries and emitters sorted so
// the file is deterministic.
func writeManifest(dir string, m Manifest) error {
	m.ManifestVersion = ManifestVersion
	m.Emitters = sortedUnique(m.Emitters)
	sort.Slice(m.Files, func(i, j int) bool { return m.Files[i].Path < m.Files[j].Path })
	if m.Files == nil {
		m.Files = []Entry{}
	}
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetIndent("", "  ")
	enc.SetEscapeHTML(false)
	if err := enc.Encode(m); err != nil {
		return fmt.Errorf("encode manifest: %w", err)
	}
	return writeAtomic(filepath.Join(dir, ManifestName), buf.Bytes(), 0o644)
}

// index maps each managed path to its recorded hash.
func (m Manifest) index() map[string]string {
	idx := make(map[string]string, len(m.Files))
	for _, e := range m.Files {
		idx[e.Path] = e.SHA256
	}
	return idx
}

// validEntry accepts only clean, local, slash-separated paths and SHA-256 hex.
func validEntry(e Entry) error {
	if err := validRelPath(e.Path); err != nil {
		return err
	}
	if len(e.SHA256) != sha256.Size*2 || strings.ToLower(e.SHA256) != e.SHA256 {
		return fmt.Errorf("path %q: invalid sha256 %q", e.Path, e.SHA256)
	}
	if _, err := hex.DecodeString(e.SHA256); err != nil {
		return fmt.Errorf("path %q: invalid sha256 %q", e.Path, e.SHA256)
	}
	return nil
}

func validRelPath(p string) error {
	switch {
	case p == "" || strings.Contains(p, `\`) || path.Clean(p) != p:
		return fmt.Errorf("path %q is not a clean slash-separated path", p)
	case !filepath.IsLocal(filepath.FromSlash(p)):
		return fmt.Errorf("path %q escapes the output directory", p)
	case p == ManifestName:
		return fmt.Errorf("path %q is reserved for the manifest", p)
	}
	return nil
}

func hashBytes(b []byte) string {
	sum := sha256.Sum256(b)
	return hex.EncodeToString(sum[:])
}

// hashFile returns the SHA-256 of a regular file. ok is false when path does
// not exist or is not a regular file.
func hashFile(p string) (sum string, ok bool, err error) {
	info, err := os.Lstat(p)
	if errors.Is(err, fs.ErrNotExist) {
		return "", false, nil
	}
	if err != nil {
		return "", false, err
	}
	if !info.Mode().IsRegular() {
		return "", false, nil
	}
	data, err := os.ReadFile(p)
	if err != nil {
		return "", false, err
	}
	return hashBytes(data), true, nil
}

func sortedUnique(in []string) []string {
	out := make([]string, 0, len(in))
	seen := make(map[string]bool, len(in))
	for _, s := range in {
		if s != "" && !seen[s] {
			seen[s] = true
			out = append(out, s)
		}
	}
	sort.Strings(out)
	return out
}

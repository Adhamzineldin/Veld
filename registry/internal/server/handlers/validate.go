package handlers

import (
	"archive/tar"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"path"
	"regexp"
	"strings"

	"github.com/BurntSushi/toml"
)

// Limits applied while inspecting an uploaded package. The registry never
// extracts a package to disk; it only streams through it.
const (
	maxUploadBytes     = 32 << 20  // compressed upload
	maxUnpackedBytes   = 128 << 20 // total decompressed bytes
	maxEntries         = 5000
	maxManifestBytes   = 256 << 10
	manifestFileName   = "veld-package.toml"
	supportedLanguage  = "2"
	maxDescriptionSize = 2000
	maxDependencies    = 256
	maxRangeLength     = 128
)

var (
	orgNameRe = regexp.MustCompile(`^[a-z0-9][a-z0-9-]{0,63}$`)
	pkgNameRe = regexp.MustCompile(`^[a-z0-9][a-z0-9._-]{0,63}$`)
	semverRe  = regexp.MustCompile(`^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$`)
	sha256Re  = regexp.MustCompile(`^[0-9a-f]{64}$`)
	// One comparator of a version range: an optional operator and a (partial) version, or `*`.
	// Ranges are comma separated, as in `>=1.2, <2`.
	comparatorRe = regexp.MustCompile(`^(\^|~|=|>=|<=|>|<)?\s*(\*|(0|[1-9]\d*)(\.(\*|(0|[1-9]\d*)(\.(\*|(0|[1-9]\d*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?))?))?)$`)
	depNameRe    = regexp.MustCompile(`^@([a-z0-9][a-z0-9-]{0,63})/([a-z0-9][a-z0-9._-]{0,63})$`)
)

// PackageManifest is the v2 `veld-package.toml` found at the tarball root.
type PackageManifest struct {
	Name        string            `toml:"name"`
	Version     string            `toml:"version"`
	Language    string            `toml:"language"`
	Description string            `toml:"description"`
	Files       map[string]string `toml:"files"`
	// Dependencies maps `@org/name` to a semver range (the package's own
	// `[dependencies]`, written by `veld publish`).
	Dependencies map[string]string `toml:"dependencies"`
}

// ValidateNames checks the org/name/version triple from the publish form. The
// values become storage keys, so they must be path safe.
func ValidateNames(org, name, version string) error {
	if !orgNameRe.MatchString(org) {
		return errors.New("invalid org name")
	}
	if !pkgNameRe.MatchString(name) || strings.Contains(name, "..") {
		return errors.New("invalid package name")
	}
	if !semverRe.MatchString(version) {
		return errors.New("version must be semver (MAJOR.MINOR.PATCH)")
	}
	return nil
}

// ValidatePackage treats the tarball as opaque except for the v2 manifest: it
// must contain veld-package.toml whose identity matches the publish form and
// whose file checksums match the archive. Nothing is extracted.
func ValidatePackage(org, name, version string, tarball io.Reader) (*PackageManifest, error) {
	gz, err := gzip.NewReader(tarball)
	if err != nil {
		return nil, errors.New("tarball is not valid gzip")
	}
	defer gz.Close()
	tr := tar.NewReader(io.LimitReader(gz, maxUnpackedBytes+1))

	var manifest *PackageManifest
	sums := map[string]string{}
	entries := 0
	var total int64
	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, errors.New("tarball is corrupt or exceeds the size limit")
		}
		entries++
		if entries > maxEntries {
			return nil, errors.New("tarball has too many entries")
		}
		if hdr.Typeflag == tar.TypeDir {
			continue
		}
		if hdr.Typeflag != tar.TypeReg {
			return nil, fmt.Errorf("tarball entry %q is not a regular file", hdr.Name)
		}
		if err := checkEntryName(hdr.Name); err != nil {
			return nil, err
		}
		h := sha256.New()
		isManifest := hdr.Name == manifestFileName
		if isManifest {
			if hdr.Size > maxManifestBytes {
				return nil, errors.New("manifest too large")
			}
			buf := &limitedBuffer{limit: maxManifestBytes}

			n, err := io.Copy(io.MultiWriter(h, buf), tr)
			if err != nil {
				return nil, errors.New("tarball is corrupt")
			}
			total += n
			m := &PackageManifest{}
			if _, err := toml.Decode(string(buf.b), m); err != nil {
				return nil, fmt.Errorf("invalid %s: %v", manifestFileName, err)
			}
			manifest = m
			continue
		}
		n, err := io.Copy(h, tr)
		if err != nil {
			return nil, errors.New("tarball is corrupt or exceeds the size limit")
		}
		total += n
		if total > maxUnpackedBytes {
			return nil, errors.New("tarball unpacks to more than the size limit")
		}
		sums[hdr.Name] = hex.EncodeToString(h.Sum(nil))
	}
	if manifest == nil {
		return nil, fmt.Errorf("package is missing %s", manifestFileName)
	}
	if err := checkManifest(manifest, org, name, version, sums); err != nil {
		return nil, err
	}
	return manifest, nil
}

func checkEntryName(n string) error {
	if n == "" || strings.HasPrefix(n, "/") || strings.Contains(n, "\\") ||
		strings.Contains(n, ":") || strings.ContainsRune(n, 0) {
		return fmt.Errorf("unsafe tarball entry name %q", n)
	}
	if path.Clean(n) != n {
		return fmt.Errorf("unsafe tarball entry name %q", n)
	}
	for _, seg := range strings.Split(n, "/") {
		if seg == ".." {
			return fmt.Errorf("unsafe tarball entry name %q", n)
		}
	}
	return nil
}

func checkManifest(m *PackageManifest, org, name, version string, sums map[string]string) error {
	if want := "@" + org + "/" + name; m.Name != want {
		return fmt.Errorf("manifest name %q does not match published package %q", m.Name, want)
	}
	if m.Version != version {
		return fmt.Errorf("manifest version %q does not match published version %q", m.Version, version)
	}
	if m.Language != supportedLanguage {
		return fmt.Errorf("manifest language must be %q (Veld 2 packages only)", supportedLanguage)
	}
	if len(m.Description) > maxDescriptionSize {
		return errors.New("manifest description too long")
	}
	if err := checkDependencies(m, org, name); err != nil {
		return err
	}
	for p, sum := range m.Files {
		if !sha256Re.MatchString(sum) {
			return fmt.Errorf("manifest checksum for %q is not a sha256 hex digest", p)
		}
		got, ok := sums[p]
		if !ok {
			return fmt.Errorf("manifest lists %q but the tarball does not contain it", p)
		}
		if got != sum {
			return fmt.Errorf("checksum mismatch for %q", p)
		}
	}
	for p := range sums {
		if _, ok := m.Files[p]; !ok {
			return fmt.Errorf("tarball file %q is not listed in the manifest", p)
		}
	}
	return nil
}

type limitedBuffer struct {
	b     []byte
	limit int
}

func (l *limitedBuffer) Write(p []byte) (int, error) {
	if len(l.b)+len(p) > l.limit {
		return 0, errors.New("manifest too large")
	}
	l.b = append(l.b, p...)
	return len(p), nil
}

// ValidRange reports whether r is a version range the registry clients can
// resolve: empty, `*`, `latest`, or comma separated comparators such as `^1.2`,
// `~1.2.3`, `>=1, <2`, `=1.0.0` or `1.*`.
func ValidRange(r string) bool {
	r = strings.TrimSpace(r)
	if r == "" || r == "latest" {
		return true
	}
	if len(r) > maxRangeLength {
		return false
	}
	for _, c := range strings.Split(r, ",") {
		if !comparatorRe.MatchString(strings.TrimSpace(c)) {
			return false
		}
	}
	return true
}

func checkDependencies(m *PackageManifest, org, name string) error {
	if len(m.Dependencies) > maxDependencies {
		return fmt.Errorf("manifest lists more than %d dependencies", maxDependencies)
	}
	self := "@" + org + "/" + name
	for dep, rng := range m.Dependencies {
		if !depNameRe.MatchString(dep) || strings.Contains(dep, "..") {
			return fmt.Errorf("dependency %q is not a valid package name (@org/name)", dep)
		}
		if dep == self {
			return fmt.Errorf("package %q cannot depend on itself", self)
		}
		if !ValidRange(rng) {
			return fmt.Errorf("dependency %q has an invalid version range %q", dep, rng)
		}
	}
	return nil
}

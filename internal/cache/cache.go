package cache

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
)

const cacheFile = ".veld-cache.json"

// Cache tracks file content hashes for incremental generation.
// It is stored as .veld-cache.json inside the config directory.
//
// Uses SHA256 content hashing instead of mtime for reliability on CI systems
// (Docker, GitHub Actions) where file modification times may be reset.
type Cache struct {
	FileHashes map[string]string `json:"fileHashes"`

	// Legacy field — if present during Load, it is discarded and the cache
	// is rebuilt from scratch on the next generation cycle.
	LegacyMTimes map[string]int64 `json:"fileMTimes,omitempty"`
}

func newCache() *Cache {
	return &Cache{FileHashes: make(map[string]string)}
}

// Load reads the cache from dir. Returns an empty cache on any error.
// If the cache contains legacy mtime data, it is migrated to hash-based tracking.
func Load(dir string) *Cache {
	data, err := os.ReadFile(filepath.Join(dir, cacheFile))
	if err != nil {
		return newCache()
	}
	var c Cache
	if err := json.Unmarshal(data, &c); err != nil {
		return newCache()
	}

	// Migrate from legacy mtime-based cache
	if len(c.LegacyMTimes) > 0 {
		c.LegacyMTimes = nil // discard legacy data
		c.FileHashes = make(map[string]string)
		return &c
	}

	if c.FileHashes == nil {
		c.FileHashes = make(map[string]string)
	}
	return &c
}

// Save writes the cache to dir.
func (c *Cache) Save(dir string) error {
	c.LegacyMTimes = nil
	data, err := json.MarshalIndent(c, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(dir, cacheFile), data, 0644)
}

// HasChanged returns true if the file's content hash differs from the cached value.
func (c *Cache) HasChanged(path string) bool {
	hash, err := hashFile(path)
	if err != nil {
		return true
	}
	cached, ok := c.FileHashes[path]
	return !ok || hash != cached
}

// Update records the current content hash for the given file.
func (c *Cache) Update(path string) {
	hash, err := hashFile(path)
	if err != nil {
		return
	}
	c.FileHashes[path] = hash
}

// ChangedFiles returns the subset of paths whose content hash differs from the cache.
func (c *Cache) ChangedFiles(paths []string) []string {
	var out []string
	for _, p := range paths {
		if c.HasChanged(p) {
			out = append(out, p)
		}
	}
	return out
}

// hashFile computes the SHA256 hex digest of the file at path.
func hashFile(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	sum := sha256.Sum256(data)
	return hex.EncodeToString(sum[:]), nil
}

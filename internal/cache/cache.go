package cache

import (
	"encoding/json"
	"os"
	"path/filepath"
)

const cacheFile = ".veld-cache.json"

// Cache tracks file modification timestamps for incremental generation.
// It is stored as .veld-cache.json inside the config directory.
type Cache struct {
	FileMTimes map[string]int64 `json:"fileMTimes"`
}

func newCache() *Cache {
	return &Cache{FileMTimes: make(map[string]int64)}
}

// Load reads the cache from dir. Returns an empty cache on any error.
func Load(dir string) *Cache {
	data, err := os.ReadFile(filepath.Join(dir, cacheFile))
	if err != nil {
		return newCache()
	}
	var c Cache
	if err := json.Unmarshal(data, &c); err != nil {
		return newCache()
	}
	if c.FileMTimes == nil {
		c.FileMTimes = make(map[string]int64)
	}
	return &c
}

// Save writes the cache to dir.
func (c *Cache) Save(dir string) error {
	data, err := json.MarshalIndent(c, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(dir, cacheFile), data, 0644)
}

// HasChanged returns true if the file's mtime differs from the cached value.
func (c *Cache) HasChanged(path string) bool {
	info, err := os.Stat(path)
	if err != nil {
		return true // treat unreadable files as changed
	}
	cached, ok := c.FileMTimes[path]
	return !ok || info.ModTime().UnixNano() != cached
}

// Update records the current mtime for the given file.
func (c *Cache) Update(path string) {
	info, err := os.Stat(path)
	if err != nil {
		return
	}
	c.FileMTimes[path] = info.ModTime().UnixNano()
}

// ChangedFiles returns the subset of paths whose mtime differs from the cache.
func (c *Cache) ChangedFiles(paths []string) []string {
	var out []string
	for _, p := range paths {
		if c.HasChanged(p) {
			out = append(out, p)
		}
	}
	return out
}

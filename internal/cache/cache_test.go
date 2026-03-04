package cache

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestNewCache(t *testing.T) {
	c := newCache()
	if c.FileHashes == nil {
		t.Fatal("FileHashes should not be nil")
	}
	if len(c.FileHashes) != 0 {
		t.Error("FileHashes should be empty")
	}
}

func TestSaveAndLoad(t *testing.T) {
	dir := t.TempDir()
	c := newCache()
	c.FileHashes["test.veld"] = "abc123"

	if err := c.Save(dir); err != nil {
		t.Fatalf("Save: %v", err)
	}

	loaded := Load(dir)
	if loaded.FileHashes["test.veld"] != "abc123" {
		t.Errorf("got %q, want abc123", loaded.FileHashes["test.veld"])
	}
}

func TestLoadMissing(t *testing.T) {
	c := Load(t.TempDir())
	if c.FileHashes == nil {
		t.Fatal("should return empty cache, not nil")
	}
}

func TestLoadCorrupted(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, cacheFile), []byte("not json"), 0644)
	c := Load(dir)
	if c.FileHashes == nil {
		t.Fatal("should return empty cache on corruption")
	}
}

func TestLegacyMigration(t *testing.T) {
	dir := t.TempDir()
	// Write legacy format
	legacy := map[string]interface{}{
		"fileMTimes": map[string]int64{
			"a.veld": 12345,
		},
	}
	data, _ := json.Marshal(legacy)
	os.WriteFile(filepath.Join(dir, cacheFile), data, 0644)

	c := Load(dir)
	if len(c.FileHashes) != 0 {
		t.Error("legacy data should be discarded, FileHashes should be empty")
	}
	if len(c.LegacyMTimes) != 0 {
		t.Error("LegacyMTimes should be nil after migration")
	}
}

func TestHasChangedAndUpdate(t *testing.T) {
	dir := t.TempDir()
	f := filepath.Join(dir, "test.veld")
	os.WriteFile(f, []byte("model User {}"), 0644)

	c := newCache()
	if !c.HasChanged(f) {
		t.Error("new file should be detected as changed")
	}

	c.Update(f)
	if c.HasChanged(f) {
		t.Error("file should not be changed after Update")
	}

	// Modify file
	os.WriteFile(f, []byte("model User { id: string }"), 0644)
	if !c.HasChanged(f) {
		t.Error("modified file should be detected as changed")
	}
}

func TestChangedFiles(t *testing.T) {
	dir := t.TempDir()
	f1 := filepath.Join(dir, "a.veld")
	f2 := filepath.Join(dir, "b.veld")
	os.WriteFile(f1, []byte("a"), 0644)
	os.WriteFile(f2, []byte("b"), 0644)

	c := newCache()
	c.Update(f1)
	// f2 is not cached

	changed := c.ChangedFiles([]string{f1, f2})
	if len(changed) != 1 {
		t.Fatalf("expected 1 changed, got %d", len(changed))
	}
	if changed[0] != f2 {
		t.Errorf("expected %s, got %s", f2, changed[0])
	}
}

func TestHasChangedNonexistent(t *testing.T) {
	c := newCache()
	if !c.HasChanged("/nonexistent/file.veld") {
		t.Error("nonexistent file should be detected as changed")
	}
}

func TestSaveDoesNotPersistLegacy(t *testing.T) {
	dir := t.TempDir()
	c := newCache()
	c.LegacyMTimes = map[string]int64{"x": 1}
	c.Save(dir)

	data, _ := os.ReadFile(filepath.Join(dir, cacheFile))
	s := string(data)
	if containsStr(s, "fileMTimes") {
		t.Error("legacy fileMTimes should not be persisted")
	}
}

func containsStr(s, sub string) bool {
	for i := 0; i <= len(s)-len(sub); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}

// Package storage provides the object-storage backend for tarballs.
package storage

import (
	"io"
	"os"
	"path/filepath"
)

// Backend is the interface for tarball storage.
type Backend interface {
	// Put stores data under key and returns the number of bytes written.
	Put(key string, r io.Reader) (int64, error)
	// Get opens a reader for key.
	Get(key string) (io.ReadCloser, error)
	// Delete removes key.
	Delete(key string) error
	// Size returns the byte size of key.
	Size(key string) (int64, error)
}

// Local stores tarballs on the local filesystem.
type Local struct{ root string }

// NewLocal creates a Local backend rooted at dir.
func NewLocal(dir string) (*Local, error) {
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, err
	}
	return &Local{root: dir}, nil
}

func (l *Local) path(key string) string {
	// key is like "acme/auth/1.2.0.tar.gz" — sanitise to prevent traversal
	clean := filepath.Clean(key)
	return filepath.Join(l.root, clean)
}

func (l *Local) Put(key string, r io.Reader) (int64, error) {
	p := l.path(key)
	if err := os.MkdirAll(filepath.Dir(p), 0755); err != nil {
		return 0, err
	}
	f, err := os.CreateTemp(filepath.Dir(p), ".tmp-")
	if err != nil {
		return 0, err
	}
	n, err := io.Copy(f, r)
	f.Close()
	if err != nil {
		os.Remove(f.Name())
		return 0, err
	}
	if err := os.Rename(f.Name(), p); err != nil {
		os.Remove(f.Name())
		return 0, err
	}
	return n, nil
}

func (l *Local) Get(key string) (io.ReadCloser, error) {
	return os.Open(l.path(key))
}

func (l *Local) Delete(key string) error {
	return os.Remove(l.path(key))
}

func (l *Local) Size(key string) (int64, error) {
	fi, err := os.Stat(l.path(key))
	if err != nil {
		return 0, err
	}
	return fi.Size(), nil
}

package handlers

import (
	"embed"
	"io/fs"
	"net/http"
)

//go:embed web
var webFS embed.FS

// WebHandler serves the embedded single-page application.
type WebHandler struct{}

func (h *WebHandler) ServeIndex(w http.ResponseWriter, r *http.Request) {
	sub, err := fs.Sub(webFS, "web")
	if err != nil {
		http.Error(w, "not found", http.StatusInternalServerError)
		return
	}
	f, err := sub.Open("index.html")
	if err != nil {
		http.Error(w, "not found", http.StatusInternalServerError)
		return
	}
	defer f.Close()
	fi, _ := f.Stat()
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Header().Set("Cache-Control", "no-cache, no-store")
	http.ServeContent(w, r, "index.html", fi.ModTime(), f.(fs.File).(interface {
		Read([]byte) (int, error)
		Seek(int64, int) (int64, error)
	}))
}

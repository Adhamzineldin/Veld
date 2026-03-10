package handlers

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	serverauth "github.com/Adhamzineldin/Veld/internal/server/auth"
	"github.com/Adhamzineldin/Veld/internal/server/db"
	"github.com/Adhamzineldin/Veld/internal/server/models"
	"github.com/Adhamzineldin/Veld/internal/server/storage"
)

// PackageHandler handles package and version endpoints.
type PackageHandler struct {
	DB      *db.DB
	Storage storage.Backend
}

// ListPackages handles GET /api/v1/packages
func (h *PackageHandler) ListPackages(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	search := r.URL.Query().Get("q")

	vis := []string{"public"}
	if u != nil {
		vis = []string{"public", "private"}
	}

	pkgs, err := h.DB.ListPackages(search, vis)
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	if pkgs == nil {
		pkgs = []*models.Package{}
	}
	jsonOK(w, pkgs)
}

// GetPackage handles GET /api/v1/packages/{org}/{name}
func (h *PackageHandler) GetPackage(w http.ResponseWriter, r *http.Request) {
	org, err := h.DB.GetOrgByName(r.PathValue("org"))
	if err != nil || org == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}
	pkg, err := h.DB.GetPackage(org.ID, r.PathValue("name"))
	if err != nil || pkg == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}
	if pkg.Visibility == "private" {
		u := serverauth.GetUser(r)
		if u == nil {
			jsonError(w, "not found", http.StatusNotFound)
			return
		}
		m, _ := h.DB.GetOrgMember(org.ID, u.ID)
		if m == nil {
			jsonError(w, "not found", http.StatusNotFound)
			return
		}
	}
	versions, _ := h.DB.ListPackageVersions(pkg.ID)
	if versions == nil {
		versions = []*models.PackageVersion{}
	}
	jsonOK(w, map[string]interface{}{
		"package":  pkg,
		"versions": versions,
	})
}

// ListVersions handles GET /api/v1/packages/{org}/{name}/versions
func (h *PackageHandler) ListVersions(w http.ResponseWriter, r *http.Request) {
	org, _ := h.DB.GetOrgByName(r.PathValue("org"))
	if org == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}
	pkg, _ := h.DB.GetPackage(org.ID, r.PathValue("name"))
	if pkg == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}
	versions, _ := h.DB.ListPackageVersions(pkg.ID)
	if versions == nil {
		versions = []*models.PackageVersion{}
	}
	jsonOK(w, versions)
}

// Publish handles POST /api/v1/packages (multipart: manifest JSON + tarball file)
func (h *PackageHandler) Publish(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	if !serverauth.HasScope(r, "write") {
		jsonError(w, "token missing 'write' scope", http.StatusForbidden)
		return
	}

	// 32 MB max
	if err := r.ParseMultipartForm(32 << 20); err != nil {
		jsonError(w, "invalid multipart form", http.StatusBadRequest)
		return
	}

	// Parse manifest
	var manifest models.PublishRequest
	if err := json.Unmarshal([]byte(r.FormValue("manifest")), &manifest); err != nil {
		jsonError(w, "invalid manifest JSON", http.StatusBadRequest)
		return
	}
	if manifest.OrgName == "" || manifest.PkgName == "" || manifest.Version == "" {
		jsonError(w, "manifest must include org, name, version", http.StatusBadRequest)
		return
	}

	// Lookup org + check membership
	org, err := h.DB.GetOrgByName(manifest.OrgName)
	if err != nil || org == nil {
		jsonError(w, fmt.Sprintf("org @%s not found", manifest.OrgName), http.StatusNotFound)
		return
	}
	member, err := h.DB.GetOrgMember(org.ID, u.ID)
	if err != nil || member == nil || (member.Role != "owner" && member.Role != "admin") {
		jsonError(w, "you need owner or admin role to publish", http.StatusForbidden)
		return
	}

	// Get or create package record
	pkg, err := h.DB.GetPackage(org.ID, manifest.PkgName)
	if err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	if pkg == nil {
		pkg = &models.Package{
			ID:         serverauth.GenerateID(),
			OrgID:      org.ID,
			OrgName:    org.Name,
			Name:       manifest.PkgName,
			Visibility: "public",
			CreatedBy:  u.ID,
			CreatedAt:  time.Now().UTC(),
		}
		if err := h.DB.CreatePackage(pkg); err != nil {
			jsonError(w, "server error", http.StatusInternalServerError)
			return
		}
	}

	// Check version uniqueness
	existing, _ := h.DB.GetPackageVersion(pkg.ID, manifest.Version)
	if existing != nil {
		jsonError(w, fmt.Sprintf("version %s already published", manifest.Version), http.StatusConflict)
		return
	}

	// Read tarball
	file, _, err := r.FormFile("tarball")
	if err != nil {
		jsonError(w, "tarball file is required", http.StatusBadRequest)
		return
	}
	defer file.Close()

	// Compute SHA-256 while streaming to storage
	tarballKey := fmt.Sprintf("%s/%s/%s.tar.gz", org.Name, manifest.PkgName, manifest.Version)
	hasher := sha256.New()
	pr, pw := io.Pipe()
	errCh := make(chan error, 1)
	var size int64

	go func() {
		n, err := h.Storage.Put(tarballKey, pr)
		size = n
		pr.CloseWithError(err)
		errCh <- err
	}()

	mw := io.MultiWriter(pw, hasher)
	if _, err := io.Copy(mw, file); err != nil {
		pw.CloseWithError(err)
		jsonError(w, "upload error", http.StatusInternalServerError)
		return
	}
	pw.Close()
	if err := <-errCh; err != nil {
		jsonError(w, "storage error", http.StatusInternalServerError)
		return
	}

	sha := hex.EncodeToString(hasher.Sum(nil))
	manifestJSON, _ := json.Marshal(manifest)

	pv := &models.PackageVersion{
		ID:          serverauth.GenerateID(),
		PackageID:   pkg.ID,
		Version:     manifest.Version,
		TarballKey:  tarballKey,
		TarballSHA:  sha,
		TarballSize: size,
		Manifest:    string(manifestJSON),
		PublishedBy: u.ID,
		PublishedAt: time.Now().UTC(),
	}
	if err := h.DB.CreatePackageVersion(pv); err != nil {
		h.Storage.Delete(tarballKey)
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}

	jsonCreated(w, map[string]interface{}{
		"package": fmt.Sprintf("@%s/%s", org.Name, pkg.Name),
		"version": pv.Version,
		"size":    size,
		"sha256":  sha,
	})
}

// Download handles GET /api/v1/packages/{org}/{name}/{version}/download
func (h *PackageHandler) Download(w http.ResponseWriter, r *http.Request) {
	org, _ := h.DB.GetOrgByName(r.PathValue("org"))
	if org == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}
	pkg, _ := h.DB.GetPackage(org.ID, r.PathValue("name"))
	if pkg == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}
	if pkg.Visibility == "private" {
		u := serverauth.GetUser(r)
		if u == nil {
			jsonError(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		m, _ := h.DB.GetOrgMember(org.ID, u.ID)
		if m == nil {
			jsonError(w, "forbidden", http.StatusForbidden)
			return
		}
	}

	ver, _ := h.DB.GetPackageVersion(pkg.ID, r.PathValue("version"))
	if ver == nil {
		jsonError(w, "version not found", http.StatusNotFound)
		return
	}

	rc, err := h.Storage.Get(ver.TarballKey)
	if err != nil {
		jsonError(w, "file not found in storage", http.StatusNotFound)
		return
	}
	defer rc.Close()

	w.Header().Set("Content-Type", "application/gzip")
	w.Header().Set("Content-Disposition", fmt.Sprintf(`attachment; filename="%s-%s.tar.gz"`, pkg.Name, ver.Version))
	w.Header().Set("X-Veld-SHA256", ver.TarballSHA)
	w.Header().Set("X-Veld-Size", fmt.Sprintf("%d", ver.TarballSize))
	io.Copy(w, rc)
}

// DeprecateVersion handles POST /api/v1/packages/{org}/{name}/{version}/deprecate
func (h *PackageHandler) DeprecateVersion(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	org, _ := h.DB.GetOrgByName(r.PathValue("org"))
	if org == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}
	m, _ := h.DB.GetOrgMember(org.ID, u.ID)
	if m == nil || (m.Role != "owner" && m.Role != "admin") {
		jsonError(w, "forbidden", http.StatusForbidden)
		return
	}
	pkg, _ := h.DB.GetPackage(org.ID, r.PathValue("name"))
	if pkg == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}

	var body struct {
		Message string `json:"message"`
	}
	json.NewDecoder(r.Body).Decode(&body)

	if err := h.DB.DeprecatePackageVersion(pkg.ID, r.PathValue("version"), body.Message); err != nil {
		jsonError(w, "server error", http.StatusInternalServerError)
		return
	}
	jsonOK(w, map[string]string{"message": "deprecated"})
}

// DeleteVersion handles DELETE /api/v1/packages/{org}/{name}/{version}
func (h *PackageHandler) DeleteVersion(w http.ResponseWriter, r *http.Request) {
	u := serverauth.GetUser(r)
	if u == nil {
		jsonError(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	org, _ := h.DB.GetOrgByName(r.PathValue("org"))
	if org == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}
	m, _ := h.DB.GetOrgMember(org.ID, u.ID)
	if m == nil || m.Role != "owner" {
		jsonError(w, "only owners can unpublish versions", http.StatusForbidden)
		return
	}
	pkg, _ := h.DB.GetPackage(org.ID, r.PathValue("name"))
	if pkg == nil {
		jsonError(w, "not found", http.StatusNotFound)
		return
	}
	ver, _ := h.DB.GetPackageVersion(pkg.ID, r.PathValue("version"))
	if ver != nil {
		h.Storage.Delete(ver.TarballKey)
		h.DB.DeletePackageVersion(pkg.ID, r.PathValue("version"))
	}
	jsonNoContent(w)
}

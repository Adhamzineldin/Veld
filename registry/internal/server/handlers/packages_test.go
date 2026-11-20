package handlers

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"context"
	serverauth "github.com/Adhamzineldin/Veld/registry/internal/server/auth"
	"github.com/Adhamzineldin/Veld/registry/internal/server/models"
)

type entry struct {
	name string
	body string
	typ  byte
}

func makeTarball(t *testing.T, entries []entry) []byte {
	t.Helper()
	var buf bytes.Buffer
	gz := gzip.NewWriter(&buf)
	tw := tar.NewWriter(gz)
	for _, e := range entries {
		typ := e.typ
		if typ == 0 {
			typ = tar.TypeReg
		}
		hdr := &tar.Header{Name: e.name, Typeflag: typ, Mode: 0o644, Size: int64(len(e.body))}
		if typ != tar.TypeReg {
			hdr.Size = 0
			hdr.Linkname = "x"
		}
		if err := tw.WriteHeader(hdr); err != nil {
			t.Fatal(err)
		}
		if typ == tar.TypeReg {
			if _, err := tw.Write([]byte(e.body)); err != nil {
				t.Fatal(err)
			}
		}
	}
	tw.Close()
	gz.Close()
	return buf.Bytes()
}

func sum(s string) string {
	h := sha256.Sum256([]byte(s))
	return hex.EncodeToString(h[:])
}

func goodEntries() []entry {
	src := "model A { id: string }\n"
	manifest := fmt.Sprintf("name = \"@acme/lib\"\nversion = \"1.0.0\"\nlanguage = \"2\"\ndescription = \"d\"\n\n[files]\n\"contracts/a.veld\" = \"%s\"\n", sum(src))
	return []entry{{name: manifestFileName, body: manifest}, {name: "contracts/a.veld", body: src}}
}

func TestValidatePackageAccepts(t *testing.T) {
	m, err := ValidatePackage("acme", "lib", "1.0.0", bytes.NewReader(makeTarball(t, goodEntries())))
	if err != nil {
		t.Fatal(err)
	}
	if m.Name != "@acme/lib" {
		t.Fatalf("bad manifest %+v", m)
	}
}

func TestValidatePackageRejects(t *testing.T) {
	replaceManifest := func(old, new string) []entry {
		e := goodEntries()
		e[0].body = strings.Replace(e[0].body, old, new, 1)
		return e
	}
	cases := map[string][]entry{
		"v1 language":      replaceManifest(`language = "2"`, `language = "1"`),
		"missing language": replaceManifest(`language = "2"`, ``),
		"wrong name":       replaceManifest("@acme/lib", "@acme/other"),
		"wrong version":    replaceManifest("1.0.0", "1.0.1"),
		"bad checksum":     replaceManifest(sum("model A { id: string }\n"), strings.Repeat("0", 64)),
		"non hex checksum": replaceManifest(sum("model A { id: string }\n"), "zzz"),
		"no manifest":      goodEntries()[1:],
		"unlisted file":    append(goodEntries(), entry{name: "contracts/b.veld", body: "x"}),
		"traversal":        append(goodEntries(), entry{name: "../evil", body: "x"}),
		"absolute":         append(goodEntries(), entry{name: "/evil", body: "x"}),
		"backslash":        append(goodEntries(), entry{name: "a\b", body: "x"}),
		"drive":            append(goodEntries(), entry{name: "C:evil", body: "x"}),
		"symlink":          append(goodEntries(), entry{name: "link", typ: tar.TypeSymlink}),
		"invalid toml":     {{name: manifestFileName, body: "name = = ="}},
	}
	for name, es := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := ValidatePackage("acme", "lib", "1.0.0", bytes.NewReader(makeTarball(t, es))); err == nil {
				t.Fatal("expected rejection")
			}
		})
	}
	if _, err := ValidatePackage("acme", "lib", "1.0.0", strings.NewReader("not gzip")); err == nil {
		t.Fatal("expected gzip rejection")
	}
}

func TestValidateNames(t *testing.T) {
	bad := [][3]string{{"Acme", "lib", "1.0.0"}, {"acme", "../x", "1.0.0"}, {"acme", "a/b", "1.0.0"}, {"acme", "lib", "1.0"}, {"acme", "lib", "v1.0.0"}, {"ac/me", "lib", "1.0.0"}, {"acme", "lib", "1.0.0/../x"}}
	for _, b := range bad {
		if ValidateNames(b[0], b[1], b[2]) == nil {
			t.Errorf("expected rejection of %v", b)
		}
	}
	if err := ValidateNames("acme", "lib", "1.2.3-beta.1"); err != nil {
		t.Error(err)
	}
}

func publishRequest(t *testing.T, manifest string, tarball []byte, user *models.User, scopes []string) *http.Request {
	t.Helper()
	var body bytes.Buffer
	mw := multipart.NewWriter(&body)
	_ = mw.WriteField("manifest", manifest)
	if tarball != nil {
		fw, _ := mw.CreateFormFile("tarball", "p.tar.gz")
		fw.Write(tarball)
	}
	mw.Close()
	req := httptest.NewRequest(http.MethodPost, "/api/v1/packages", &body)
	req.Header.Set("Content-Type", mw.FormDataContentType())
	ctx := req.Context()
	if user != nil {
		ctx = context.WithValue(ctx, serverauth.ContextUser, user)
		if scopes != nil {
			ctx = context.WithValue(ctx, serverauth.ContextToken, &models.Token{Scopes: scopes})
		}
	}
	return req.WithContext(ctx)
}

// The handler is driven with a nil DB: every case below must be rejected
// before any database access happens.
func TestPublishRejectsBeforeDatabase(t *testing.T) {
	h := &PackageHandler{}
	user := &models.User{ID: "u1"}
	good := makeTarball(t, goodEntries())
	okManifest := `{"org":"acme","name":"lib","version":"1.0.0"}`
	cases := []struct {
		name     string
		user     *models.User
		scopes   []string
		manifest string
		tarball  []byte
		want     int
	}{
		{"anonymous", nil, nil, okManifest, good, http.StatusUnauthorized},
		{"read-only token", user, []string{"read"}, okManifest, good, http.StatusForbidden},
		{"bad manifest json", user, nil, "{", good, http.StatusBadRequest},
		{"missing fields", user, nil, `{"org":"acme"}`, good, http.StatusBadRequest},
		{"traversal name", user, nil, `{"org":"acme","name":"../x","version":"1.0.0"}`, good, http.StatusBadRequest},
		{"bad semver", user, nil, `{"org":"acme","name":"lib","version":"one"}`, good, http.StatusBadRequest},
		{"no tarball", user, nil, okManifest, nil, http.StatusBadRequest},
		{"not gzip", user, nil, okManifest, []byte("junk"), http.StatusUnprocessableEntity},
		{"name mismatch", user, nil, `{"org":"acme","name":"other","version":"1.0.0"}`, good, http.StatusUnprocessableEntity},
		{"v1 package", user, nil, okManifest, makeTarball(t, []entry{{name: "veld.config.json", body: "{}"}}), http.StatusUnprocessableEntity},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			rec := httptest.NewRecorder()
			h.Publish(rec, publishRequest(t, c.manifest, c.tarball, c.user, c.scopes))
			if rec.Code != c.want {
				t.Fatalf("status %d, want %d: %s", rec.Code, c.want, rec.Body.String())
			}
		})
	}
}

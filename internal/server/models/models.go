// Package models defines all shared data structures for the Veld Registry server.
package models

import "time"

// User represents a registry account.
type User struct {
	ID           string    `json:"id"`
	Email        string    `json:"email"`
	Username     string    `json:"username"`
	PasswordHash string    `json:"-"`
	CreatedAt    time.Time `json:"created_at"`
}

// Org represents a registry organisation (the @acme scope).
type Org struct {
	ID          string    `json:"id"`
	Name        string    `json:"name"`
	DisplayName string    `json:"display_name"`
	Description string    `json:"description"`
	CreatedAt   time.Time `json:"created_at"`
}

// OrgMember links a user to an org with a role.
type OrgMember struct {
	OrgID    string `json:"org_id"`
	UserID   string `json:"user_id"`
	Username string `json:"username,omitempty"`
	Email    string `json:"email,omitempty"`
	Role     string `json:"role"` // owner | admin | member
}

// Package is a published veld contract package.
type Package struct {
	ID            string    `json:"id"`
	OrgID         string    `json:"org_id"`
	OrgName       string    `json:"org_name,omitempty"`
	Name          string    `json:"name"`
	Description   string    `json:"description"`
	Visibility    string    `json:"visibility"` // public | private
	CreatedBy     string    `json:"created_by"`
	CreatedAt     time.Time `json:"created_at"`
	LatestVersion string    `json:"latest_version,omitempty"`
	VersionCount  int       `json:"version_count,omitempty"`
}

// PackageVersion is a specific semver release of a package.
type PackageVersion struct {
	ID          string    `json:"id"`
	PackageID   string    `json:"package_id"`
	PackageName string    `json:"package_name,omitempty"`
	OrgName     string    `json:"org_name,omitempty"`
	Version     string    `json:"version"`
	TarballKey  string    `json:"tarball_key"`
	TarballSHA  string    `json:"tarball_sha256"`
	TarballSize int64     `json:"tarball_size"`
	Manifest    string    `json:"manifest"` // raw JSON of veld.config.json
	Deprecated  string    `json:"deprecated,omitempty"`
	PublishedBy string    `json:"published_by,omitempty"`
	PublishedAt time.Time `json:"published_at"`
}

// Token is an API authentication token.
type Token struct {
	ID        string     `json:"id"`
	UserID    string     `json:"user_id"`
	OrgID     string     `json:"org_id,omitempty"`
	Name      string     `json:"name"`
	TokenHash string     `json:"-"`
	Scopes    []string   `json:"scopes"`
	ExpiresAt *time.Time `json:"expires_at,omitempty"`
	LastUsed  *time.Time `json:"last_used,omitempty"`
	CreatedAt time.Time  `json:"created_at"`
	// Plain token — only populated on creation, never stored.
	PlainToken string `json:"token,omitempty"`
}

// PublishRequest is parsed from a multipart publish upload.
type PublishRequest struct {
	OrgName  string `json:"org"`
	PkgName  string `json:"name"`
	Version  string `json:"version"`
	Manifest string `json:"manifest"`
}

// Semver helpers ─────────────────────────────────────────────────────────────

// ParseSemver returns (major, minor, patch, pre-release) from "1.2.3" or "1.2.3-beta".
func ParseSemver(v string) (int, int, int, string) {
	// Strip leading 'v'
	if len(v) > 0 && v[0] == 'v' {
		v = v[1:]
	}
	var major, minor, patch int
	var pre string
	// Split off pre-release
	for i, c := range v {
		if c == '-' {
			pre = v[i+1:]
			v = v[:i]
			break
		}
	}
	parts := splitDots(v)
	if len(parts) >= 1 {
		major = atoi(parts[0])
	}
	if len(parts) >= 2 {
		minor = atoi(parts[1])
	}
	if len(parts) >= 3 {
		patch = atoi(parts[2])
	}
	return major, minor, patch, pre
}

// CompareSemver returns -1, 0, or +1.
func CompareSemver(a, b string) int {
	ma, na, pa, prea := ParseSemver(a)
	mb, nb, pb, preb := ParseSemver(b)
	if ma != mb {
		return cmp(ma, mb)
	}
	if na != nb {
		return cmp(na, nb)
	}
	if pa != pb {
		return cmp(pa, pb)
	}
	// pre-release < release
	if prea == "" && preb != "" {
		return 1
	}
	if prea != "" && preb == "" {
		return -1
	}
	if prea < preb {
		return -1
	}
	if prea > preb {
		return 1
	}
	return 0
}

func cmp(a, b int) int {
	if a < b {
		return -1
	}
	return 1
}

func splitDots(s string) []string {
	var out []string
	cur := ""
	for _, c := range s {
		if c == '.' {
			out = append(out, cur)
			cur = ""
		} else {
			cur += string(c)
		}
	}
	out = append(out, cur)
	return out
}

func atoi(s string) int {
	n := 0
	for _, c := range s {
		if c < '0' || c > '9' {
			break
		}
		n = n*10 + int(c-'0')
	}
	return n
}

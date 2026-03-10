// Package db provides the PostgreSQL database layer for the Veld Registry.
package db

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/Adhamzineldin/Veld/internal/server/models"
	_ "github.com/lib/pq"
)

// DB wraps *sql.DB with all registry query methods.
type DB struct{ conn *sql.DB }

// Open connects to PostgreSQL at dsn and runs schema migrations.
// dsn example: "postgres://user:pass@localhost:5432/veld?sslmode=disable"
func Open(dsn string) (*DB, error) {
	conn, err := sql.Open("postgres", dsn)
	if err != nil {
		return nil, err
	}
	conn.SetMaxOpenConns(25)
	conn.SetMaxIdleConns(5)
	conn.SetConnMaxLifetime(5 * time.Minute)
	if err := conn.Ping(); err != nil {
		return nil, fmt.Errorf("cannot reach postgres: %w", err)
	}
	d := &DB{conn: conn}
	if err := d.migrate(); err != nil {
		return nil, fmt.Errorf("migrate: %w", err)
	}
	return d, nil
}

// Close shuts down the connection pool.
func (d *DB) Close() error { return d.conn.Close() }

// ── Schema ────────────────────────────────────────────────────────────────────

func (d *DB) migrate() error {
	_, err := d.conn.Exec(schema)
	return err
}

const schema = `
CREATE TABLE IF NOT EXISTS users (
  id            TEXT PRIMARY KEY,
  email         TEXT UNIQUE NOT NULL,
  username      TEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS orgs (
  id           TEXT PRIMARY KEY,
  name         TEXT UNIQUE NOT NULL,
  display_name TEXT NOT NULL DEFAULT '',
  description  TEXT NOT NULL DEFAULT '',
  created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS org_members (
  org_id  TEXT NOT NULL REFERENCES orgs(id)  ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role    TEXT NOT NULL CHECK (role IN ('owner','admin','member')),
  PRIMARY KEY (org_id, user_id)
);

CREATE TABLE IF NOT EXISTS packages (
  id          TEXT PRIMARY KEY,
  org_id      TEXT NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  visibility  TEXT NOT NULL DEFAULT 'public' CHECK (visibility IN ('public','private')),
  created_by  TEXT REFERENCES users(id),
  created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (org_id, name)
);

CREATE TABLE IF NOT EXISTS package_versions (
  id           TEXT PRIMARY KEY,
  package_id   TEXT NOT NULL REFERENCES packages(id) ON DELETE CASCADE,
  version      TEXT NOT NULL,
  tarball_key  TEXT NOT NULL,
  tarball_sha  TEXT NOT NULL,
  tarball_size BIGINT NOT NULL DEFAULT 0,
  manifest     TEXT NOT NULL DEFAULT '{}',
  deprecated   TEXT,
  published_by TEXT REFERENCES users(id),
  published_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (package_id, version)
);

CREATE TABLE IF NOT EXISTS tokens (
  id          TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  org_id      TEXT REFERENCES orgs(id) ON DELETE SET NULL,
  name        TEXT NOT NULL,
  token_hash  TEXT UNIQUE NOT NULL,
  scopes      TEXT NOT NULL DEFAULT '["read"]',
  expires_at  TIMESTAMPTZ,
  last_used   TIMESTAMPTZ,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_packages_org  ON packages(org_id);
CREATE INDEX IF NOT EXISTS idx_versions_pkg  ON package_versions(package_id);
CREATE INDEX IF NOT EXISTS idx_tokens_user   ON tokens(user_id);
`

// ── Users ─────────────────────────────────────────────────────────────────────

func (d *DB) CreateUser(u *models.User) error {
	_, err := d.conn.Exec(
		`INSERT INTO users(id,email,username,password_hash,created_at) VALUES($1,$2,$3,$4,$5)`,
		u.ID, u.Email, u.Username, u.PasswordHash, u.CreatedAt,
	)
	return err
}

func (d *DB) GetUserByEmail(email string) (*models.User, error) {
	return d.scanUser(d.conn.QueryRow(
		`SELECT id,email,username,password_hash,created_at FROM users WHERE email=$1`, email,
	))
}

func (d *DB) GetUserByUsername(username string) (*models.User, error) {
	return d.scanUser(d.conn.QueryRow(
		`SELECT id,email,username,password_hash,created_at FROM users WHERE username=$1`, username,
	))
}

func (d *DB) GetUserByID(id string) (*models.User, error) {
	return d.scanUser(d.conn.QueryRow(
		`SELECT id,email,username,password_hash,created_at FROM users WHERE id=$1`, id,
	))
}

func (d *DB) scanUser(row *sql.Row) (*models.User, error) {
	u := &models.User{}
	if err := row.Scan(&u.ID, &u.Email, &u.Username, &u.PasswordHash, &u.CreatedAt); err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	return u, nil
}

func (d *DB) CountUsers() (int, error) {
	var n int
	err := d.conn.QueryRow(`SELECT COUNT(*) FROM users`).Scan(&n)
	return n, err
}

// ── Orgs ──────────────────────────────────────────────────────────────────────

func (d *DB) CreateOrg(o *models.Org) error {
	_, err := d.conn.Exec(
		`INSERT INTO orgs(id,name,display_name,description,created_at) VALUES($1,$2,$3,$4,$5)`,
		o.ID, o.Name, o.DisplayName, o.Description, o.CreatedAt,
	)
	return err
}

func (d *DB) GetOrgByName(name string) (*models.Org, error) {
	o := &models.Org{}
	err := d.conn.QueryRow(
		`SELECT id,name,display_name,description,created_at FROM orgs WHERE name=$1`, name,
	).Scan(&o.ID, &o.Name, &o.DisplayName, &o.Description, &o.CreatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	return o, err
}

func (d *DB) GetOrgByID(id string) (*models.Org, error) {
	o := &models.Org{}
	err := d.conn.QueryRow(
		`SELECT id,name,display_name,description,created_at FROM orgs WHERE id=$1`, id,
	).Scan(&o.ID, &o.Name, &o.DisplayName, &o.Description, &o.CreatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	return o, err
}

func (d *DB) ListOrgs() ([]*models.Org, error) {
	rows, err := d.conn.Query(
		`SELECT id,name,display_name,description,created_at FROM orgs ORDER BY name`,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	return scanOrgs(rows)
}

func (d *DB) ListOrgsForUser(userID string) ([]*models.Org, error) {
	rows, err := d.conn.Query(`
		SELECT o.id,o.name,o.display_name,o.description,o.created_at
		FROM orgs o JOIN org_members m ON o.id=m.org_id
		WHERE m.user_id=$1 ORDER BY o.name`, userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	return scanOrgs(rows)
}

func scanOrgs(rows *sql.Rows) ([]*models.Org, error) {
	var out []*models.Org
	for rows.Next() {
		o := &models.Org{}
		if err := rows.Scan(&o.ID, &o.Name, &o.DisplayName, &o.Description, &o.CreatedAt); err != nil {
			return nil, err
		}
		out = append(out, o)
	}
	return out, rows.Err()
}

func (d *DB) AddOrgMember(orgID, userID, role string) error {
	_, err := d.conn.Exec(`
		INSERT INTO org_members(org_id,user_id,role) VALUES($1,$2,$3)
		ON CONFLICT(org_id,user_id) DO UPDATE SET role=EXCLUDED.role`,
		orgID, userID, role,
	)
	return err
}

func (d *DB) RemoveOrgMember(orgID, userID string) error {
	_, err := d.conn.Exec(
		`DELETE FROM org_members WHERE org_id=$1 AND user_id=$2`, orgID, userID,
	)
	return err
}

func (d *DB) GetOrgMember(orgID, userID string) (*models.OrgMember, error) {
	m := &models.OrgMember{}
	err := d.conn.QueryRow(
		`SELECT org_id,user_id,role FROM org_members WHERE org_id=$1 AND user_id=$2`, orgID, userID,
	).Scan(&m.OrgID, &m.UserID, &m.Role)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	return m, err
}

func (d *DB) ListOrgMembers(orgID string) ([]*models.OrgMember, error) {
	rows, err := d.conn.Query(`
		SELECT m.org_id,m.user_id,u.username,u.email,m.role
		FROM org_members m JOIN users u ON m.user_id=u.id
		WHERE m.org_id=$1 ORDER BY m.role,u.username`, orgID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []*models.OrgMember
	for rows.Next() {
		m := &models.OrgMember{}
		if err := rows.Scan(&m.OrgID, &m.UserID, &m.Username, &m.Email, &m.Role); err != nil {
			return nil, err
		}
		out = append(out, m)
	}
	return out, rows.Err()
}

func (d *DB) CountOrgOwners(orgID string) (int, error) {
	var n int
	err := d.conn.QueryRow(
		`SELECT COUNT(*) FROM org_members WHERE org_id=$1 AND role='owner'`, orgID,
	).Scan(&n)
	return n, err
}

// ── Packages ──────────────────────────────────────────────────────────────────

func (d *DB) CreatePackage(p *models.Package) error {
	_, err := d.conn.Exec(
		`INSERT INTO packages(id,org_id,name,description,visibility,created_by,created_at)
		 VALUES($1,$2,$3,$4,$5,$6,$7)`,
		p.ID, p.OrgID, p.Name, p.Description, p.Visibility, p.CreatedBy, p.CreatedAt,
	)
	return err
}

const packageSelectCols = `
	SELECT p.id, p.org_id, o.name, p.name, p.description, p.visibility, p.created_by, p.created_at,
	       COALESCE((SELECT v.version FROM package_versions v WHERE v.package_id=p.id ORDER BY v.published_at DESC LIMIT 1),''),
	       COALESCE((SELECT COUNT(*) FROM package_versions v WHERE v.package_id=p.id),0)
	FROM packages p JOIN orgs o ON p.org_id=o.id`

func scanPackage(row *sql.Row) (*models.Package, error) {
	p := &models.Package{}
	err := row.Scan(&p.ID, &p.OrgID, &p.OrgName, &p.Name, &p.Description,
		&p.Visibility, &p.CreatedBy, &p.CreatedAt, &p.LatestVersion, &p.VersionCount)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	return p, err
}

func scanPackages(rows *sql.Rows) ([]*models.Package, error) {
	var out []*models.Package
	for rows.Next() {
		p := &models.Package{}
		if err := rows.Scan(&p.ID, &p.OrgID, &p.OrgName, &p.Name, &p.Description,
			&p.Visibility, &p.CreatedBy, &p.CreatedAt, &p.LatestVersion, &p.VersionCount); err != nil {
			return nil, err
		}
		out = append(out, p)
	}
	return out, rows.Err()
}

func (d *DB) GetPackage(orgID, name string) (*models.Package, error) {
	return scanPackage(d.conn.QueryRow(
		packageSelectCols+` WHERE p.org_id=$1 AND p.name=$2`, orgID, name,
	))
}

func (d *DB) GetPackageByID(id string) (*models.Package, error) {
	return scanPackage(d.conn.QueryRow(
		packageSelectCols+` WHERE p.id=$1`, id,
	))
}

func (d *DB) ListPackages(search string, visibilities []string) ([]*models.Package, error) {
	// Build $N placeholders for IN clause
	placeholders := make([]string, len(visibilities))
	args := make([]interface{}, len(visibilities))
	for i, v := range visibilities {
		placeholders[i] = fmt.Sprintf("$%d", i+1)
		args[i] = v
	}
	q := packageSelectCols + ` WHERE p.visibility IN (` + strings.Join(placeholders, ",") + `)`

	if search != "" {
		n := len(args) + 1
		q += fmt.Sprintf(` AND (p.name ILIKE $%d OR o.name ILIKE $%d OR p.description ILIKE $%d)`, n, n, n)
		args = append(args, "%"+search+"%")
	}
	q += ` ORDER BY p.created_at DESC LIMIT 100`

	rows, err := d.conn.Query(q, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	return scanPackages(rows)
}

func (d *DB) ListPackagesForOrg(orgID string) ([]*models.Package, error) {
	rows, err := d.conn.Query(
		packageSelectCols+` WHERE p.org_id=$1 ORDER BY p.name`, orgID,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	return scanPackages(rows)
}

func (d *DB) CountPackages() (int, error) {
	var n int
	err := d.conn.QueryRow(`SELECT COUNT(*) FROM packages`).Scan(&n)
	return n, err
}

// ── Package Versions ──────────────────────────────────────────────────────────

func (d *DB) CreatePackageVersion(v *models.PackageVersion) error {
	_, err := d.conn.Exec(`
		INSERT INTO package_versions(id,package_id,version,tarball_key,tarball_sha,tarball_size,manifest,published_by,published_at)
		VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)`,
		v.ID, v.PackageID, v.Version, v.TarballKey, v.TarballSHA,
		v.TarballSize, v.Manifest, v.PublishedBy, v.PublishedAt,
	)
	return err
}

const versionSelectCols = `
	SELECT id,package_id,version,tarball_key,tarball_sha,tarball_size,manifest,deprecated,published_by,published_at
	FROM package_versions`

func scanVersion(row *sql.Row) (*models.PackageVersion, error) {
	pv := &models.PackageVersion{}
	var dep sql.NullString
	err := row.Scan(&pv.ID, &pv.PackageID, &pv.Version, &pv.TarballKey, &pv.TarballSHA,
		&pv.TarballSize, &pv.Manifest, &dep, &pv.PublishedBy, &pv.PublishedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if dep.Valid {
		pv.Deprecated = dep.String
	}
	return pv, nil
}

func (d *DB) GetPackageVersion(packageID, version string) (*models.PackageVersion, error) {
	return scanVersion(d.conn.QueryRow(
		versionSelectCols+` WHERE package_id=$1 AND version=$2`, packageID, version,
	))
}

func (d *DB) ListPackageVersions(packageID string) ([]*models.PackageVersion, error) {
	rows, err := d.conn.Query(
		versionSelectCols+` WHERE package_id=$1 ORDER BY published_at DESC`, packageID,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []*models.PackageVersion
	for rows.Next() {
		pv := &models.PackageVersion{}
		var dep sql.NullString
		if err := rows.Scan(&pv.ID, &pv.PackageID, &pv.Version, &pv.TarballKey, &pv.TarballSHA,
			&pv.TarballSize, &pv.Manifest, &dep, &pv.PublishedBy, &pv.PublishedAt); err != nil {
			return nil, err
		}
		if dep.Valid {
			pv.Deprecated = dep.String
		}
		out = append(out, pv)
	}
	return out, rows.Err()
}

func (d *DB) DeprecatePackageVersion(packageID, version, message string) error {
	_, err := d.conn.Exec(
		`UPDATE package_versions SET deprecated=$1 WHERE package_id=$2 AND version=$3`,
		message, packageID, version,
	)
	return err
}

func (d *DB) DeletePackageVersion(packageID, version string) error {
	_, err := d.conn.Exec(
		`DELETE FROM package_versions WHERE package_id=$1 AND version=$2`, packageID, version,
	)
	return err
}

// ── Tokens ────────────────────────────────────────────────────────────────────

func (d *DB) CreateToken(t *models.Token) error {
	scopes, _ := json.Marshal(t.Scopes)
	_, err := d.conn.Exec(`
		INSERT INTO tokens(id,user_id,org_id,name,token_hash,scopes,expires_at,created_at)
		VALUES($1,$2,$3,$4,$5,$6,$7,$8)`,
		t.ID, t.UserID, nullStr(t.OrgID), t.Name, t.TokenHash, string(scopes), t.ExpiresAt, t.CreatedAt,
	)
	return err
}

func (d *DB) GetTokenByHash(hash string) (*models.Token, error) {
	t := &models.Token{}
	var scopesJSON string
	var orgID sql.NullString
	err := d.conn.QueryRow(`
		SELECT id,user_id,org_id,name,token_hash,scopes,expires_at,last_used,created_at
		FROM tokens WHERE token_hash=$1`, hash,
	).Scan(&t.ID, &t.UserID, &orgID, &t.Name, &t.TokenHash, &scopesJSON,
		&t.ExpiresAt, &t.LastUsed, &t.CreatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if orgID.Valid {
		t.OrgID = orgID.String
	}
	json.Unmarshal([]byte(scopesJSON), &t.Scopes)
	return t, nil
}

func (d *DB) ListTokensForUser(userID string) ([]*models.Token, error) {
	rows, err := d.conn.Query(`
		SELECT id,user_id,org_id,name,token_hash,scopes,expires_at,last_used,created_at
		FROM tokens WHERE user_id=$1 ORDER BY created_at DESC`, userID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []*models.Token
	for rows.Next() {
		t := &models.Token{}
		var scopesJSON string
		var orgID sql.NullString
		if err := rows.Scan(&t.ID, &t.UserID, &orgID, &t.Name, &t.TokenHash, &scopesJSON,
			&t.ExpiresAt, &t.LastUsed, &t.CreatedAt); err != nil {
			return nil, err
		}
		if orgID.Valid {
			t.OrgID = orgID.String
		}
		json.Unmarshal([]byte(scopesJSON), &t.Scopes)
		out = append(out, t)
	}
	return out, rows.Err()
}

func (d *DB) DeleteToken(id, userID string) error {
	_, err := d.conn.Exec(
		`DELETE FROM tokens WHERE id=$1 AND user_id=$2`, id, userID,
	)
	return err
}

func (d *DB) TouchToken(id string) {
	d.conn.Exec(`UPDATE tokens SET last_used=NOW() WHERE id=$1`, id)
}

func (d *DB) CountTokens() (int, error) {
	var n int
	err := d.conn.QueryRow(`SELECT COUNT(*) FROM tokens`).Scan(&n)
	return n, err
}

// ── Stats ─────────────────────────────────────────────────────────────────────

type Stats struct {
	Users    int `json:"users"`
	Orgs     int `json:"orgs"`
	Packages int `json:"packages"`
	Versions int `json:"versions"`
}

func (d *DB) GetStats() (Stats, error) {
	s := Stats{}
	d.conn.QueryRow(`SELECT COUNT(*) FROM users`).Scan(&s.Users)
	d.conn.QueryRow(`SELECT COUNT(*) FROM orgs`).Scan(&s.Orgs)
	d.conn.QueryRow(`SELECT COUNT(*) FROM packages`).Scan(&s.Packages)
	d.conn.QueryRow(`SELECT COUNT(*) FROM package_versions`).Scan(&s.Versions)
	return s, nil
}

// ── helpers ───────────────────────────────────────────────────────────────────

func nullStr(s string) interface{} {
	if s == "" {
		return nil
	}
	return s
}

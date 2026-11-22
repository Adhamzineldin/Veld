# Veld Registry

The package registry server for Veld 2.0. It is a separate Go HTTP service (its own module);
the `veld` CLI talks to it through `veld login`, `veld add`, `veld publish` and `veld install`.

The registry stores v2 packages as opaque tarballs. On publish it only validates the v2 manifest
`veld-package.toml` at the tarball root:

```toml
name = "@org/name"        # must match the published org/name
version = "1.2.3"         # semver, must match the published version
language = "2"            # Veld 2 packages only
description = "optional"

[files]                   # sha256 of every file in the tarball except the manifest
"contracts/invoices.veld" = "<64 hex chars>"
```

It also enforces size limits (32 MB upload, 128 MB unpacked, 5000 entries), rejects non-regular
entries and unsafe paths, and checks the publisher is an owner or admin of the org.

## Configuration

Precedence (highest first): flags, environment variables, `registry.config.json`, defaults.
See `registry.config.example.json`.

| Setting | Flag | Env | Config key | Default |
|---|---|---|---|---|
| Listen address | `--addr` | `VELD_ADDR` | `addr` | `:8080` |
| PostgreSQL DSN (required) | `--dsn` | `VELD_DSN` | `dsn` | |
| Tarball directory | `--storage` | `VELD_STORAGE` | `storage` | `./packages` |
| JWT secret, 16+ chars (required) | `--secret` | `VELD_SECRET` | `secret` | |
| Public URL for email links | | `VELD_BASE_URL` | `base_url` | |
| SMTP | | `SMTP_HOST`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM` | `smtp.*` | disabled |

`--config path` selects a config file; otherwise `registry.config.json` in the working directory is used.
The schema is created automatically on first start.

## Running with PostgreSQL

```bash
createdb veld
export VELD_DSN='postgres://veld:secret@localhost:5432/veld?sslmode=disable'
export VELD_SECRET="$(openssl rand -hex 32)"
go run ./cmd/veld-registry --addr :8080
```

Terminate TLS in front of the registry (the CLI requires `https` except for localhost).
Create a user with `POST /api/v1/auth/register`, an org with `POST /api/v1/orgs`, and an API token
(`vtk_...`, scope `write` to publish) with `POST /api/v1/tokens`.

## Docker

```bash
docker build -t veld-registry .
docker run -p 8080:8080 -v veld-data:/data \
  -e VELD_DSN='postgres://veld:secret@db:5432/veld?sslmode=disable' \
  -e VELD_SECRET="$(openssl rand -hex 32)" veld-registry
```

## Tests

```bash
go vet ./... && go test ./...
```

// Package server wires up the Veld Registry HTTP server.
package server

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"time"

	serverauth "github.com/Adhamzineldin/Veld/internal/server/auth"
	"github.com/Adhamzineldin/Veld/internal/server/db"
	"github.com/Adhamzineldin/Veld/internal/server/email"
	"github.com/Adhamzineldin/Veld/internal/server/handlers"
	"github.com/Adhamzineldin/Veld/internal/server/storage"
)

// Config holds runtime configuration for the registry server.
type Config struct {
	Addr        string       // e.g. ":8080"
	DSN         string       // PostgreSQL DSN: postgres://user:pass@host/db?sslmode=disable
	StoragePath string       // path to tarball storage directory
	JWTSecret   string       // HMAC secret for session JWTs (min 16 chars)
	BaseURL     string       // public base URL for email links, e.g. "https://registry.example.com"
	Email       email.Config // optional SMTP settings
}

// Server is the assembled registry HTTP server.
type Server struct {
	cfg   Config
	db    *db.DB
	store storage.Backend
	mux   *http.ServeMux
}

// New creates and initialises a Server.
func New(cfg Config) (*Server, error) {
	if cfg.JWTSecret == "" || len(cfg.JWTSecret) < 16 {
		return nil, fmt.Errorf("JWT_SECRET must be at least 16 characters")
	}
	database, err := db.Open(cfg.DSN)
	if err != nil {
		return nil, fmt.Errorf("open db: %w", err)
	}
	store, err := storage.NewLocal(cfg.StoragePath)
	if err != nil {
		return nil, fmt.Errorf("open storage: %w", err)
	}

	s := &Server{cfg: cfg, db: database, store: store, mux: http.NewServeMux()}
	s.registerRoutes()
	return s, nil
}

// Start runs the HTTP server until ctx is cancelled.
func (s *Server) Start(ctx context.Context) error {
	srv := &http.Server{
		Addr:         s.cfg.Addr,
		Handler:      cors(logger(s.mux)),
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 120 * time.Second,
		IdleTimeout:  120 * time.Second,
	}
	log.Printf("Veld Registry listening on http://localhost%s", s.cfg.Addr)

	go func() {
		<-ctx.Done()
		shutCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		srv.Shutdown(shutCtx)
	}()

	if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		return err
	}
	return nil
}

// Close releases database resources.
func (s *Server) Close() { s.db.Close() }

// ── Route registration ─────────────────────────────────────────────────────────

func (s *Server) registerRoutes() {
	authH := &handlers.AuthHandler{DB: s.db, JWTSecret: s.cfg.JWTSecret, Email: s.cfg.Email, BaseURL: s.cfg.BaseURL}
	orgH := &handlers.OrgHandler{DB: s.db}
	pkgH := &handlers.PackageHandler{DB: s.db, Storage: s.store}
	webH := &handlers.WebHandler{}

	// ── Auth middleware factories ───────────────────────────────────────────
	requireAuth := serverauth.Middleware(s.db, s.cfg.JWTSecret, false)
	optionalAuth := serverauth.Middleware(s.db, s.cfg.JWTSecret, true)

	// ── Auth ───────────────────────────────────────────────────────────────
	s.mux.HandleFunc("POST /api/v1/auth/register", authH.Register)
	s.mux.HandleFunc("POST /api/v1/auth/login", authH.Login)
	s.mux.HandleFunc("POST /api/v1/auth/logout", authH.Logout)
	s.mux.Handle("GET /api/v1/auth/me", requireAuth(http.HandlerFunc(authH.Me)))
	s.mux.HandleFunc("POST /api/v1/auth/totp-login", authH.TOTPLogin)
	s.mux.Handle("POST /api/v1/auth/setup-totp", requireAuth(http.HandlerFunc(authH.SetupTOTP)))
	s.mux.Handle("POST /api/v1/auth/confirm-totp", requireAuth(http.HandlerFunc(authH.ConfirmTOTP)))
	s.mux.Handle("DELETE /api/v1/auth/totp", requireAuth(http.HandlerFunc(authH.DisableTOTP)))
	s.mux.HandleFunc("POST /api/v1/auth/verify-email", authH.VerifyEmail)
	s.mux.Handle("POST /api/v1/auth/resend-verification", requireAuth(http.HandlerFunc(authH.ResendVerification)))

	// ── Tokens ─────────────────────────────────────────────────────────────
	s.mux.Handle("GET /api/v1/tokens", requireAuth(http.HandlerFunc(authH.ListTokens)))
	s.mux.Handle("POST /api/v1/tokens", requireAuth(http.HandlerFunc(authH.CreateToken)))
	s.mux.Handle("DELETE /api/v1/tokens/{id}", requireAuth(http.HandlerFunc(authH.DeleteToken)))

	// ── Stats (public) ──────────────────────────────────────────────────────
	s.mux.HandleFunc("GET /api/v1/stats", authH.Stats)

	// ── Orgs ───────────────────────────────────────────────────────────────
	s.mux.Handle("GET /api/v1/orgs", optionalAuth(http.HandlerFunc(orgH.ListOrgs)))
	s.mux.Handle("POST /api/v1/orgs", requireAuth(http.HandlerFunc(orgH.CreateOrg)))
	s.mux.Handle("GET /api/v1/orgs/{org}", optionalAuth(http.HandlerFunc(orgH.GetOrg)))
	s.mux.Handle("POST /api/v1/orgs/{org}/members", requireAuth(http.HandlerFunc(orgH.AddMember)))
	s.mux.Handle("DELETE /api/v1/orgs/{org}/members/{username}", requireAuth(http.HandlerFunc(orgH.RemoveMember)))

	// ── Packages ───────────────────────────────────────────────────────────
	s.mux.Handle("GET /api/v1/packages", optionalAuth(http.HandlerFunc(pkgH.ListPackages)))
	s.mux.Handle("POST /api/v1/packages", requireAuth(http.HandlerFunc(pkgH.Publish)))
	s.mux.Handle("GET /api/v1/packages/{org}/{name}", optionalAuth(http.HandlerFunc(pkgH.GetPackage)))
	s.mux.Handle("GET /api/v1/packages/{org}/{name}/versions", optionalAuth(http.HandlerFunc(pkgH.ListVersions)))
	s.mux.Handle("GET /api/v1/packages/{org}/{name}/{version}/download",
		optionalAuth(http.HandlerFunc(pkgH.Download)))
	s.mux.Handle("POST /api/v1/packages/{org}/{name}/{version}/deprecate",
		requireAuth(http.HandlerFunc(pkgH.DeprecateVersion)))
	s.mux.Handle("DELETE /api/v1/packages/{org}/{name}/{version}",
		requireAuth(http.HandlerFunc(pkgH.DeleteVersion)))

	// ── Web UI ──────────────────────────────────────────────────────────────
	// All non-API routes serve the SPA index (hash routing handles the rest client-side)
	s.mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		webH.ServeIndex(w, r)
	})
}

// ── Middleware ─────────────────────────────────────────────────────────────────

func logger(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		next.ServeHTTP(w, r)
		log.Printf("%s %s %s", r.Method, r.URL.Path, time.Since(start))
	})
}

func cors(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET,POST,PUT,DELETE,OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Authorization,Content-Type")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

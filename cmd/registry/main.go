// Command registry is the Veld Registry server binary.
//
// Usage:
//
//	veld-registry --addr :8080 --dsn postgres://veld:secret@localhost/veld?sslmode=disable --storage ./packages --secret <32-char-string>
//
// Environment variables (override flags):
//
//	VELD_ADDR, VELD_DSN, VELD_STORAGE, VELD_SECRET
package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/Adhamzineldin/Veld/internal/server"
	"github.com/spf13/cobra"
)

var (
	addr        string
	dsn         string
	storagePath string
	jwtSecret   string
)

func main() {
	root := &cobra.Command{
		Use:   "veld-registry",
		Short: "Veld Registry — self-hostable contract package registry",
		Example: `  # Minimal self-hosted start
  veld-registry \
    --dsn "postgres://veld:secret@localhost/veld?sslmode=disable" \
    --secret "$(openssl rand -hex 32)"

  # Full flags
  veld-registry --addr :8080 --dsn $DATABASE_URL --storage ./packages --secret $VELD_SECRET`,
		RunE: runServer,
	}

	root.Flags().StringVar(&addr, "addr", env("VELD_ADDR", ":8080"), "listen address")
	root.Flags().StringVar(&dsn, "dsn", env("VELD_DSN", ""), "PostgreSQL DSN (postgres://user:pass@host/db?sslmode=disable)")
	root.Flags().StringVar(&storagePath, "storage", env("VELD_STORAGE", "./packages"), "tarball storage directory")
	root.Flags().StringVar(&jwtSecret, "secret", env("VELD_SECRET", ""), "JWT signing secret (min 16 chars)")

	if err := root.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func runServer(cmd *cobra.Command, args []string) error {
	if dsn == "" {
		return fmt.Errorf(
			"--dsn / VELD_DSN is required.\n\n" +
				"  Example: postgres://veld:secret@localhost/veld?sslmode=disable\n\n" +
				"  Quick local setup:\n" +
				"    createdb veld\n" +
				"    veld-registry --dsn \"postgres://localhost/veld?sslmode=disable\" --secret $(openssl rand -hex 32)",
		)
	}
	if jwtSecret == "" {
		return fmt.Errorf(
			"--secret / VELD_SECRET is required.\n\n" +
				"  Generate one with: openssl rand -hex 32",
		)
	}

	cfg := server.Config{
		Addr:        addr,
		DSN:         dsn,
		StoragePath: storagePath,
		JWTSecret:   jwtSecret,
	}

	srv, err := server.New(cfg)
	if err != nil {
		return fmt.Errorf("init server: %w", err)
	}
	defer srv.Close()

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	log.Printf("Veld Registry  →  http://localhost%s", addr)
	log.Printf("  Storage: %s", storagePath)
	log.Printf("  Web UI:  http://localhost%s/", addr)

	return srv.Start(ctx)
}

func env(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

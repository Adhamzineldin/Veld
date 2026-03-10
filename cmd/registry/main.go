// Command veld-registry is the Veld Registry server.
//
// Config is loaded from registry.config.json (current dir or --config flag).
// CLI flags and environment variables override config file values.
//
// Priority (highest → lowest): CLI flags > env vars > registry.config.json > defaults
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/Adhamzineldin/Veld/internal/server"
	"github.com/spf13/cobra"
)

// registryConfig mirrors registry.config.json on disk.
type registryConfig struct {
	Addr        string `json:"addr"`
	DSN         string `json:"dsn"`
	StoragePath string `json:"storage"`
	JWTSecret   string `json:"secret"`
}

func loadConfigFile(path string) registryConfig {
	data, err := os.ReadFile(path)
	if err != nil {
		return registryConfig{}
	}
	var c registryConfig
	if err := json.Unmarshal(data, &c); err != nil {
		log.Printf("warning: could not parse %s: %v", path, err)
	}
	return c
}

var (
	configFile  string
	flagAddr    string
	flagDSN     string
	flagStorage string
	flagSecret  string
)

func main() {
	root := &cobra.Command{
		Use:   "veld-registry",
		Short: "Veld Registry — self-hostable contract package registry",
		Example: `  # Use a config file (recommended)
  veld-registry --config registry.config.json

  # All inline
  veld-registry --addr :9000 --dsn "postgres://localhost/veld?sslmode=disable" --secret mysecret`,
		RunE: runServer,
	}

	root.Flags().StringVarP(&configFile, "config", "c", "", "path to registry.config.json (auto-detected if omitted)")
	root.Flags().StringVar(&flagAddr, "addr", "", "listen address (default :8080)")
	root.Flags().StringVar(&flagDSN, "dsn", "", "PostgreSQL DSN")
	root.Flags().StringVar(&flagStorage, "storage", "", "tarball storage directory (default ./packages)")
	root.Flags().StringVar(&flagSecret, "secret", "", "JWT signing secret (min 16 chars)")

	if err := root.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func runServer(cmd *cobra.Command, args []string) error {
	// 1. Find config file
	cfgPath := configFile
	if cfgPath == "" {
		for _, candidate := range []string{"registry.config.json", "veld/registry.config.json"} {
			if _, err := os.Stat(candidate); err == nil {
				cfgPath = candidate
				break
			}
		}
	}

	// 2. Load config file (empty struct if not found)
	fileCfg := loadConfigFile(cfgPath)
	if cfgPath != "" {
		if _, err := os.Stat(cfgPath); err == nil {
			log.Printf("Config: %s", cfgPath)
		}
	}

	// 3. Merge: file → env → flag (highest wins)
	cfg := server.Config{
		Addr:        resolve(flagAddr, env("VELD_ADDR"), fileCfg.Addr, ":8080"),
		DSN:         resolve(flagDSN, env("VELD_DSN"), fileCfg.DSN, ""),
		StoragePath: resolve(flagStorage, env("VELD_STORAGE"), fileCfg.StoragePath, "./packages"),
		JWTSecret:   resolve(flagSecret, env("VELD_SECRET"), fileCfg.JWTSecret, ""),
	}

	// 4. Validate required fields
	if cfg.DSN == "" {
		return fmt.Errorf(
			"database DSN is required.\n\n" +
				"Set it in registry.config.json:\n" +
				"  { \"dsn\": \"postgres://localhost/veld?sslmode=disable\" }\n\n" +
				"Or via flag:  --dsn \"postgres://localhost/veld?sslmode=disable\"\n" +
				"Or via env:   VELD_DSN=postgres://localhost/veld?sslmode=disable",
		)
	}
	if cfg.JWTSecret == "" {
		return fmt.Errorf(
			"JWT secret is required.\n\n" +
				"Set it in registry.config.json:\n" +
				"  { \"secret\": \"your-secret-here\" }\n\n" +
				"Or via flag:  --secret \"your-secret\"\n" +
				"Or generate: openssl rand -hex 32",
		)
	}

	// 5. Start server
	srv, err := server.New(cfg)
	if err != nil {
		return fmt.Errorf("init: %w", err)
	}
	defer srv.Close()

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	log.Printf("Veld Registry  →  http://localhost%s", cfg.Addr)
	log.Printf("  Storage: %s", cfg.StoragePath)

	return srv.Start(ctx)
}

// resolve returns the first non-empty value from the provided candidates.
func resolve(candidates ...string) string {
	for _, c := range candidates {
		if c != "" {
			return c
		}
	}
	return ""
}

func env(key string) string { return os.Getenv(key) }

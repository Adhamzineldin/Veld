package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/signal"

	"github.com/Adhamzineldin/Veld/internal/server"
	"github.com/Adhamzineldin/Veld/internal/server/email"
	"github.com/spf13/cobra"
	// Register all emitters via init(). To add a new emitter, add one line here.
	// Register tool emitters (auxiliary generators — NOT backends).
)

type serveRegistryConfig struct {
	Addr        string          `json:"addr"`
	DSN         string          `json:"dsn"`
	StoragePath string          `json:"storage"`
	JWTSecret   string          `json:"secret"`
	BaseURL     string          `json:"base_url"`
	SMTP        serveSmtpConfig `json:"smtp"`
}

type serveSmtpConfig struct {
	Host     string `json:"host"`
	Port     int    `json:"port"`
	Username string `json:"username"`
	Password string `json:"password"`
	From     string `json:"from"`
}

func loadServeConfigFile(path string) serveRegistryConfig {
	data, err := os.ReadFile(path)
	if err != nil {
		return serveRegistryConfig{}
	}
	var c serveRegistryConfig
	if err := json.Unmarshal(data, &c); err != nil {
		fmt.Fprintf(os.Stderr, yellow("warning: ")+"could not parse %s: %v\n", path, err)
	}
	return c
}

func resolveServeVal(candidates ...string) string {
	for _, c := range candidates {
		if c != "" {
			return c
		}
	}
	return ""
}

func newServeCmd() *cobra.Command {
	var configFile, flagAddr, flagDSN, flagStorage, flagSecret string

	cmd := &cobra.Command{
		Use:   "serve",
		Short: "Start the Veld Registry server",
		Long: `Start a self-hosted Veld Registry server.

Config is loaded from registry.config.json (current dir or --config flag).
CLI flags and environment variables override config file values.

Priority (highest → lowest): CLI flags > env vars > registry.config.json > defaults`,
		Example: `  # Use a config file (recommended)
  veld registry serve --config registry.config.json

  # All inline
  veld registry serve --addr :9000 --dsn "postgres://localhost/veld?sslmode=disable" --secret mysecret

  # Via environment variables
  VELD_DSN=postgres://localhost/veld VELD_SECRET=mysecret veld registry serve`,
		RunE: func(cmd *cobra.Command, args []string) error {
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

			// 2. Load config file
			fileCfg := loadServeConfigFile(cfgPath)
			if cfgPath != "" {
				if _, err := os.Stat(cfgPath); err == nil {
					fmt.Printf(dim("⬡")+"  Config: %s\n", cfgPath)
				}
			}

			// 3. Merge: file → env → flag (highest wins)
			cfg := server.Config{
				Addr:        resolveServeVal(flagAddr, os.Getenv("VELD_ADDR"), fileCfg.Addr, ":8080"),
				DSN:         resolveServeVal(flagDSN, os.Getenv("VELD_DSN"), fileCfg.DSN, ""),
				StoragePath: resolveServeVal(flagStorage, os.Getenv("VELD_STORAGE"), fileCfg.StoragePath, "./packages"),
				JWTSecret:   resolveServeVal(flagSecret, os.Getenv("VELD_SECRET"), fileCfg.JWTSecret, ""),
				BaseURL:     resolveServeVal(os.Getenv("VELD_BASE_URL"), fileCfg.BaseURL, ""),
				Email: email.Config{
					Host:     resolveServeVal(os.Getenv("SMTP_HOST"), fileCfg.SMTP.Host, ""),
					Port:     fileCfg.SMTP.Port,
					Username: resolveServeVal(os.Getenv("SMTP_USERNAME"), fileCfg.SMTP.Username, ""),
					Password: resolveServeVal(os.Getenv("SMTP_PASSWORD"), fileCfg.SMTP.Password, ""),
					From:     resolveServeVal(os.Getenv("SMTP_FROM"), fileCfg.SMTP.From, ""),
				},
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

			ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
			defer stop()

			fmt.Printf(green("✓")+" Veld Registry  →  http://localhost%s\n", cfg.Addr)
			fmt.Printf(dim("   Storage: %s\n"), cfg.StoragePath)

			return srv.Start(ctx)
		},
	}

	cmd.Flags().StringVarP(&configFile, "config", "c", "", "path to registry.config.json (auto-detected if omitted)")
	cmd.Flags().StringVar(&flagAddr, "addr", "", "listen address (default :8080)")
	cmd.Flags().StringVar(&flagDSN, "dsn", "", "PostgreSQL DSN")
	cmd.Flags().StringVar(&flagStorage, "storage", "", "tarball storage directory (default ./packages)")
	cmd.Flags().StringVar(&flagSecret, "secret", "", "JWT signing secret (min 16 chars)")
	return cmd
}

// ── registry helpers ──────────────────────────────────────────────────────────

// parsePackageRef parses "@org/name@version" → (org, name, version, err).

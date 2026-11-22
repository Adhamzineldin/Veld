// Command veld-registry runs the Veld package registry server.
//
// Configuration precedence (highest first): flags, environment variables,
// registry.config.json, defaults.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/signal"

	"github.com/Adhamzineldin/Veld/registry/internal/server"
	"github.com/Adhamzineldin/Veld/registry/internal/server/email"
)

type fileConfig struct {
	Addr        string `json:"addr"`
	DSN         string `json:"dsn"`
	StoragePath string `json:"storage"`
	JWTSecret   string `json:"secret"`
	BaseURL     string `json:"base_url"`
	SMTP        struct {
		Host     string `json:"host"`
		Port     int    `json:"port"`
		Username string `json:"username"`
		Password string `json:"password"`
		From     string `json:"from"`
	} `json:"smtp"`
}

func loadFile(path string) fileConfig {
	var c fileConfig
	data, err := os.ReadFile(path)
	if err != nil {
		return c
	}
	if err := json.Unmarshal(data, &c); err != nil {
		fmt.Fprintf(os.Stderr, "warning: could not parse %s: %v\n", path, err)
	}
	return c
}

func first(candidates ...string) string {
	for _, c := range candidates {
		if c != "" {
			return c
		}
	}
	return ""
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}

func run() error {
	var configFile, flagAddr, flagDSN, flagStorage, flagSecret string
	flag.StringVar(&configFile, "config", "", "path to registry.config.json (auto-detected if omitted)")
	flag.StringVar(&flagAddr, "addr", "", "listen address (default :8080)")
	flag.StringVar(&flagDSN, "dsn", "", "PostgreSQL DSN")
	flag.StringVar(&flagStorage, "storage", "", "tarball storage directory (default ./packages)")
	flag.StringVar(&flagSecret, "secret", "", "JWT signing secret (min 16 chars)")
	flag.Parse()

	cfgPath := configFile
	if cfgPath == "" {
		for _, c := range []string{"registry.config.json", "veld/registry.config.json"} {
			if _, err := os.Stat(c); err == nil {
				cfgPath = c
				break
			}
		}
	}
	fc := loadFile(cfgPath)

	cfg := server.Config{
		Addr:        first(flagAddr, os.Getenv("VELD_ADDR"), fc.Addr, ":8080"),
		DSN:         first(flagDSN, os.Getenv("VELD_DSN"), fc.DSN),
		StoragePath: first(flagStorage, os.Getenv("VELD_STORAGE"), fc.StoragePath, "./packages"),
		JWTSecret:   first(flagSecret, os.Getenv("VELD_SECRET"), fc.JWTSecret),
		BaseURL:     first(os.Getenv("VELD_BASE_URL"), fc.BaseURL),
		Email: email.Config{
			Host:     first(os.Getenv("SMTP_HOST"), fc.SMTP.Host),
			Port:     fc.SMTP.Port,
			Username: first(os.Getenv("SMTP_USERNAME"), fc.SMTP.Username),
			Password: first(os.Getenv("SMTP_PASSWORD"), fc.SMTP.Password),
			From:     first(os.Getenv("SMTP_FROM"), fc.SMTP.From),
		},
	}
	if cfg.DSN == "" {
		return fmt.Errorf("database DSN is required (--dsn, VELD_DSN or \"dsn\" in registry.config.json)")
	}
	if cfg.JWTSecret == "" {
		return fmt.Errorf("JWT secret is required (--secret, VELD_SECRET or \"secret\" in registry.config.json; generate with: openssl rand -hex 32)")
	}

	srv, err := server.New(cfg)
	if err != nil {
		return fmt.Errorf("init: %w", err)
	}
	defer srv.Close()

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	defer stop()
	fmt.Printf("Veld Registry listening on %s (storage: %s)\n", cfg.Addr, cfg.StoragePath)
	return srv.Start(ctx)
}

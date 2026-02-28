package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// RawConfig mirrors veld.config.json on disk.
type RawConfig struct {
	Input    string `json:"input"`
	Backend  string `json:"backend"`
	Frontend string `json:"frontend"`
	Out      string `json:"out"`
	BaseUrl  string `json:"baseUrl,omitempty"` // baked into frontend SDK; empty = use env var
}

// ResolvedConfig has all paths resolved to be absolute.
type ResolvedConfig struct {
	Input     string
	Backend   string
	Frontend  string
	Out       string
	ConfigDir string // absolute dir of veld.config.json; used for cache storage
	BaseUrl   string // base URL for frontend SDK (empty = process.env.VELD_API_URL)
}

// FlagOverrides carries CLI flag values that override config-file settings.
// A zero-value string means "not set by the user".
type FlagOverrides struct {
	Backend  string
	Frontend string
	Input    string
	Out      string
	// Changed tracks which flags were explicitly passed.
	BackendSet  bool
	FrontendSet bool
	InputSet    bool
	OutSet      bool
}

// frontendAlias normalises legacy frontend names.
func frontendAlias(name string) string {
	switch name {
	case "react":
		return "typescript"
	default:
		return name
	}
}

// FindConfig locates veld.config.json and returns its contents plus its
// absolute directory path.
func FindConfig() (RawConfig, string, error) {
	candidates := []string{
		"veld.config.json",
		filepath.Join("veld", "veld.config.json"),
	}
	for _, p := range candidates {
		data, err := os.ReadFile(p)
		if err != nil {
			continue
		}
		var cfg RawConfig
		if err := json.Unmarshal(data, &cfg); err != nil {
			return RawConfig{}, "", fmt.Errorf("parsing %s: %w", p, err)
		}
		abs, err := filepath.Abs(filepath.Dir(p))
		if err != nil {
			return RawConfig{}, "", err
		}
		return cfg, abs, nil
	}
	abs, _ := filepath.Abs(".")
	return RawConfig{}, abs, nil
}

// BuildResolved merges config file values with flag overrides and
// resolves all paths to absolute form.
func BuildResolved(flags FlagOverrides) (ResolvedConfig, error) {
	cfg, cfgDir, err := FindConfig()
	if err != nil {
		return ResolvedConfig{}, err
	}

	if flags.BackendSet {
		cfg.Backend = flags.Backend
	}
	if flags.FrontendSet {
		cfg.Frontend = flags.Frontend
	}
	if flags.InputSet {
		cfg.Input = flags.Input
		cfgDir, _ = filepath.Abs(".")
	}
	if flags.OutSet {
		cfg.Out = flags.Out
	}

	if cfg.Backend == "" {
		cfg.Backend = "node"
	}
	if cfg.Frontend == "" {
		cfg.Frontend = "typescript"
	}
	cfg.Frontend = frontendAlias(cfg.Frontend)

	if cfg.Out == "" {
		cfg.Out = "./generated"
	}

	if cfg.Input == "" {
		return ResolvedConfig{}, fmt.Errorf("no input file (use --input or create veld/veld.config.json)")
	}

	return ResolvedConfig{
		Input:     filepath.Clean(filepath.Join(cfgDir, cfg.Input)),
		Backend:   cfg.Backend,
		Frontend:  cfg.Frontend,
		Out:       filepath.Clean(filepath.Join(cfgDir, cfg.Out)),
		ConfigDir: cfgDir,
		BaseUrl:   cfg.BaseUrl,
	}, nil
}

// ResolveInput returns the .veld input path from positional args or config file.
func ResolveInput(args []string) (string, error) {
	if len(args) == 1 {
		return args[0], nil
	}
	cfg, cfgDir, err := FindConfig()
	if err != nil {
		return "", err
	}
	if cfg.Input == "" {
		return "", fmt.Errorf("no input file specified and no veld.config.json found")
	}
	return filepath.Clean(filepath.Join(cfgDir, cfg.Input)), nil
}

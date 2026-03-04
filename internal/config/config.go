package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// RawConfig mirrors veld.config.json on disk.
type RawConfig struct {
	Input             string            `json:"input"`
	Backend           string            `json:"backend"`
	Frontend          string            `json:"frontend"`
	Out               string            `json:"out"`
	BackendOut        string            `json:"backendOut,omitempty"`        // separate output dir for backend code
	FrontendOut       string            `json:"frontendOut,omitempty"`       // separate output dir for frontend code
	BackendDir        string            `json:"backendDir,omitempty"`        // path to backend project dir (for setup)
	BackendDirectory  string            `json:"backendDirectory,omitempty"`  // alias for backendDir
	FrontendDir       string            `json:"frontendDir,omitempty"`       // path to frontend project dir (for setup)
	FrontendDirectory string            `json:"frontendDirectory,omitempty"` // alias for frontendDir
	BaseUrl           string            `json:"baseUrl,omitempty"`           // baked into frontend SDK; empty = use env var
	Validate          bool              `json:"validate,omitempty"`          // emit zero-dep runtime validators (default false)
	Aliases           map[string]string `json:"aliases,omitempty"`           // custom @alias → relative dir
}

// effectiveBackendDir returns the configured backend directory, preferring backendDir over backendDirectory.
func (c RawConfig) effectiveBackendDir() string {
	if c.BackendDir != "" {
		return c.BackendDir
	}
	return c.BackendDirectory
}

// effectiveFrontendDir returns the configured frontend directory, preferring frontendDir over frontendDirectory.
func (c RawConfig) effectiveFrontendDir() string {
	if c.FrontendDir != "" {
		return c.FrontendDir
	}
	return c.FrontendDirectory
}

// ResolvedConfig has all paths resolved to be absolute.
type ResolvedConfig struct {
	Input       string
	Backend     string
	Frontend    string
	Out         string            // legacy single output dir (always set for backward compat)
	BackendOut  string            // output dir for backend code (defaults to Out)
	FrontendOut string            // output dir for frontend code (defaults to Out)
	ConfigDir   string            // absolute dir of veld.config.json; used for cache storage
	BackendDir  string            // absolute path to backend project dir (empty = projectDir)
	FrontendDir string            // absolute path to frontend project dir (empty = projectDir)
	BaseUrl     string            // base URL for frontend SDK (empty = process.env.VELD_API_URL)
	Validate    bool              // emit zero-dep runtime validators and wire into routes
	Aliases     map[string]string // merged: default aliases + config overrides
}

// FlagOverrides carries CLI flag values that override config-file settings.
// A zero-value string means "not set by the user".
type FlagOverrides struct {
	Backend     string
	Frontend    string
	Input       string
	Out         string
	BackendOut  string
	FrontendOut string
	Validate    bool
	// Changed tracks which flags were explicitly passed.
	BackendSet     bool
	FrontendSet    bool
	InputSet       bool
	OutSet         bool
	BackendOutSet  bool
	FrontendOutSet bool
	ValidateSet    bool
}

// backendAlias normalises backend shorthand names.
func backendAlias(name string) string {
	switch name {
	case "js":
		return "javascript"
	default:
		return name
	}
}

// frontendAlias normalises legacy frontend names.
func frontendAlias(name string) string {
	switch name {
	case "flutter":
		return "dart"
	case "ts":
		return "typescript"
	case "hooks", "react-hooks":
		return "react"
	case "js":
		return "javascript"
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
	if flags.BackendOutSet {
		cfg.BackendOut = flags.BackendOut
	}
	if flags.FrontendOutSet {
		cfg.FrontendOut = flags.FrontendOut
	}
	if flags.ValidateSet {
		cfg.Validate = flags.Validate
	}

	if cfg.Backend == "" {
		cfg.Backend = "node"
	}
	if cfg.Frontend == "" {
		cfg.Frontend = "typescript"
	}
	cfg.Backend = backendAlias(cfg.Backend)
	cfg.Frontend = frontendAlias(cfg.Frontend)

	if cfg.Out == "" {
		cfg.Out = "./generated"
	}

	// Validate out path — must end with a named directory, not just ".." or "/"
	validateOutPath := func(p, label string) error {
		base := filepath.Base(filepath.Clean(p))
		if base == ".." || base == "." || base == "/" || base == string(filepath.Separator) {
			return fmt.Errorf(
				"invalid %s path %q: must end with a folder name (e.g. \"../generated\", not \"..\")",
				label, p,
			)
		}
		return nil
	}

	if err := validateOutPath(cfg.Out, "out"); err != nil {
		return ResolvedConfig{}, err
	}
	if cfg.BackendOut != "" {
		if err := validateOutPath(cfg.BackendOut, "backendOut"); err != nil {
			return ResolvedConfig{}, err
		}
	}
	if cfg.FrontendOut != "" {
		if err := validateOutPath(cfg.FrontendOut, "frontendOut"); err != nil {
			return ResolvedConfig{}, err
		}
	}

	if cfg.Input == "" {
		return ResolvedConfig{}, fmt.Errorf("no input file (use --input or create veld/veld.config.json)")
	}

	// Merge default aliases with user-defined overrides
	aliases := DefaultAliases()
	for k, v := range cfg.Aliases {
		aliases[k] = v
	}

	resolvedOut := filepath.Clean(filepath.Join(cfgDir, cfg.Out))

	// BackendOut / FrontendOut: if set, resolve relative to cfgDir; otherwise fall back to Out
	resolvedBackendOut := resolvedOut
	if cfg.BackendOut != "" {
		resolvedBackendOut = filepath.Clean(filepath.Join(cfgDir, cfg.BackendOut))
	}
	resolvedFrontendOut := resolvedOut
	if cfg.FrontendOut != "" {
		resolvedFrontendOut = filepath.Clean(filepath.Join(cfgDir, cfg.FrontendOut))
	}

	return ResolvedConfig{
		Input:       filepath.Clean(filepath.Join(cfgDir, cfg.Input)),
		Backend:     cfg.Backend,
		Frontend:    cfg.Frontend,
		Out:         resolvedOut,
		BackendOut:  resolvedBackendOut,
		FrontendOut: resolvedFrontendOut,
		ConfigDir:   cfgDir,
		BackendDir:  resolveOptionalDir(cfgDir, cfg.effectiveBackendDir()),
		FrontendDir: resolveOptionalDir(cfgDir, cfg.effectiveFrontendDir()),
		BaseUrl:     cfg.BaseUrl,
		Validate:    cfg.Validate,
		Aliases:     aliases,
	}, nil
}

// SplitOutput returns true when backend and frontend have different output directories.
func (rc ResolvedConfig) SplitOutput() bool {
	return rc.BackendOut != rc.FrontendOut
}

// OutputDirs returns the unique set of output directories (1 if same, 2 if split).
func (rc ResolvedConfig) OutputDirs() []string {
	if rc.SplitOutput() {
		return []string{rc.BackendOut, rc.FrontendOut}
	}
	return []string{rc.Out}
}

// resolveOptionalDir resolves a dir path relative to base. Returns "" if dir is empty.
func resolveOptionalDir(base, dir string) string {
	if dir == "" {
		return ""
	}
	if filepath.IsAbs(dir) {
		return filepath.Clean(dir)
	}
	return filepath.Clean(filepath.Join(base, dir))
}

// DefaultAliases returns the built-in alias→folder mappings.
// These work without any config (alias name = folder name).
func DefaultAliases() map[string]string {
	return map[string]string{
		"models":   "models",
		"modules":  "modules",
		"types":    "types",
		"enums":    "enums",
		"schemas":  "schemas",
		"services": "services",
		"lib":      "lib",
		"common":   "common",
		"shared":   "shared",
	}
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

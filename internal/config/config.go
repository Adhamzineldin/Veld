package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// WorkspaceEntry defines one service in a multi-service monorepo workspace.
type WorkspaceEntry struct {
	Name      string   `json:"name"`               // logical service name
	Input     string   `json:"input"`              // path to .veld entry file
	Backend   string   `json:"backend,omitempty"`  // overrides top-level backend
	Frontend  string   `json:"frontend,omitempty"` // overrides top-level frontend
	Out       string   `json:"out,omitempty"`      // output dir; defaults to generated/<name>
	BaseUrl   string   `json:"baseUrl,omitempty"`  // this service's base URL
	ServerSdk bool     `json:"serverSdk,omitempty"`
	Consumes  []string `json:"consumes,omitempty"` // workspace entry names this service depends on (service SDK generation)
}

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
	BaseUrl           string            `json:"baseUrl,omitempty"`           // default base URL for all services; empty = env var
	Validate          bool              `json:"validate,omitempty"`          // emit zero-dep runtime validators (default false)
	BackendFramework  string            `json:"backendFramework,omitempty"`  // e.g. "express", "flask" — "" means plain
	FrontendFramework string            `json:"frontendFramework,omitempty"` // e.g. "react", "vue" — "" means none
	Aliases           map[string]string `json:"aliases,omitempty"`           // custom @alias → relative dir
	PostGenerate      string            `json:"postGenerate,omitempty"`      // shell command to run after generation
	Registry          RegistryConfig    `json:"registry,omitempty"`          // cloud registry publishing config
	Description       string            `json:"description,omitempty"`       // human/AI-readable project description
	Services          map[string]string `json:"services,omitempty"`          // module name → base URL override (optional)
	ServerSdk         bool              `json:"serverSdk,omitempty"`         // also emit a server-to-server typed client
	Workspace         []WorkspaceEntry  `json:"workspace,omitempty"`         // multi-service monorepo entries
}

// RegistryConfig holds optional publish metadata baked into veld.config.json.
type RegistryConfig struct {
	Enabled bool   `json:"enabled"`           // set to true to activate push/pull
	URL     string `json:"url,omitempty"`     // registry base URL
	Org     string `json:"org,omitempty"`     // organisation name (@scope)
	Package string `json:"package,omitempty"` // package name
	Version string `json:"version,omitempty"` // current version to publish
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
	Input             string
	Backend           string
	Frontend          string
	Out               string            // legacy single output dir (always set for backward compat)
	BackendOut        string            // output dir for backend code (defaults to Out)
	FrontendOut       string            // output dir for frontend code (defaults to Out)
	ConfigDir         string            // absolute dir of veld.config.json; used for cache storage
	BackendDir        string            // absolute path to backend project dir (empty = projectDir)
	FrontendDir       string            // absolute path to frontend project dir (empty = projectDir)
	BaseUrl           string            // default base URL; empty = process.env.VELD_API_URL
	Validate          bool              // emit zero-dep runtime validators and wire into routes
	BackendFramework  string            // e.g. "express", "flask" — "" means plain
	FrontendFramework string            // e.g. "react", "vue" — "" means none
	Aliases           map[string]string // merged: default aliases + config overrides
	PostGenerate      string            // shell command to run after generation (empty = none)
	Registry          RegistryConfig    // publish metadata
	Description       string            // human/AI-readable project description
	Services          map[string]string // module name → base URL override (nil = use global BaseUrl)
	ServerSdk         bool              // also emit a server-to-server typed client
	Workspace         []WorkspaceEntry  // resolved workspace entries (empty = single-service mode)
}

// FlagOverrides carries CLI flag values that override config-file settings.
// A zero-value string means "not set by the user".
type FlagOverrides struct {
	Backend           string
	Frontend          string
	Input             string
	Out               string
	BackendOut        string
	FrontendOut       string
	Validate          bool
	BackendFramework  string // "" means not set
	FrontendFramework string // "" means not set
	// Changed tracks which flags were explicitly passed.
	BackendSet           bool
	FrontendSet          bool
	InputSet             bool
	OutSet               bool
	BackendOutSet        bool
	FrontendOutSet       bool
	ValidateSet          bool
	BackendFrameworkSet  bool
	FrontendFrameworkSet bool
}

// backendAlias normalises backend shorthand names.
func backendAlias(name string) string {
	switch name {
	case "node":
		return "node-ts"
	case "js", "javascript", "node-javascript":
		return "node-js"
	case "node-typescript":
		return "node-ts"
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
	if flags.BackendFrameworkSet {
		cfg.BackendFramework = flags.BackendFramework
	}
	if flags.FrontendFrameworkSet {
		cfg.FrontendFramework = flags.FrontendFramework
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

	if cfg.Input == "" && len(cfg.Workspace) == 0 {
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

	resolvedInput := cfg.Input
	if !filepath.IsAbs(resolvedInput) {
		resolvedInput = filepath.Join(cfgDir, resolvedInput)
	}

	return ResolvedConfig{
		Input:             filepath.Clean(resolvedInput),
		Backend:           cfg.Backend,
		Frontend:          cfg.Frontend,
		Out:               resolvedOut,
		BackendOut:        resolvedBackendOut,
		FrontendOut:       resolvedFrontendOut,
		ConfigDir:         cfgDir,
		BackendDir:        resolveOptionalDir(cfgDir, cfg.effectiveBackendDir()),
		FrontendDir:       resolveOptionalDir(cfgDir, cfg.effectiveFrontendDir()),
		BaseUrl:           cfg.BaseUrl,
		Validate:          cfg.Validate,
		BackendFramework:  cfg.BackendFramework,
		FrontendFramework: cfg.FrontendFramework,
		Aliases:           aliases,
		PostGenerate:      cfg.PostGenerate,
		Registry:          cfg.Registry,
		Description:       cfg.Description,
		Services:          cfg.Services,
		ServerSdk:         cfg.ServerSdk,
		Workspace:         cfg.Workspace,
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

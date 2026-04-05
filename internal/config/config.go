package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// WorkspaceEntry defines one service in a multi-service monorepo workspace.
type WorkspaceEntry struct {
	Name        string   `json:"name"`               // logical service name
	Input       string   `json:"input"`              // path to .veld entry file (optional for frontend-only entries)
	Backend     string   `json:"backend,omitempty"`  // legacy flat: overrides top-level backend (string)
	Frontend    string   `json:"frontend,omitempty"` // legacy flat: overrides top-level frontend (string)
	Out         string   `json:"out,omitempty"`      // legacy flat: output dir
	BaseUrl     string   `json:"baseUrl,omitempty"`  // this service's base URL
	ServerSdk   bool     `json:"serverSdk,omitempty"`
	Consumes    []string `json:"consumes,omitempty"`    // workspace entry names this service depends on
	Description string   `json:"description,omitempty"` // per-service description
	Validate    *bool    `json:"validate,omitempty"`    // per-service validate override

	// New nested format (takes precedence over flat fields).
	BackendCfg  *BackendConfig  `json:"backendConfig,omitempty"`
	FrontendCfg *FrontendConfig `json:"frontendConfig,omitempty"`
}

// BackendConfig is the nested config for backend code generation.
type BackendConfig struct {
	Target    string `json:"target"`              // emitter name: "node-ts", "python", "go", etc.
	Framework string `json:"framework,omitempty"` // "express", "flask", "chi", "spring", etc.
	Out       string `json:"out,omitempty"`       // output directory
	Dir       string `json:"dir,omitempty"`       // project directory (for setup)
	Validate  *bool  `json:"validate,omitempty"`  // emit validators
}

// FrontendConfig is the nested config for frontend SDK generation.
type FrontendConfig struct {
	Target string `json:"target"`        // emitter name: "react", "vue", "angular", etc.
	Out    string `json:"out,omitempty"` // output directory
	Dir    string `json:"dir,omitempty"` // project directory (for setup)
}

// ToolsConfig controls auxiliary generators (cicd, dockerfile, etc.).
type ToolsConfig struct {
	OpenAPI    *bool `json:"openapi,omitempty"`
	Dockerfile *bool `json:"dockerfile,omitempty"`
	CICD       *bool `json:"cicd,omitempty"`
	Database   *bool `json:"database,omitempty"`
	Scaffold   *bool `json:"scaffold,omitempty"`
	EnvConfig  *bool `json:"envconfig,omitempty"`
}

// HooksConfig contains lifecycle hook commands.
type HooksConfig struct {
	PostGenerate string `json:"postGenerate,omitempty"` // shell command after generation
}

// RawConfig mirrors veld.config.json on disk.
// Supports BOTH the legacy flat format and the new nested format.
type RawConfig struct {
	Schema string `json:"$schema,omitempty"` // JSON Schema reference (ignored by parser)

	Input       string            `json:"input"`
	Description string            `json:"description,omitempty"` // human/AI-readable project description
	BaseUrl     string            `json:"baseUrl,omitempty"`     // default base URL for all services
	Aliases     map[string]string `json:"aliases,omitempty"`     // custom @alias → relative dir
	Services    map[string]string `json:"services,omitempty"`    // module name → base URL override
	ServerSdk   bool              `json:"serverSdk,omitempty"`   // emit server-to-server typed client
	Workspace   []WorkspaceEntry  `json:"workspace,omitempty"`   // multi-service monorepo entries
	Registry    RegistryConfig    `json:"registry,omitempty"`    // cloud registry publishing config

	// ── New nested format ────────────────────────────────────────────────
	BackendCfg  *BackendConfig  `json:"backendConfig,omitempty"`
	FrontendCfg *FrontendConfig `json:"frontendConfig,omitempty"`
	Tools       *ToolsConfig    `json:"tools,omitempty"`
	Hooks       *HooksConfig    `json:"hooks,omitempty"`

	// ── Legacy flat format (still supported, normalized internally) ──────
	Backend           string `json:"backend"`
	Frontend          string `json:"frontend"`
	Out               string `json:"out"`
	BackendOut        string `json:"backendOut,omitempty"`
	FrontendOut       string `json:"frontendOut,omitempty"`
	BackendDir        string `json:"backendDir,omitempty"`
	BackendDirectory  string `json:"backendDirectory,omitempty"` // deprecated alias
	FrontendDir       string `json:"frontendDir,omitempty"`
	FrontendDirectory string `json:"frontendDirectory,omitempty"` // deprecated alias
	Validate          bool   `json:"validate,omitempty"`
	BackendFramework  string `json:"backendFramework,omitempty"`
	FrontendFramework string `json:"frontendFramework,omitempty"`
	PostGenerate      string `json:"postGenerate,omitempty"` // deprecated: use hooks.postGenerate
}

// RegistryConfig holds optional publish metadata baked into veld.config.json.
type RegistryConfig struct {
	Enabled bool   `json:"enabled"`           // set to true to activate push/pull
	URL     string `json:"url,omitempty"`     // registry base URL
	Org     string `json:"org,omitempty"`     // organisation name (@scope)
	Package string `json:"package,omitempty"` // package name
	Version string `json:"version,omitempty"` // current version to publish
}

// effectiveBackendDir returns the configured backend directory.
func (c RawConfig) effectiveBackendDir() string {
	// New nested format takes precedence.
	if c.BackendCfg != nil && c.BackendCfg.Dir != "" {
		return c.BackendCfg.Dir
	}
	if c.BackendDir != "" {
		return c.BackendDir
	}
	return c.BackendDirectory
}

// effectiveFrontendDir returns the configured frontend directory.
func (c RawConfig) effectiveFrontendDir() string {
	if c.FrontendCfg != nil && c.FrontendCfg.Dir != "" {
		return c.FrontendCfg.Dir
	}
	if c.FrontendDir != "" {
		return c.FrontendDir
	}
	return c.FrontendDirectory
}

// effectivePostGenerate returns the lifecycle hook command.
func (c RawConfig) effectivePostGenerate() string {
	if c.Hooks != nil && c.Hooks.PostGenerate != "" {
		return c.Hooks.PostGenerate
	}
	return c.PostGenerate
}

// normalize promotes nested BackendCfg/FrontendCfg into the flat fields
// so BuildResolved doesn't need two code paths. Nested config takes precedence.
func (c *RawConfig) normalize() {
	if c.BackendCfg != nil {
		if c.BackendCfg.Target != "" && c.Backend == "" {
			c.Backend = c.BackendCfg.Target
		}
		if c.BackendCfg.Framework != "" && c.BackendFramework == "" {
			c.BackendFramework = c.BackendCfg.Framework
		}
		if c.BackendCfg.Out != "" && c.BackendOut == "" {
			c.BackendOut = c.BackendCfg.Out
		}
		if c.BackendCfg.Validate != nil && !c.Validate {
			c.Validate = *c.BackendCfg.Validate
		}
	}
	if c.FrontendCfg != nil {
		if c.FrontendCfg.Target != "" && c.Frontend == "" {
			c.Frontend = c.FrontendCfg.Target
		}
		if c.FrontendCfg.Out != "" && c.FrontendOut == "" {
			c.FrontendOut = c.FrontendCfg.Out
		}
	}
	if c.Hooks != nil && c.Hooks.PostGenerate != "" && c.PostGenerate == "" {
		c.PostGenerate = c.Hooks.PostGenerate
	}

	// Normalize workspace entries too.
	for i := range c.Workspace {
		e := &c.Workspace[i]
		if e.BackendCfg != nil {
			if e.BackendCfg.Target != "" && e.Backend == "" {
				e.Backend = e.BackendCfg.Target
			}
			if e.BackendCfg.Out != "" && e.Out == "" {
				e.Out = e.BackendCfg.Out
			}
		}
		if e.FrontendCfg != nil {
			if e.FrontendCfg.Target != "" && e.Frontend == "" {
				e.Frontend = e.FrontendCfg.Target
			}
			if e.FrontendCfg.Out != "" && e.Out == "" {
				e.Out = e.FrontendCfg.Out
			}
		}
	}

	// Expand wildcard consumes: ["*"] → all other workspace entry names.
	for i := range c.Workspace {
		e := &c.Workspace[i]
		for _, consumed := range e.Consumes {
			if consumed == "*" {
				expanded := make([]string, 0, len(c.Workspace)-1)
				for _, other := range c.Workspace {
					if other.Name != e.Name {
						expanded = append(expanded, other.Name)
					}
				}
				e.Consumes = expanded
				break
			}
		}
	}
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
		cfg.normalize()
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

	// Resolve out paths: if already absolute (e.g. on Windows "G:\..."), don't prepend cfgDir.
	// filepath.Join on Windows will produce "CWD\G:\..." which is invalid.
	resolveDir := func(p string) string {
		if filepath.IsAbs(p) {
			return filepath.Clean(p)
		}
		return filepath.Clean(filepath.Join(cfgDir, p))
	}

	resolvedOut := resolveDir(cfg.Out)

	// BackendOut / FrontendOut: if set, resolve relative to cfgDir; otherwise fall back to Out
	resolvedBackendOut := resolvedOut
	if cfg.BackendOut != "" {
		resolvedBackendOut = resolveDir(cfg.BackendOut)
	}
	resolvedFrontendOut := resolvedOut
	if cfg.FrontendOut != "" {
		resolvedFrontendOut = resolveDir(cfg.FrontendOut)
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
// For single-service configs this is 1–2 directories; for workspace configs it
// includes every workspace entry's resolved output directory.
func (rc ResolvedConfig) OutputDirs() []string {
	if len(rc.Workspace) > 0 {
		return rc.workspaceOutputDirs()
	}
	if rc.SplitOutput() {
		return []string{rc.BackendOut, rc.FrontendOut}
	}
	return []string{rc.Out}
}

// workspaceOutputDirs resolves and deduplicates output directories for every
// workspace entry. The resolution logic mirrors the generate command:
//  1. entry.Out (flat) → entry.BackendCfg.Out (nested) → <configDir>/generated/<name>
//  2. entry.FrontendCfg.Out if present (split frontend output)
func (rc ResolvedConfig) workspaceOutputDirs() []string {
	seen := make(map[string]bool)
	var dirs []string
	add := func(d string) {
		if d == "" || seen[d] {
			return
		}
		seen[d] = true
		dirs = append(dirs, d)
	}

	for _, entry := range rc.Workspace {
		outDir := entry.Out
		if outDir == "" && entry.BackendCfg != nil && entry.BackendCfg.Out != "" {
			outDir = entry.BackendCfg.Out
		}
		if outDir == "" {
			outDir = filepath.Join(rc.ConfigDir, "generated", entry.Name)
		} else if !filepath.IsAbs(outDir) {
			outDir = filepath.Clean(filepath.Join(rc.ConfigDir, outDir))
		}
		add(outDir)

		// Split frontend output directory.
		if entry.FrontendCfg != nil && entry.FrontendCfg.Out != "" {
			feOut := entry.FrontendCfg.Out
			if !filepath.IsAbs(feOut) {
				feOut = filepath.Clean(filepath.Join(rc.ConfigDir, feOut))
			}
			add(feOut)
		}
	}
	return dirs
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

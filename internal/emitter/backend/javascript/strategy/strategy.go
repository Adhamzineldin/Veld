package strategy

// JSFrameworkStrategy provides framework-specific declarations for the
// plain JavaScript backend emitter. Swap to add Express or other router types.
type JSFrameworkStrategy interface {
	// RouterType returns the JSDoc type string for the router parameter.
	RouterType() string
	// ExtraImports returns additional require() lines (full statements).
	ExtraImports() []string
	// PackageDependencies returns npm package name → semver version entries.
	PackageDependencies() map[string]string
}

// New returns the strategy for the given framework name.
// "" or "plain" → PlainStrategy. "express" → ExpressStrategy.
func New(framework string) JSFrameworkStrategy {
	switch framework {
	case "express":
		return &ExpressStrategy{}
	default: // "", "plain"
		return &PlainStrategy{}
	}
}

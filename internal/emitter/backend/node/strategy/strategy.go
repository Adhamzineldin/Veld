package strategy

// NodeFrameworkStrategy provides framework-specific TypeScript declarations
// for the Node.js HTTP backend. Swap to add Express/Fastify/Hono types.
type NodeFrameworkStrategy interface {
	// RouterType returns the TypeScript type for the router parameter.
	RouterType() string
	// RequestType returns the TypeScript type for request objects.
	RequestType() string
	// ResponseType returns the TypeScript type for response objects.
	ResponseType() string
	// ExtraImports returns additional TypeScript import lines (full statements).
	ExtraImports() []string
	// PackageDependencies returns npm package name → semver version entries.
	PackageDependencies() map[string]string
}

// New returns the strategy for the given framework name.
// "" or "plain" → PlainStrategy. "express" → ExpressStrategy.
func New(framework string) NodeFrameworkStrategy {
	switch framework {
	case "express":
		return &ExpressStrategy{}
	default: // "", "plain"
		return &PlainStrategy{}
	}
}

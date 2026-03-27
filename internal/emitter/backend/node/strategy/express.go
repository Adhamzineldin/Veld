package strategy

// ExpressStrategy adds Express 4.x type annotations to generated handlers.
type ExpressStrategy struct{}

func (s *ExpressStrategy) RouterType() string     { return "import('express').Router" }
func (s *ExpressStrategy) RequestType() string    { return "import('express').Request" }
func (s *ExpressStrategy) ResponseType() string   { return "import('express').Response" }
func (s *ExpressStrategy) ExtraImports() []string { return nil }
func (s *ExpressStrategy) PackageDependencies() map[string]string {
	return map[string]string{
		"express":        "^4.18.0",
		"@types/express": "^4.17.0",
	}
}

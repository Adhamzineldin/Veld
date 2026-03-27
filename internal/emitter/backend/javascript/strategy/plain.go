package strategy

// PlainStrategy is the default — router typed as *, fully framework-agnostic.
// This is Veld's original JavaScript output: the developer provides any router.
type PlainStrategy struct{}

func (s *PlainStrategy) RouterType() string                     { return "*" }
func (s *PlainStrategy) ExtraImports() []string                 { return nil }
func (s *PlainStrategy) PackageDependencies() map[string]string { return nil }

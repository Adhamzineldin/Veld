package strategy

// PlainStrategy is the default — router: any, fully framework-agnostic.
// This is Veld's original Node output: the developer provides any router.
type PlainStrategy struct{}

func (s *PlainStrategy) RouterType() string                     { return "any" }
func (s *PlainStrategy) RequestType() string                    { return "any" }
func (s *PlainStrategy) ResponseType() string                   { return "any" }
func (s *PlainStrategy) ExtraImports() []string                 { return nil }
func (s *PlainStrategy) PackageDependencies() map[string]string { return nil }

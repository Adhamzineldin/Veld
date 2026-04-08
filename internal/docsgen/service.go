package docsgen

// ServiceInfo carries per-service metadata for workspace/microservice documentation.
// When nil or empty, the docs generator treats all modules as a single service.
type ServiceInfo struct {
	Name        string   // workspace entry name (e.g. "iam", "accounts")
	Description string   // human-readable service description
	BaseUrl     string   // service base URL (e.g. "http://iam-service:3001")
	ModuleNames []string // module names owned by this service
}

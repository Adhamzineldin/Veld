// Package registry provides client-side support for the Veld Registry:
// credential storage, HTTP client, tarball packaging and semver resolution.
package registry

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// Credentials stores one entry per registry URL.
type Credentials struct {
	Registries map[string]RegistryAuth `json:"registries"`
}

// RegistryAuth holds the token for one registry.
type RegistryAuth struct {
	Token    string `json:"token"`
	Username string `json:"username,omitempty"`
}

func credPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".veld", "credentials.json"), nil
}

// LoadCredentials reads ~/.veld/credentials.json.
func LoadCredentials() (*Credentials, error) {
	p, err := credPath()
	if err != nil {
		return nil, err
	}
	data, err := os.ReadFile(p)
	if os.IsNotExist(err) {
		return &Credentials{Registries: map[string]RegistryAuth{}}, nil
	}
	if err != nil {
		return nil, err
	}
	var c Credentials
	if err := json.Unmarshal(data, &c); err != nil {
		return nil, err
	}
	if c.Registries == nil {
		c.Registries = map[string]RegistryAuth{}
	}
	return &c, nil
}

// SaveCredentials writes credentials back to disk with 0600 permissions.
func SaveCredentials(c *Credentials) error {
	p, err := credPath()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(p), 0700); err != nil {
		return err
	}
	data, err := json.MarshalIndent(c, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(p, data, 0600)
}

// GetToken returns the stored token for registry URL (empty string if not logged in).
func GetToken(registryURL string) string {
	c, err := LoadCredentials()
	if err != nil {
		return ""
	}
	return c.Registries[registryURL].Token
}

// SetToken stores a token for registry URL.
func SetToken(registryURL, token, username string) error {
	c, err := LoadCredentials()
	if err != nil {
		return err
	}
	c.Registries[registryURL] = RegistryAuth{Token: token, Username: username}
	return SaveCredentials(c)
}

// ClearToken removes credentials for registry URL.
func ClearToken(registryURL string) error {
	c, err := LoadCredentials()
	if err != nil {
		return err
	}
	delete(c.Registries, registryURL)
	return SaveCredentials(c)
}

// DefaultRegistry returns the first stored registry URL, or "".
func DefaultRegistry() string {
	c, err := LoadCredentials()
	if err != nil || len(c.Registries) == 0 {
		return ""
	}
	for url := range c.Registries {
		return url
	}
	return ""
}

// ListRegistries prints all stored registries to stdout.
func ListRegistries() {
	c, err := LoadCredentials()
	if err != nil || len(c.Registries) == 0 {
		fmt.Println("No registries configured. Run: veld login")
		return
	}
	for url, auth := range c.Registries {
		user := auth.Username
		if user == "" {
			user = "(token auth)"
		}
		fmt.Printf("  %s  →  %s\n", url, user)
	}
}

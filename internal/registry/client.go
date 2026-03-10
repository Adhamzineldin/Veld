package registry

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"mime/multipart"
	"net/http"
	"strings"
	"time"
)

// Client is an authenticated HTTP client for the Veld Registry API.
type Client struct {
	BaseURL string
	Token   string
	http    *http.Client
}

// NewClient creates a client for the given registry URL and token.
func NewClient(registryURL, token string) *Client {
	return &Client{
		BaseURL: strings.TrimRight(registryURL, "/"),
		Token:   token,
		http:    &http.Client{Timeout: 120 * time.Second},
	}
}

// NewClientFromCreds creates a client using stored credentials.
func NewClientFromCreds(registryURL string) (*Client, error) {
	if registryURL == "" {
		registryURL = DefaultRegistry()
	}
	if registryURL == "" {
		return nil, fmt.Errorf("no registry configured — run: veld login")
	}
	token := GetToken(registryURL)
	if token == "" {
		return nil, fmt.Errorf("not logged in to %s — run: veld login", registryURL)
	}
	return NewClient(registryURL, token), nil
}

func (c *Client) do(method, path string, body interface{}) ([]byte, int, error) {
	var bodyReader io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, 0, err
		}
		bodyReader = bytes.NewReader(data)
	}
	req, err := http.NewRequest(method, c.BaseURL+"/api/v1"+path, bodyReader)
	if err != nil {
		return nil, 0, err
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	if c.Token != "" {
		req.Header.Set("Authorization", "Bearer "+c.Token)
	}
	resp, err := c.http.Do(req)
	if err != nil {
		return nil, 0, err
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	return data, resp.StatusCode, err
}

func (c *Client) get(path string) ([]byte, error) {
	data, code, err := c.do("GET", path, nil)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, apiError(data, code)
	}
	return data, nil
}

func (c *Client) post(path string, body interface{}) ([]byte, error) {
	data, code, err := c.do("POST", path, body)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, apiError(data, code)
	}
	return data, nil
}

func apiError(data []byte, code int) error {
	var e struct {
		Error string `json:"error"`
	}
	json.Unmarshal(data, &e)
	if e.Error != "" {
		return fmt.Errorf("registry error (%d): %s", code, e.Error)
	}
	return fmt.Errorf("registry returned status %d", code)
}

// ── Registry API calls ────────────────────────────────────────────────────────

// Login authenticates with email+password and returns the JWT.
func (c *Client) Login(email, password string) (string, error) {
	data, err := c.post("/auth/login", map[string]string{"email": email, "password": password})
	if err != nil {
		return "", err
	}
	var r struct {
		Token string `json:"token"`
	}
	json.Unmarshal(data, &r)
	return r.Token, nil
}

// Me returns the current authenticated user info.
func (c *Client) Me() (map[string]interface{}, error) {
	data, err := c.get("/auth/me")
	if err != nil {
		return nil, err
	}
	var r map[string]interface{}
	json.Unmarshal(data, &r)
	return r, nil
}

// ListPackageVersions returns all versions for @org/name.
func (c *Client) ListPackageVersions(org, name string) ([]map[string]interface{}, error) {
	data, err := c.get(fmt.Sprintf("/packages/%s/%s/versions", org, name))
	if err != nil {
		return nil, err
	}
	var r []map[string]interface{}
	json.Unmarshal(data, &r)
	return r, nil
}

// Publish sends a tarball to the registry.
// manifest is the raw JSON of the publish request.
// tarball is the .tar.gz reader.
func (c *Client) Publish(manifestJSON string, tarballName string, tarball io.Reader) ([]byte, error) {
	var buf bytes.Buffer
	mw := multipart.NewWriter(&buf)

	mw.WriteField("manifest", manifestJSON)

	fw, err := mw.CreateFormFile("tarball", tarballName)
	if err != nil {
		return nil, err
	}
	if _, err := io.Copy(fw, tarball); err != nil {
		return nil, err
	}
	mw.Close()

	req, err := http.NewRequest("POST", c.BaseURL+"/api/v1/packages", &buf)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", mw.FormDataContentType())
	req.Header.Set("Authorization", "Bearer "+c.Token)

	resp, err := c.http.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	data, _ := io.ReadAll(resp.Body)
	if resp.StatusCode >= 400 {
		return nil, apiError(data, resp.StatusCode)
	}
	return data, nil
}

// PostJSON is a generic authenticated POST returning the raw response body.
func (c *Client) PostJSON(path string, body interface{}) (string, error) {
	data, err := c.post(path, body)
	if err != nil {
		return "", err
	}
	return string(data), nil
}

// Download streams a tarball to dst.
func (c *Client) Download(org, name, version string, dst io.Writer) (string, error) {
	req, err := http.NewRequest("GET",
		fmt.Sprintf("%s/api/v1/packages/%s/%s/%s/download", c.BaseURL, org, name, version), nil)
	if err != nil {
		return "", err
	}
	if c.Token != "" {
		req.Header.Set("Authorization", "Bearer "+c.Token)
	}
	resp, err := c.http.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 400 {
		data, _ := io.ReadAll(resp.Body)
		return "", apiError(data, resp.StatusCode)
	}
	sha := resp.Header.Get("X-Veld-SHA256")
	io.Copy(dst, resp.Body)
	return sha, nil
}

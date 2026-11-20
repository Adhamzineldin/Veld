//! The registry HTTP client (`ureq` with rustls).

use std::io::Read;
use std::time::Duration;

use serde::Deserialize;
use url::{Host, Url};

use crate::credentials::{Token, normalize_url};
use crate::error::{RegistryError, Result};

/// Largest package download accepted (compressed).
pub const MAX_DOWNLOAD: u64 = 32 << 20;

/// One published version, as listed by `GET /api/v1/packages/{org}/{name}/versions`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    #[serde(rename = "tarball_sha256")]
    pub sha256: String,
    #[serde(default)]
    pub deprecated: Option<String>,
}

/// What a registry offers the resolver and installer. Tests implement it with fakes.
pub trait Source {
    fn versions(&self, org: &str, name: &str) -> Result<Vec<VersionInfo>>;
    fn download(&self, org: &str, name: &str, version: &str) -> Result<Vec<u8>>;
}

/// Checks the registry URL policy: `https`, or `http` only for loopback hosts unless
/// `allow_insecure`. Returns the normalised URL.
pub fn check_url(raw: &str, allow_insecure: bool) -> Result<String> {
    let norm = normalize_url(raw);
    let url = Url::parse(&norm).map_err(|_| RegistryError::BadUrl(norm.clone()))?;
    if !url.username().is_empty() || url.password().is_some() || url.host().is_none() {
        return Err(RegistryError::BadUrl(norm));
    }
    match url.scheme() {
        "https" => Ok(norm),
        "http" if allow_insecure || is_loopback(&url) => Ok(norm),
        "http" => Err(RegistryError::Insecure(norm)),
        _ => Err(RegistryError::BadUrl(norm)),
    }
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// A registry connection.
pub struct Client {
    base: String,
    token: Option<Token>,
    agent: ureq::Agent,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("authenticated", &self.token.is_some())
            .finish()
    }
}

impl Client {
    pub fn new(url: &str, token: Option<Token>, allow_insecure: bool) -> Result<Self> {
        let base = check_url(url, allow_insecure)?;
        // No redirects: a redirect could carry the token to another host.
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .user_agent("veld")
            .build();
        Ok(Self { base, token, agent })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    fn request(&self, method: &str, path: &str) -> ureq::Request {
        let req = self.agent.request(method, &format!("{}{path}", self.base));
        match &self.token {
            Some(t) => req.set("Authorization", &format!("Bearer {}", t.expose())),
            None => req,
        }
    }

    fn call(&self, req: ureq::Request, body: Option<(&str, &[u8])>) -> Result<ureq::Response> {
        let result = match body {
            Some((content_type, bytes)) => req.set("Content-Type", content_type).send_bytes(bytes),
            None => req.call(),
        };
        result.map_err(|e| match e {
            ureq::Error::Status(status, resp) => RegistryError::Http {
                status,
                message: error_message(resp),
            },
            ureq::Error::Transport(t) => RegistryError::Network(t.to_string()),
        })
    }

    /// `GET /api/v1/auth/me`: the account behind the token (its `username`).
    pub fn me(&self) -> Result<String> {
        let resp = self.call(self.request("GET", "/api/v1/auth/me"), None)?;
        let body = read_limited(resp, 1 << 20)?;
        let json: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|e| RegistryError::Other(format!("unexpected response: {e}")))?;
        let user = json.get("user").unwrap_or(&json);
        Ok(user
            .get("username")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_string())
    }

    /// `POST /api/v1/packages`: uploads one packed version.
    pub fn publish(&self, org: &str, name: &str, version: &str, tarball: &[u8]) -> Result<()> {
        let boundary = "veld-boundary-7f3a9c1e5b2d4f60";
        let manifest = serde_json::json!({ "org": org, "name": name, "version": version });
        let mut body = Vec::new();
        body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"manifest\"\r\n\r\n{manifest}\r\n").into_bytes());
        body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"tarball\"; filename=\"{name}-{version}.tar.gz\"\r\nContent-Type: application/gzip\r\n\r\n").into_bytes());
        body.extend_from_slice(tarball);
        body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
        let ct = format!("multipart/form-data; boundary={boundary}");
        self.call(self.request("POST", "/api/v1/packages"), Some((&ct, &body)))
            .map(drop)
    }
}

impl Source for Client {
    fn versions(&self, org: &str, name: &str) -> Result<Vec<VersionInfo>> {
        let path = format!("/api/v1/packages/{org}/{name}/versions");
        let resp = self.call(self.request("GET", &path), None)?;
        let body = read_limited(resp, 8 << 20)?;
        serde_json::from_slice(&body)
            .map_err(|e| RegistryError::Other(format!("unexpected versions response: {e}")))
    }

    fn download(&self, org: &str, name: &str, version: &str) -> Result<Vec<u8>> {
        let path = format!("/api/v1/packages/{org}/{name}/{version}/download");
        let resp = self.call(self.request("GET", &path), None)?;
        read_limited(resp, MAX_DOWNLOAD)
    }
}

fn read_limited(resp: ureq::Response, max: u64) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    resp.into_reader()
        .take(max + 1)
        .read_to_end(&mut out)
        .map_err(|e| RegistryError::Network(e.to_string()))?;
    if out.len() as u64 > max {
        return Err(RegistryError::Other(format!(
            "response is larger than {max} bytes"
        )));
    }
    Ok(out)
}

fn error_message(resp: ureq::Response) -> String {
    let mut buf = Vec::new();
    let _ = resp.into_reader().take(64 << 10).read_to_end(&mut buf);
    serde_json::from_slice::<serde_json::Value>(&buf)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .unwrap_or_else(|| {
            String::from_utf8_lossy(&buf)
                .trim()
                .chars()
                .take(300)
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_is_required_except_for_loopback() {
        assert!(check_url("https://registry.example.com/", false).is_ok());
        assert_eq!(
            check_url("https://registry.example.com/", false).unwrap(),
            "https://registry.example.com"
        );
        for ok in [
            "http://localhost:8080",
            "http://127.0.0.1:9",
            "http://[::1]:80",
            "http://LOCALHOST",
        ] {
            assert!(check_url(ok, false).is_ok(), "{ok}");
        }
        assert!(matches!(
            check_url("http://registry.example.com", false),
            Err(RegistryError::Insecure(_))
        ));
        assert!(check_url("http://registry.example.com", true).is_ok());
        for bad in [
            "ftp://x",
            "not a url",
            "https://user:pw@host",
            "file:///etc",
        ] {
            assert!(check_url(bad, true).is_err(), "{bad}");
        }
        assert!(check_url("http://localhost.evil.com", false).is_err());
    }

    #[test]
    fn debug_hides_the_token() {
        let c = Client::new("https://r.example", Some(Token::new("vtk_secret")), false).unwrap();
        assert!(!format!("{c:?}").contains("secret"));
    }
}

//! What `veld init` builds: the answers (from flags or prompts) and their parsing.

use anyhow::Result;

use crate::error::user;

/// The server of one service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Server {
    /// `node` with a framework: express, fastify, hono, nest or node-http.
    Node(String),
    /// `java` with a framework: jdk or spring.
    Java(String),
    /// `rust` with a framework: axum.
    Rust(String),
    None,
}

/// The client target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Client {
    /// `typescript`, with an optional framework (`react`, `vue`).
    Ts(Option<String>),
    Java,
    /// `rust` with a transport: reqwest or ureq.
    Rust(String),
    None,
}

pub const NODE_FRAMEWORKS: [&str; 5] = ["express", "fastify", "hono", "nest", "node-http"];
pub const JAVA_FRAMEWORKS: [&str; 2] = ["jdk", "spring"];
pub const RUST_SERVER_FRAMEWORKS: [&str; 1] = ["axum"];
pub const RUST_CLIENT_FRAMEWORKS: [&str; 2] = ["reqwest", "ureq"];
pub const TS_FRAMEWORKS: [&str; 2] = ["react", "vue"];

impl Server {
    /// Parses `node`, `node:fastify`, `java`, `java:spring`, `rust` or `none`.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.trim().to_ascii_lowercase();
        let (kind, fw) = match text.split_once(':') {
            Some((k, f)) => (k, Some(f)),
            None => (text.as_str(), None),
        };
        let pick = |fw: Option<&str>, list: &[&str], default: &str| -> Result<String> {
            match fw {
                None => Ok(default.to_string()),
                Some(f) if list.contains(&f) => Ok(f.to_string()),
                Some(f) => Err(user(format!(
                    "unknown framework `{f}` (valid: {})",
                    list.join(", ")
                ))),
            }
        };
        match kind {
            "node" => Ok(Self::Node(pick(fw, &NODE_FRAMEWORKS, "express")?)),
            "java" => Ok(Self::Java(pick(fw, &JAVA_FRAMEWORKS, "jdk")?)),
            "rust" => Ok(Self::Rust(pick(fw, &RUST_SERVER_FRAMEWORKS, "axum")?)),
            "none" if fw.is_none() => Ok(Self::None),
            other => Err(user(format!(
                "unknown server `{other}` (valid: node[:{}], java[:{}], rust[:{}], none)",
                NODE_FRAMEWORKS.join("|"),
                JAVA_FRAMEWORKS.join("|"),
                RUST_SERVER_FRAMEWORKS.join("|")
            ))),
        }
    }
}

impl Client {
    /// Parses `typescript`, `typescript:react`, `typescript:vue`, `java`, `rust`, `rust:ureq` or `none`.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.trim().to_ascii_lowercase();
        let (kind, fw) = match text.split_once(':') {
            Some((k, f)) => (k, Some(f)),
            None => (text.as_str(), None),
        };
        match (kind, fw) {
            ("typescript" | "ts", None | Some("none" | "")) => Ok(Self::Ts(None)),
            ("typescript" | "ts", Some(f)) if TS_FRAMEWORKS.contains(&f) => {
                Ok(Self::Ts(Some(f.to_string())))
            }
            ("typescript" | "ts", Some(f)) => Err(user(format!(
                "unknown client framework `{f}` (valid: {}, none)",
                TS_FRAMEWORKS.join(", ")
            ))),
            ("java", None) => Ok(Self::Java),
            ("rust", None) => Ok(Self::Rust("reqwest".to_string())),
            ("rust", Some(f)) if RUST_CLIENT_FRAMEWORKS.contains(&f) => {
                Ok(Self::Rust(f.to_string()))
            }
            ("rust", Some(f)) => Err(user(format!(
                "unknown client framework `{f}` (valid: {})",
                RUST_CLIENT_FRAMEWORKS.join(", ")
            ))),
            ("none", None) => Ok(Self::None),
            _ => Err(user(format!(
                "unknown client `{text}` (valid: typescript[:{}], java, rust[:{}], none)",
                TS_FRAMEWORKS.join("|"),
                RUST_CLIENT_FRAMEWORKS.join("|")
            ))),
        }
    }
}

impl Client {
    /// Parses every `--client` value; `none` adds nothing and a kind may appear only once.
    pub fn parse_list(specs: &[String]) -> Result<Vec<Self>> {
        let mut out: Vec<Self> = Vec::new();
        for spec in specs {
            let c = Self::parse(spec)?;
            if c == Self::None {
                continue;
            }
            if out
                .iter()
                .any(|o| std::mem::discriminant(o) == std::mem::discriminant(&c))
            {
                return Err(user(format!("client `{spec}` is given twice")));
            }
            out.push(c);
        }
        Ok(out)
    }
}

/// Everything `init` needs to write the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answers {
    /// The starter contracts of a single-service project.
    pub template: crate::cli::Template,
    pub name: String,
    /// Empty for a single-service project (the one service is `name`).
    pub services: Vec<String>,
    /// `(service, server)`; a single-service project has one entry for `name`.
    pub servers: Vec<(String, Server)>,
    /// The client targets; empty for none.
    pub clients: Vec<Client>,
    /// `(service, base URL)` of every service of a workspace; empty for a single service.
    pub base_urls: Vec<(String, String)>,
    /// `(service, consumed services)` of a workspace. Nothing is consumed unless the user says so.
    pub consumes: Vec<(String, Vec<String>)>,
    /// Output folder overrides by target name.
    pub outs: Vec<(String, String)>,
    /// One line for `[project].description`; empty for none.
    pub description: String,
}

impl Answers {
    #[must_use]
    pub fn is_workspace(&self) -> bool {
        !self.services.is_empty()
    }

    /// The base URL of `service`.
    #[must_use]
    pub fn base_url(&self, service: &str) -> String {
        self.base_urls
            .iter()
            .find(|(s, _)| s == service)
            .map_or_else(
                || default_base_url(self.services.iter().position(|s| s == service).unwrap_or(0)),
                |(_, u)| u.clone(),
            )
    }

    /// The services `service` consumes.
    #[must_use]
    pub fn consumes_of(&self, service: &str) -> Vec<String> {
        self.consumes
            .iter()
            .find(|(s, _)| s == service)
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }

    /// The output folder of `target`: the override, else `default`.
    #[must_use]
    pub fn out(&self, target: &str, default: &str) -> String {
        self.outs
            .iter()
            .find(|(t, _)| t == target)
            .map_or_else(|| default.to_string(), |(_, o)| o.clone())
    }
}

/// The suggested base URL of the `index`-th service of a workspace.
#[must_use]
pub fn default_base_url(index: usize) -> String {
    format!("http://localhost:{}", 3001 + index)
}

/// A base URL: `http://` or `https://`, then a host.
pub fn check_base_url(url: &str) -> Result<String> {
    let u = url.trim();
    let host = u
        .strip_prefix("http://")
        .or_else(|| u.strip_prefix("https://"));
    match host {
        Some(h) if !h.is_empty() && !u.contains('"') && !u.contains(char::is_whitespace) => {
            Ok(u.trim_end_matches('/').to_string())
        }
        _ => Err(user(format!(
            "`{u}` is not a base URL (use http://host:port or https://host)"
        ))),
    }
}

/// Parses `--base-url service=URL` values; services without one get `default_base_url`.
pub fn assign_base_urls(services: &[String], specs: &[String]) -> Result<Vec<(String, String)>> {
    let mut urls: Vec<(String, String)> = services
        .iter()
        .enumerate()
        .map(|(i, s)| (s.clone(), default_base_url(i)))
        .collect();
    for spec in specs {
        let (svc, url) = spec
            .split_once('=')
            .ok_or_else(|| user(format!("`--base-url {spec}`: use SERVICE=URL")))?;
        let slot = urls
            .iter_mut()
            .find(|(s, _)| s == svc.trim())
            .ok_or_else(|| user(format!("`--base-url {spec}`: no service `{svc}`")))?;
        slot.1 = check_base_url(url)?;
    }
    Ok(urls)
}

/// Parses `--consumes service=a,b` values. Nothing is assigned for a service that has none.
pub fn assign_consumes(
    services: &[String],
    specs: &[String],
) -> Result<Vec<(String, Vec<String>)>> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for spec in specs {
        let (svc, list) = spec
            .split_once('=')
            .ok_or_else(|| user(format!("`--consumes {spec}`: use SERVICE=a,b")))?;
        let svc = svc.trim();
        if !services.iter().any(|s| s == svc) {
            return Err(user(format!("`--consumes {spec}`: no service `{svc}`")));
        }
        let mut wanted: Vec<String> = Vec::new();
        for raw in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if raw == svc {
                return Err(user(format!(
                    "`--consumes {spec}`: a service cannot consume itself"
                )));
            }
            if !services.iter().any(|s| s == raw) {
                return Err(user(format!("`--consumes {spec}`: no service `{raw}`")));
            }
            if !wanted.iter().any(|w| w == raw) {
                wanted.push(raw.to_string());
            }
        }
        out.retain(|(s, _)| s != svc);
        out.push((svc.to_string(), wanted));
    }
    Ok(out)
}

/// Letters, digits and `_` only (the name becomes a package name).
#[must_use]
pub fn sanitize(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    s.trim_matches('_').to_string()
}

/// `order_items` becomes `OrderItems`.
#[must_use]
pub fn pascal(name: &str) -> String {
    name.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// Splits `a, b,c` into sanitised, distinct, non-empty service names.
pub fn parse_services(text: &str) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split(',') {
        let name = sanitize(raw.trim()).to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        if name.starts_with(|c: char| c.is_ascii_digit()) {
            return Err(user(format!(
                "service name `{name}` must not start with a digit"
            )));
        }
        if out.contains(&name) {
            return Err(user(format!("service `{name}` is listed twice")));
        }
        out.push(name);
    }
    if out.is_empty() {
        return Err(user("a workspace needs at least one service name"));
    }
    Ok(out)
}

/// Parses `--server` values: `kind[:framework]` for every service, or `service=kind[:framework]`.
pub fn assign_servers(services: &[String], specs: &[String]) -> Result<Vec<(String, Server)>> {
    let mut servers: Vec<(String, Server)> = services
        .iter()
        .map(|s| (s.clone(), Server::Node("express".to_string())))
        .collect();
    for spec in specs {
        match spec.split_once('=') {
            Some((svc, kind)) => {
                let slot = servers
                    .iter_mut()
                    .find(|(s, _)| s == svc.trim())
                    .ok_or_else(|| user(format!("`--server {spec}`: no service `{svc}`")))?;
                slot.1 = Server::parse(kind)?;
            }
            None => {
                let kind = Server::parse(spec)?;
                servers.iter_mut().for_each(|(_, s)| *s = kind.clone());
            }
        }
    }
    Ok(servers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_parse_with_defaults_and_errors() {
        assert_eq!(
            Server::parse("node").unwrap(),
            Server::Node("express".into())
        );
        assert_eq!(
            Server::parse("Node:Fastify").unwrap(),
            Server::Node("fastify".into())
        );
        assert_eq!(
            Server::parse("java:spring").unwrap(),
            Server::Java("spring".into())
        );
        assert_eq!(Server::parse("java").unwrap(), Server::Java("jdk".into()));
        assert_eq!(Server::parse("none").unwrap(), Server::None);
        for bad in ["go", "node:koa", "java:play", "none:x", ""] {
            assert!(Server::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn clients_parse() {
        assert_eq!(Client::parse("typescript").unwrap(), Client::Ts(None));
        assert_eq!(
            Client::parse("typescript:react").unwrap(),
            Client::Ts(Some("react".into()))
        );
        assert_eq!(Client::parse("typescript:none").unwrap(), Client::Ts(None));
        assert_eq!(Client::parse("java").unwrap(), Client::Java);
        assert_eq!(Client::parse("none").unwrap(), Client::None);
        for bad in ["python", "typescript:svelte", "java:x"] {
            assert!(Client::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn client_lists_combine_and_reject_duplicates() {
        let l = |v: &[&str]| {
            Client::parse_list(&v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
        };
        assert_eq!(
            l(&["typescript:vue", "java"]).unwrap(),
            [Client::Ts(Some("vue".into())), Client::Java]
        );
        assert!(l(&["none"]).unwrap().is_empty());
        assert!(l(&["typescript", "ts:react"]).is_err());
    }

    #[test]
    fn service_lists_are_cleaned() {
        assert_eq!(
            parse_services("Users, order-items ,").unwrap(),
            ["users", "order_items"]
        );
        assert!(parse_services(" , ").is_err());
        assert!(parse_services("a,a").is_err());
        assert!(parse_services("1a").is_err());
        assert_eq!(pascal("order_items"), "OrderItems");
    }

    #[test]
    fn server_specs_apply_to_all_or_one_service() {
        let svcs = vec!["a".to_string(), "b".to_string()];
        let s = assign_servers(&svcs, &["java:spring".into(), "b=node:hono".into()]).unwrap();
        assert_eq!(s[0].1, Server::Java("spring".into()));
        assert_eq!(s[1].1, Server::Node("hono".into()));
        assert!(assign_servers(&svcs, &["c=node".into()]).is_err());
    }

    #[test]
    fn base_urls_default_per_service_and_flags_override() {
        let svcs = vec!["a".to_string(), "b".to_string()];
        let u = assign_base_urls(&svcs, &["b=https://b.example.com/".into()]).unwrap();
        assert_eq!(u[0].1, "http://localhost:3001");
        assert_eq!(u[1].1, "https://b.example.com");
        for bad in ["c=http://x", "a", "a=ftp://x", "a=http://"] {
            assert!(assign_base_urls(&svcs, &[bad.into()]).is_err(), "{bad}");
        }
    }

    #[test]
    fn consumes_are_only_what_was_asked_for() {
        let svcs = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert!(assign_consumes(&svcs, &[]).unwrap().is_empty());
        let c = assign_consumes(&svcs, &["a=b,c".into(), "c=a".into()]).unwrap();
        assert_eq!(
            c[0],
            ("a".to_string(), vec!["b".to_string(), "c".to_string()])
        );
        assert_eq!(c[1], ("c".to_string(), vec!["a".to_string()]));
        for bad in ["a=a", "a=z", "z=a", "a"] {
            assert!(assign_consumes(&svcs, &[bad.into()]).is_err(), "{bad}");
        }
    }
}

//! Looks at the folder `veld init` runs in, so the questions start from what is already there:
//! a `package.json` pre-selects the Node server and TypeScript client (and the framework it
//! lists as a dependency), a `pom.xml` or Gradle build pre-selects the Java server.

use std::path::Path;

use super::answers::{Client, Server};

/// What the folder already contains. Everything is `None` for an empty folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detected {
    pub server: Option<Server>,
    pub client: Option<Client>,
}

/// Inspects `dir` (only the top level; nothing is read recursively).
#[must_use]
pub fn detect(dir: &Path) -> Detected {
    let package_json = std::fs::read_to_string(dir.join("package.json")).ok();
    let java_build = ["pom.xml", "build.gradle", "build.gradle.kts"]
        .iter()
        .find_map(|f| std::fs::read_to_string(dir.join(f)).ok());
    let has = |text: &str, dep: &str| text.contains(&format!("\"{dep}\""));
    let node_server = package_json.as_deref().and_then(|t| {
        [
            ("express", "express"),
            ("fastify", "fastify"),
            ("hono", "hono"),
            ("@nestjs/core", "nest"),
        ]
        .iter()
        .find(|(dep, _)| has(t, dep))
        .map(|(_, fw)| Server::Node((*fw).to_string()))
    });
    let java_server = java_build.map(|t| {
        Server::Java(
            if t.contains("spring-boot") {
                "spring"
            } else {
                "jdk"
            }
            .to_string(),
        )
    });
    let client = package_json.as_deref().map(|t| {
        let fw = [("react", "react"), ("vue", "vue")]
            .iter()
            .find(|(dep, _)| has(t, dep))
            .map(|(_, fw)| (*fw).to_string());
        Client::Ts(fw)
    });
    Detected {
        server: node_server.or(java_server),
        client,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, text: &str) {
        std::fs::write(dir.join(name), text).unwrap();
    }

    #[test]
    fn an_empty_folder_detects_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(detect(tmp.path()), Detected::default());
    }

    #[test]
    fn package_json_preselects_node_server_and_ts_client() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            tmp.path(),
            "package.json",
            r#"{"dependencies":{"fastify":"^4","react":"^18"}}"#,
        );
        let d = detect(tmp.path());
        assert_eq!(d.server, Some(Server::Node("fastify".into())));
        assert_eq!(d.client, Some(Client::Ts(Some("react".into()))));
    }

    #[test]
    fn a_bare_package_json_selects_only_the_client() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "package.json", "{}");
        let d = detect(tmp.path());
        assert_eq!(d.server, None);
        assert_eq!(d.client, Some(Client::Ts(None)));
    }

    #[test]
    fn pom_xml_preselects_java_and_spring_boot_selects_spring() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "pom.xml", "<project/>");
        assert_eq!(detect(tmp.path()).server, Some(Server::Java("jdk".into())));
        write(
            tmp.path(),
            "pom.xml",
            "<artifactId>spring-boot-starter-web</artifactId>",
        );
        assert_eq!(
            detect(tmp.path()).server,
            Some(Server::Java("spring".into()))
        );
    }
}

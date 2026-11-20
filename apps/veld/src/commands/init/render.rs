//! Renders the files `veld init` writes from its [`Answers`].

use super::answers::{Answers, Client, Server, pascal, sanitize};

/// One file to write. An `optional` file is skipped (not an error) when it already exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub path: String,
    pub text: String,
    pub optional: bool,
}

/// The app directory (relative to the project) holding an output folder: what comes before
/// its first `src` or `generated` component, else its parent.
fn app_of(out: &str) -> String {
    let parts: Vec<&str> = out
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    match parts.iter().position(|p| matches!(*p, "src" | "generated")) {
        Some(i) => parts[..i].join("/"),
        None => parts[..parts.len().saturating_sub(1)].join("/"),
    }
}

/// Everything `init` writes, in a stable order.
#[must_use]
pub fn files(a: &Answers) -> Vec<File> {
    let (config, outs) = config(a);
    let mut files = vec![File {
        path: "veld.toml".to_string(),
        text: config,
        optional: false,
    }];
    files.extend(contracts(a));
    files.extend(package_jsons(a, &outs));
    files
}

/// The output folders of the TypeScript targets, which sit in apps with a `package.json`.
fn package_jsons(a: &Answers, ts_outs: &[String]) -> Vec<File> {
    let mut apps: Vec<String> = ts_outs.iter().map(|o| app_of(o)).collect();
    apps.sort();
    apps.dedup();
    apps.iter()
        .map(|app| {
            let name = if app.is_empty() {
                sanitize(&a.name)
            } else {
                format!("{}-{}", sanitize(&a.name), sanitize(&app.replace('/', "_")))
            }
            .to_ascii_lowercase()
            .replace('_', "-");
            File {
                path: if app.is_empty() {
                    "package.json".to_string()
                } else {
                    format!("{app}/package.json")
                },
                text: format!(
                    "{{\n  \"name\": \"{name}\",\n  \"version\": \"0.0.0\",\n  \"private\": true\n}}\n"
                ),
                optional: true,
            }
        })
        .collect()
}

/// `veld.toml` text and the output folders of its TypeScript targets.
fn config(a: &Answers) -> (String, Vec<String>) {
    let mut t = String::new();
    let mut ts_outs = Vec::new();
    t.push_str(&format!("[project]\nname = \"{}\"\n", a.name));
    if a.is_workspace() {
        t.push('\n');
        for (i, svc) in a.services.iter().enumerate() {
            t.push_str(&format!(
                "[services.{svc}]\ncontracts = \"contracts/{svc}\"\nbase-url = \"http://localhost:{}\"\n",
                3001 + i
            ));
            if i > 0 {
                t.push_str(&format!(
                    "consumes = [\"{}\"]   # {svc} gets a generated client for {}\n",
                    a.services[0], a.services[0]
                ));
            }
            t.push('\n');
        }
    } else {
        t.push_str("contracts = \"contracts\"\n\n");
    }
    for (svc, server) in &a.servers {
        server_target(a, &mut t, &mut ts_outs, svc, server);
    }
    client_target(a, &mut t, &mut ts_outs);
    (t, ts_outs)
}

fn server_target(a: &Answers, t: &mut String, ts_outs: &mut Vec<String>, svc: &str, s: &Server) {
    let (name, flat) = if a.is_workspace() {
        (format!("{svc}-api"), format!("services/{svc}"))
    } else {
        ("api".to_string(), String::new())
    };
    let (emitter, fw, default_out) = match s {
        Server::None => return,
        Server::Node(fw) => (
            "node",
            fw,
            if a.is_workspace() {
                format!("{flat}/generated")
            } else {
                "generated/server".to_string()
            },
        ),
        Server::Java(fw) => (
            "java",
            fw,
            if a.is_workspace() {
                format!("{flat}/src/main/java")
            } else {
                "java/src/main/java".to_string()
            },
        ),
    };
    let out = a.out(&name, &default_out);
    t.push_str(&format!(
        "[[targets]]\nname = \"{name}\"\nrole = \"server\"\nemitter = \"{emitter}\"\nframework = \"{fw}\"\nservice = \"{svc}\"\nout = \"{out}\"\n"
    ));
    if emitter == "java" {
        t.push_str(&format!(
            "\n[targets.options]\npackage = \"com.example.{}\"\n",
            sanitize(svc).to_ascii_lowercase()
        ));
    } else {
        ts_outs.push(out);
    }
    t.push('\n');
}

fn client_target(a: &Answers, t: &mut String, ts_outs: &mut Vec<String>) {
    match &a.client {
        Client::None => {}
        Client::Ts(fw) => {
            let default = if a.is_workspace() {
                "web/generated"
            } else {
                "generated/client"
            };
            let out = a.out("web", default);
            t.push_str(
                "[[targets]]\nname = \"web\"\nrole = \"client\"\nemitter = \"typescript\"\n",
            );
            if let Some(fw) = fw {
                t.push_str(&format!("framework = \"{fw}\"\n"));
            }
            t.push_str(&format!("out = \"{out}\"\n\n"));
            ts_outs.push(out);
        }
        Client::Java => {
            let default = if a.is_workspace() {
                "clients/java/src/main/java"
            } else {
                "java-client/src/main/java"
            };
            let out = a.out("java-client", default);
            t.push_str(&format!(
                "[[targets]]\nname = \"java-client\"\nrole = \"client\"\nemitter = \"java\"\nout = \"{out}\"\n\n[targets.options]\npackage = \"com.example.{}.client\"\n\n",
                sanitize(&a.name).to_ascii_lowercase()
            ));
        }
    }
}

fn contracts(a: &Answers) -> Vec<File> {
    let file = |path: String, text: String| File {
        path,
        text,
        optional: false,
    };
    if !a.is_workspace() {
        return vec![file("contracts/hello.veld".to_string(), HELLO.to_string())];
    }
    a.services
        .iter()
        .map(|svc| {
            let text = match svc.as_str() {
                "users" => USERS.to_string(),
                "orders" => ORDERS.to_string(),
                other => generic(other),
            };
            file(format!("contracts/{svc}/{svc}.veld"), text)
        })
        .collect()
}

fn generic(svc: &str) -> String {
    let p = pascal(svc);
    format!(
        "/// A {svc} record.\nmodel {p}Item {{\n  id: uuid\n  name: string\n}}\n\n/// The {svc} service.\nservice {p} at \"/{svc}\" {{\n  /// Fetch one record.\n  get \"/{{id}}\" get{p}Item(id: uuid) -> {p}Item | NotFound\n}}\n"
    )
}

const HELLO: &str = r#"/// A greeting for one person.
model Greeting {
  message: string
}

/// Says hello.
service Hello at "/hello" {
  /// Greets the person with the given name.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}
"#;

const USERS: &str = r#"/// A registered user.
model User {
  id: uuid
  name: string
}

/// User accounts.
service Users at "/users" {
  /// Fetch one user.
  get "/{id}" getUser(id: uuid) -> User | NotFound
}
"#;

const ORDERS: &str = r#"/// An order placed by a user.
model Order {
  id: uuid
  userId: uuid
}

/// Order management.
service Orders at "/orders" {
  /// Fetch one order.
  get "/{id}" getOrder(id: uuid) -> Order | NotFound
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use veld_project::config::load_str;

    fn answers(workspace: &[&str], server: Server, client: Client) -> Answers {
        let services: Vec<String> = workspace.iter().map(|s| (*s).to_string()).collect();
        let servers = if services.is_empty() {
            vec![("shop".to_string(), server)]
        } else {
            services
                .iter()
                .map(|s| (s.clone(), server.clone()))
                .collect()
        };
        Answers {
            name: "shop".to_string(),
            services,
            servers,
            client,
            outs: Vec::new(),
        }
    }

    fn valid(a: &Answers) -> veld_project::config::Project {
        let dir = std::env::temp_dir().join("veld-init-render");
        let f = files(a);
        load_str(&dir, &f[0].text).unwrap_or_else(|e| panic!("{e}\n{}", f[0].text))
    }

    #[test]
    fn every_combination_renders_a_valid_config() {
        let servers = [
            Server::Node("express".into()),
            Server::Node("nest".into()),
            Server::Java("jdk".into()),
            Server::Java("spring".into()),
            Server::None,
        ];
        let clients = [
            Client::Ts(None),
            Client::Ts(Some("react".into())),
            Client::Java,
            Client::None,
        ];
        for s in &servers {
            for c in &clients {
                for ws in [&[][..], &["users", "orders"], &["billing"]] {
                    let a = answers(ws, s.clone(), c.clone());
                    let p = valid(&a);
                    let n = usize::from(*s != Server::None) * a.servers.len()
                        + usize::from(*c != Client::None);
                    assert_eq!(p.targets.len(), n, "{s:?} {c:?} {ws:?}");
                }
            }
        }
    }

    #[test]
    fn the_default_single_project_is_what_it_always_was() {
        let a = answers(&[], Server::Node("express".into()), Client::Ts(None));
        let f = files(&a);
        assert_eq!(f[0].path, "veld.toml");
        assert!(
            f[0].text.contains("out = \"generated/server\"")
                && f[0].text.contains("out = \"generated/client\"")
        );
        assert_eq!(f[1].path, "contracts/hello.veld");
        assert_eq!(f[2].path, "package.json");
        assert!(f[2].optional);
    }

    #[test]
    fn workspace_services_consume_the_first_and_get_their_own_apps() {
        let a = answers(
            &["users", "orders", "billing"],
            Server::Node("express".into()),
            Client::Ts(Some("react".into())),
        );
        let f = files(&a);
        assert!(f[0].text.matches("consumes = [\"users\"]").count() == 2);
        assert!(f[0].text.contains("framework = \"react\""));
        let paths: Vec<&str> = f.iter().map(|x| x.path.as_str()).collect();
        for want in [
            "contracts/billing/billing.veld",
            "services/billing/package.json",
            "web/package.json",
        ] {
            assert!(paths.contains(&want), "{want} in {paths:?}");
        }
    }

    #[test]
    fn output_overrides_apply_and_set_the_app_dir() {
        let mut a = answers(&[], Server::Node("express".into()), Client::Ts(None));
        a.outs.push(("api".into(), "server/src/generated".into()));
        let f = files(&a);
        assert!(f[0].text.contains("out = \"server/src/generated\""));
        assert!(f.iter().any(|x| x.path == "server/package.json"));
    }

    #[test]
    fn app_dirs() {
        assert_eq!(app_of("generated/server"), "");
        assert_eq!(app_of("web/out"), "web");
        assert_eq!(app_of("api/src/generated"), "api");
        assert_eq!(app_of("src/generated"), "");
        assert_eq!(app_of("services/users/generated"), "services/users");
    }
}

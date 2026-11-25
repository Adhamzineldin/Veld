//! Renders the files `veld init` writes from its [`Answers`].

use super::answers::{Answers, pascal, sanitize};
use super::config;
use crate::cli::Template;

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
    let outs: Vec<String> = config::planned(a)
        .into_iter()
        .filter(config::Planned::is_npm)
        .map(|t| t.out)
        .collect();
    let mut files = config::config_files(a);
    files.extend(contracts(a));
    files.extend(package_jsons(a, &outs));
    files.push(readme(a));
    files
}

/// `(target name, output folder)` of every target the answers render, overrides applied.
#[must_use]
pub fn target_outs(a: &Answers) -> Vec<(String, String)> {
    config::planned(a)
        .into_iter()
        .map(|t| (t.name, t.out))
        .collect()
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

fn contracts(a: &Answers) -> Vec<File> {
    let file = |path: String, text: String| File {
        path,
        text,
        optional: false,
    };
    if !a.is_workspace() {
        return match a.template {
            Template::Hello => vec![file("contracts/hello.veld".to_string(), HELLO.to_string())],
            Template::Crud => crud_contracts(),
        };
    }
    a.services
        .iter()
        .map(|svc| {
            let text = match svc.as_str() {
                "users" => USERS.to_string(),
                "orders" => ORDERS.to_string(),
                other => generic(other),
            };
            file(format!("services/{svc}/contracts/{svc}.veld"), text)
        })
        .collect()
}

fn generic(svc: &str) -> String {
    let p = pascal(svc);
    format!(
        "/// A {svc} record.\nmodel {p}Item {{\n  id: uuid\n  name: string\n}}\n\n/// The {svc} service.\n@auth(none)\nservice {p} at \"/{svc}\" {{\n  /// Fetch one record.\n  get \"/{{id}}\" get{p}Item(id: uuid) -> {p}Item | NotFound\n}}\n"
    )
}

const HELLO: &str = r#"/// A greeting for one person.
model Greeting {
  message: string
}

/// Says hello.
@auth(none)
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
@auth(none)
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
@auth(none)
service Orders at "/orders" {
  /// Fetch one order.
  get "/{id}" getOrder(id: uuid) -> Order | NotFound
}
"#;

/// The project `README.md`: what the folders are and the commands to use. Deterministic, and
/// `optional`, so an existing README is never replaced.
fn readme(a: &Answers) -> File {
    let mut t = format!("# {}\n\n", a.name);
    if !a.description.is_empty() {
        t.push_str(&format!("{}\n\n", a.description));
    }
    t.push_str("An API described with [Veld](https://github.com/Adhamzineldin/Veld) contracts. ");
    t.push_str("The `.veld` files are the source of truth; the servers and clients are generated from them.\n\n");
    t.push_str("## Layout\n\n");
    if a.is_workspace() {
        t.push_str("- `services/<service>/contracts/`: the contracts, one folder per service\n");
        t.push_str("- `veld.toml`: the workspace and its members\n");
        t.push_str("- `services/<service>/veld.toml`: one service, its base URL and its server target\n");
    } else {
        t.push_str("- `contracts/`: the API contracts (`.veld` files)\n");
        t.push_str("- `veld.toml`: the service and its generated targets\n");
    }
    for (name, out) in target_outs(a) {
        t.push_str(&format!(
            "- `{}`: generated output of `{name}` (do not edit; `veld clean` removes it)\n",
            a.out(&name, &out)
        ));
    }
    t.push_str("\n## Commands\n\n```bash\nveld check            # compile the contracts and lint them\nveld generate         # write the generated code\nveld generate --check # exit 1 if the generated code is out of date (CI)\nveld setup            # make the generated packages importable\nveld doctor           # check the project and the tools it needs\nveld export openapi   # OpenAPI 3.1 for the contracts\n```\n\n");
    t.push_str("Edit a contract, run `veld generate`, and commit both the contracts and the generated output.\n");
    File {
        path: "README.md".to_string(),
        text: t,
        optional: true,
    }
}

/// The `crud` starter: users, auth and pagination in three modules of one package.
fn crud_contracts() -> Vec<File> {
    [
        ("contracts/common.veld", COMMON),
        ("contracts/users.veld", CRUD_USERS),
        ("contracts/auth.veld", CRUD_AUTH),
    ]
    .iter()
    .map(|(path, text)| File {
        path: (*path).to_string(),
        text: (*text).to_string(),
        optional: false,
    })
    .collect()
}

const COMMON: &str = r#"/// Paging parameters shared by every list endpoint.
model PageQuery {
  /// 1-based page number.
  page?: int @min(1)
  /// Items per page.
  size?: int @min(1) @max(100)
}
"#;

const CRUD_USERS: &str = r#"use common.{PageQuery}

enum Role { admin, member }

/// A registered user.
model User {
  id: uuid @readonly
  email: string @format(email)
  name: string @length(min: 1, max: 80)
  role: Role = .member
  createdAt: datetime @readonly
}

/// Payload to create a user.
model CreateUserInput {
  email: string @format(email)
  name: string @length(min: 1, max: 80)
  role?: Role
}

/// Payload to change a user; absent fields stay as they are.
model UpdateUserInput {
  name?: string @length(min: 1, max: 80)
  role?: Role
}

/// A user with this email already exists.
error EmailTaken 409 {
  email: string
}

/// User management with pagination.
@auth(bearer)
service Users at "/users" {
  /// Lists users, one page at a time.
  get "/" listUsers(query: PageQuery) -> Page<User>

  /// Fetches one user.
  get "/{id}" getUser(id: uuid) -> User | NotFound

  /// Creates a user.
  post "/" createUser(body: CreateUserInput) -> User | EmailTaken

  /// Changes a user.
  patch "/{id}" updateUser(id: uuid, body: UpdateUserInput) -> User | NotFound

  /// Deletes a user.
  delete "/{id}" deleteUser(id: uuid) -> 204 | NotFound
}
"#;

const CRUD_AUTH: &str = r#"use users.{EmailTaken, User}

/// Payload to sign up.
model RegisterInput {
  email: string @format(email)
  name: string @length(min: 1, max: 80)
  password: string @length(min: 8, max: 128)
}

/// Payload to sign in.
model LoginInput {
  email: string @format(email)
  password: string
}

/// A bearer token to send in the `Authorization` header.
model Session {
  accessToken: string
  /// Seconds until the token expires.
  expiresIn: int
}

/// The email or password is wrong.
error InvalidCredentials 401 {
  message?: string
}

/// Sign up, sign in and the current user.
service Auth at "/auth" {
  /// Creates an account.
  @auth(none)
  post "/register" register(body: RegisterInput) -> User | EmailTaken

  /// Exchanges credentials for a token.
  @auth(none)
  post "/login" login(body: LoginInput) -> 200 Session | InvalidCredentials

  /// The signed-in user.
  @auth(bearer)
  get "/me" me() -> User
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::answers::{Client, Server};
    use veld_project::config::load_virtual;

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
            template: crate::cli::Template::Hello,
            name: "shop".to_string(),
            services,
            servers,
            clients: match client {
                Client::None => Vec::new(),
                c @ (Client::Ts(_) | Client::Java | Client::Rust(_)) => vec![c],
            },
            base_urls: Vec::new(),
            consumes: Vec::new(),
            outs: Vec::new(),
            description: String::new(),
        }
    }

    fn valid(a: &Answers) -> veld_project::config::Project {
        let dir = std::env::temp_dir().join("veld-init-render");
        let tomls: Vec<(String, String)> = files(a)
            .into_iter()
            .filter(|f| f.path.ends_with("veld.toml"))
            .map(|f| (f.path, f.text))
            .collect();
        load_virtual(&dir, &tomls).unwrap_or_else(|e| panic!("{e}\n{tomls:#?}"))
    }

    fn text_of(files: &[File], path: &str) -> String {
        files
            .iter()
            .find(|f| f.path == path)
            .map(|f| f.text.clone())
            .unwrap_or_else(|| panic!("{path} not written"))
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
    fn workspace_services_consume_only_what_was_chosen_and_get_their_own_apps() {
        let mut a = answers(
            &["users", "orders", "billing"],
            Server::Node("express".into()),
            Client::Ts(Some("react".into())),
        );
        let none = files(&a);
        assert!(
            none.iter().all(|f| !f.text.contains("consumes")),
            "no auto-assigned consumes"
        );
        assert!(
            text_of(&none, "services/orders/veld.toml")
                .contains("base-url = \"http://localhost:3002\"")
        );
        a.consumes = vec![
            ("orders".to_string(), vec!["users".to_string()]),
            (
                "billing".to_string(),
                vec!["users".to_string(), "orders".to_string()],
            ),
        ];
        a.base_urls = vec![("users".to_string(), "https://users.example.com".to_string())];
        let f = files(&a);
        assert!(text_of(&f, "services/orders/veld.toml").contains("consumes = [\"users\"]"));
        assert!(
            text_of(&f, "services/billing/veld.toml").contains("consumes = [\"users\", \"orders\"]")
        );
        assert!(
            text_of(&f, "services/users/veld.toml")
                .contains("base-url = \"https://users.example.com\"")
        );
        assert!(text_of(&f, "web/veld.toml").contains("framework = \"react\""));
        assert!(text_of(&f, "veld.toml").contains("members = [\"services/*\", \"web\"]"));
        let p = valid(&a);
        assert_eq!(p.package("billing").unwrap().consumes, ["users", "orders"]);
        let paths: Vec<&str> = f.iter().map(|x| x.path.as_str()).collect();
        for want in [
            "services/billing/contracts/billing.veld",
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
        let mut ws = answers(&["users"], Server::Node("express".into()), Client::None);
        ws.outs.push(("users-api".into(), "backend/users/gen".into()));
        let f = files(&ws);
        assert!(text_of(&f, "services/users/veld.toml").contains("out = \"../../backend/users/gen\""));
        assert_eq!(valid(&ws).targets[0].out.ends_with("backend/users/gen"), true);
    }

    #[test]
    fn description_is_escaped_and_two_clients_render() {
        let mut a = answers(&[], Server::Node("express".into()), Client::Ts(None));
        a.description = "Say \"hi\" \\ now".into();
        a.clients.push(Client::Java);
        let p = valid(&a);
        assert_eq!(p.targets.len(), 3);
        assert!(
            files(&a)[0]
                .text
                .contains(r#"description = "Say \"hi\" \\ now""#)
        );
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

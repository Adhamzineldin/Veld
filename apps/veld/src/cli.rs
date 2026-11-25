//! The clap definitions. Help texts are deliberately short.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "veld",
    version,
    about = "Contract-first API code generator",
    long_about = "Write .veld contracts once; generate typed servers and clients.\n\
                  Veld only ever writes inside a target's `out` directory and never touches your own files."
)]
pub struct Cli {
    /// Path to veld.toml (or a directory containing it). Default: search upward from here
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Print only errors and warnings
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Never colour output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Show phases, write-plan entries and timings (repeat for more detail)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create veld.toml and an example contract
    Init(InitArgs),
    /// Compile all contracts, lint them and report diagnostics
    Check(CheckArgs),
    /// Generate code for every target (or one, by name)
    Generate(GenerateArgs),
    /// Same as `generate --watch`
    #[command(hide = true)]
    Watch(WatchArgs),
    /// Remove generated files that you have not modified
    Clean(CleanArgs),
    /// Wire generated targets into your apps (package.json, pom.xml, build.gradle)
    Setup(SetupArgs),
    /// Check the project, its tools and its environment, and say how to fix problems
    Doctor,

    /// Format .veld files canonically
    Fmt(FmtArgs),
    /// Show breaking and non-breaking contract changes
    Diff(DiffArgs),
    /// Run the `example` blocks of the contracts, offline or against a live test handler
    Test(TestArgs),
    /// Export OpenAPI or the compiled IR
    #[command(subcommand)]
    Export(ExportCommand),
    /// Create and test emitter plugins
    #[command(subcommand)]
    Plugin(PluginCommand),

    /// Internal commands for debugging the compiler
    #[command(hide = true, subcommand)]
    Debug(DebugCommand),
    /// Language server
    #[command(hide = true)]
    Lsp,
    /// Print a shell completion script
    #[command(hide = true)]
    Completions {
        /// The shell to generate for
        shell: clap_complete::Shell,
    },

    /// Add a registry package to veld.toml, resolve it and pin it in veld.lock
    Add(AddArgs),
    /// Install exactly the packages pinned in veld.lock
    Install(InstallArgs),
    /// Pack this project's library or service and upload it to the registry
    Publish(PublishArgs),
    /// Store a registry API token
    Login(LoginArgs),
    /// Forget the stored registry token
    Logout(LogoutArgs),
    /// Look things up in the registry: info, search, versions, token
    #[command(subcommand)]
    Registry(RegistryCommand),
}

/// `veld registry`: the read-only and account commands that have no natural home elsewhere
/// (login, logout, add, install and publish stay top-level because they change a project).
#[derive(Debug, Subcommand)]
pub enum RegistryCommand {
    /// Show the registry URL in use and the account behind the stored token
    Info(RegistryOpts),
    /// Search packages by name or description
    Search {
        /// Text to search for (default: list everything visible)
        query: Option<String>,
        #[command(flatten)]
        opts: RegistryOpts,
    },
    /// List the published versions of a package
    Versions {
        /// The package, as @org/name
        package: String,
        #[command(flatten)]
        opts: RegistryOpts,
    },
    /// Manage API tokens
    #[command(subcommand)]
    Token(TokenCommand),
}

#[derive(Debug, Subcommand)]
pub enum TokenCommand {
    /// Create an API token (it is printed once)
    Create {
        /// A label for the token
        #[arg(long)]
        name: String,
        /// Scopes, comma-separated: read, write, delete
        #[arg(long, value_delimiter = ',', default_value = "read")]
        scopes: Vec<String>,
        /// Days until it expires (default: never)
        #[arg(long, value_name = "DAYS")]
        expires_days: Option<u32>,
        #[command(flatten)]
        opts: RegistryOpts,
    },
}

#[derive(Debug, Args, Default)]
pub struct RegistryOpts {
    /// Registry URL (default: veld.toml `[registry]`, then $VELD_REGISTRY, then the last login)
    #[arg(long, value_name = "URL")]
    pub registry: Option<String>,
    /// Allow a plain-http registry that is not on localhost
    #[arg(long)]
    pub allow_insecure: bool,
}

/// `veld check`.
#[derive(Debug, Args, Default)]
pub struct CheckArgs {
    /// Report only diagnostics in these .veld files or directories (the project still compiles whole)
    pub paths: Vec<PathBuf>,
    /// Skip the lint rules
    #[arg(long)]
    pub no_lint: bool,
    /// Lint warnings fail the check too
    #[arg(long)]
    pub strict: bool,
    /// Print the service dependency graph instead of the summary
    #[arg(long, value_name = "FORMAT", num_args = 0..=1, default_missing_value = "text", value_enum)]
    pub graph: Option<GraphFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum GraphFormat {
    Text,
    Dot,
    Json,
}

/// `veld init --template`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Template {
    /// One tiny service with a `hello` endpoint
    Hello,
    /// Users CRUD, auth (login, token) and a reusable `Page` type
    Crud,
}
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Project name (default: the directory name)
    #[arg(long)]
    pub name: Option<String>,
    /// Scaffold a microservices workspace; optionally the comma-separated service names
    #[arg(long, value_name = "SERVICES", num_args = 0..=1, default_missing_value = "users,orders")]
    pub workspace: Option<String>,
    /// Server target: node[:express|fastify|hono|nest|node-http], java[:jdk|spring], rust[:axum] or none.
    /// Prefix with `service=` to set one service of a workspace
    #[arg(long, value_name = "KIND[:FRAMEWORK]")]
    pub server: Vec<String>,
    /// Client targets (repeat or comma-separate): typescript[:react|vue], java, rust[:reqwest|ureq] or none
    #[arg(long, value_name = "KIND[:FRAMEWORK]", value_delimiter = ',')]
    pub client: Vec<String>,
    /// Base URL of a workspace service, as SERVICE=URL (repeatable; default http://localhost:300N)
    #[arg(long, value_name = "SERVICE=URL")]
    pub base_url: Vec<String>,
    /// Services a workspace service consumes, as SERVICE=a,b (repeatable; nothing is consumed by default)
    #[arg(long, value_name = "SERVICE=A,B")]
    pub consumes: Vec<String>,
    /// Change the output folder of a target, as TARGET=DIR (repeatable)
    #[arg(long, value_name = "TARGET=DIR")]
    pub out: Vec<String>,
    /// A one-line project description, written to veld.toml
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// Use the defaults for everything not given as a flag; never prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
    /// List the files that would be written, and write nothing
    #[arg(long)]
    pub dry_run: bool,
    /// Starter contracts: `hello` (default) or `crud` (users, auth and pagination)
    #[arg(long, value_name = "NAME", value_enum)]
    pub template: Option<Template>,
}

impl InitArgs {
    /// No flag at all: the interactive flow applies on a terminal.
    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.name.is_none()
            && self.workspace.is_none()
            && self.server.is_empty()
            && self.client.is_empty()
            && self.out.is_empty()
            && self.base_url.is_empty()
            && self.consumes.is_empty()
            && self.description.is_none()
            && !self.yes
            && !self.dry_run
    }
}

#[derive(Debug, Args, Default)]
pub struct GenerateArgs {
    /// Generate only the target with this name
    pub target: Option<String>,
    /// Show what would change without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Write nothing; list what is stale and exit 1 if any target would change (for CI)
    #[arg(long, conflicts_with_all = ["dry_run", "force", "watch"])]
    pub check: bool,
    /// With --check or --dry-run, print unified diffs of the changes
    #[arg(long)]
    pub diff: bool,
    /// Overwrite existing files that Veld did not create
    #[arg(long)]
    pub force: bool,
    /// Regenerate whenever a .veld file or veld.toml changes
    #[arg(long)]
    pub watch: bool,
    /// Fail on breaking changes instead of prompting (for CI)
    #[arg(long, conflicts_with = "accept_breaking")]
    pub strict: bool,
    /// Continue past breaking changes without asking
    #[arg(long)]
    pub accept_breaking: bool,
    /// Run `veld setup` after generating
    #[arg(long)]
    pub setup: bool,
}

impl GenerateArgs {
    /// Whether this run writes nothing.
    #[must_use]
    pub fn read_only(&self) -> bool {
        self.dry_run || self.check
    }
}
/// `veld watch`.
#[derive(Debug, Args)]
pub struct WatchArgs {
    /// Watch and regenerate only the target with this name
    pub target: Option<String>,
    /// Overwrite existing files that Veld did not create
    #[arg(long)]
    pub force: bool,
    /// Show what would change on each run without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Quiet period in milliseconds after the last change before regenerating
    #[arg(long, default_value_t = 300, value_name = "MS")]
    pub debounce: u64,
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    /// Wire only the target with this name
    pub target: Option<String>,
    /// Show the planned edits without writing anything
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct CleanArgs {
    /// Also remove veld.lock.ir.json, the breaking-change baseline
    #[arg(long)]
    pub lock: bool,
    /// List what would be removed without removing it
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum DebugCommand {
    /// Print the compiled IR as JSON
    Ir,
    /// Print the syntax tree of one file
    Ast {
        /// A .veld file
        file: PathBuf,
    },
}

#[derive(Debug, Args)]
pub struct FmtArgs {
    /// Files or directories to format (default: every contract of the project)
    pub paths: Vec<PathBuf>,
    /// Change nothing; list the files that would change and exit 1 if there are any
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Compare against the contracts at this git ref (read with `git show`, nothing is checked out)
    #[arg(long, value_name = "REF", conflicts_with = "lock")]
    pub against: Option<String>,
    /// Compare against veld.lock.ir.json (the default)
    #[arg(long)]
    pub lock: bool,
    /// Exit 1 if any change is breaking
    #[arg(long)]
    pub strict: bool,
}

/// `veld test`.
#[derive(Debug, Args, Default)]
pub struct TestArgs {
    /// Run only the examples written in these .veld files or directories
    pub paths: Vec<PathBuf>,
    /// Run only the examples whose name or `Service.endpoint` contains this text
    #[arg(long, value_name = "TEXT")]
    pub filter: Option<String>,
    /// Send the examples to a running test handler at this base URL instead of running them offline
    #[arg(long, value_name = "BASE_URL")]
    pub live: Option<String>,
    /// With --live: an example without `returns` stops before the application's handler
    #[arg(long, requires = "live")]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum ExportCommand {
    /// OpenAPI 3.1 JSON
    Openapi(DocOut),
    /// The compiled IR as JSON, for plugin authors
    Ir {
        /// Write to this file instead of stdout
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
}

/// Exports that produce one document.
#[derive(Debug, Args, Default)]
pub struct DocOut {
    /// Write to this file instead of stdout
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Export only this service (repeatable; default: all)
    #[arg(long)]
    pub service: Vec<String>,
    /// Overwrite an existing, different FILE
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Subcommand)]
pub enum PluginCommand {
    /// Scaffold a standalone plugin project in ./<name>
    New {
        /// Plugin name, for example `kotlin`
        name: String,
        /// Implementation language
        #[arg(long, default_value = "ts")]
        lang: String,
    },
    /// Run the protocol conformance suite against a plugin command
    Test {
        /// The plugin executable and any leading arguments, for example `node dist/main.js`
        #[arg(required = true, num_args = 1.., trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
}

/// `veld login`.
#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Registry URL (default: veld.toml `[registry]`, then $VELD_REGISTRY, then the last login)
    #[arg(long, value_name = "URL")]
    pub registry: Option<String>,
    /// API token (vtk_...); read from stdin when omitted
    #[arg(long, value_name = "TOKEN")]
    pub token: Option<String>,
    /// Allow a plain-http registry that is not on localhost
    #[arg(long)]
    pub allow_insecure: bool,
}

#[derive(Debug, Args)]
pub struct LogoutArgs {
    /// Registry URL to forget the token for (default: as for `login`)
    #[arg(long, value_name = "URL")]
    pub registry: Option<String>,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// The package, as @org/name or @org/name@range (for example @acme/billing@^1.2)
    pub package: String,
    /// Registry URL (default: veld.toml `[registry]`, then $VELD_REGISTRY, then the last login)
    #[arg(long, value_name = "URL")]
    pub registry: Option<String>,
    /// Allow a plain-http registry that is not on localhost
    #[arg(long)]
    pub allow_insecure: bool,
}

#[derive(Debug, Args)]
pub struct PublishArgs {
    /// Pack and check the package, but upload nothing
    #[arg(long)]
    pub dry_run: bool,
    /// The library or service to publish (required in a workspace with several)
    #[arg(long, value_name = "NAME")]
    pub package: Option<String>,
    /// Registry URL (default: veld.toml `[registry]`, then $VELD_REGISTRY, then the last login)
    #[arg(long, value_name = "URL")]
    pub registry: Option<String>,
    /// Allow a plain-http registry that is not on localhost
    #[arg(long)]
    pub allow_insecure: bool,
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    /// Allow a plain-http registry that is not on localhost
    #[arg(long)]
    pub allow_insecure: bool,
}
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn reserved_commands_are_hidden_from_help() {
        let help = Cli::command().render_help().to_string();
        for hidden in ["debug", "lsp", "watch", "completions"] {
            assert!(
                !help.lines().any(|l| l.trim_start().starts_with(hidden)),
                "`{hidden}` is visible:\n{help}"
            );
        }
        for shown in [
            "init", "check", "generate", "setup", "clean", "doctor", "fmt", "diff", "export",
            "plugin", "add", "install", "publish", "login", "logout", "registry",
        ] {
            assert!(
                help.lines().any(|l| l.trim_start().starts_with(shown)),
                "`{shown}` is missing:\n{help}"
            );
        }
    }

    fn ok(args: &[&str]) {
        let mut v = vec!["veld"];
        v.extend_from_slice(args);
        assert!(Cli::try_parse_from(&v).is_ok(), "{args:?} should parse");
    }

    fn bad(args: &[&str]) {
        let mut v = vec!["veld"];
        v.extend_from_slice(args);
        assert!(
            Cli::try_parse_from(&v).is_err(),
            "{args:?} should not parse"
        );
    }

    #[test]
    fn registry_commands_parse() {
        ok(&["publish", "--dry-run", "--package", "lib"]);
        ok(&["login", "--registry", "https://r", "--token", "t"]);
        ok(&["add", "@acme/billing@^1.2"]);
        bad(&["add"]);
        ok(&["install"]);
        ok(&["logout"]);
        ok(&["registry", "info"]);
        ok(&["registry", "search", "billing"]);
        ok(&["registry", "versions", "@acme/billing"]);
        ok(&[
            "registry",
            "token",
            "create",
            "--name",
            "ci",
            "--scopes",
            "read,write",
        ]);
        bad(&["registry", "token", "create"]);
    }

    #[test]
    fn tool_commands_parse() {
        ok(&["fmt", "--check", "x.veld"]);
        ok(&["diff", "--against", "main", "--strict"]);
        bad(&["diff", "--against", "main", "--lock"]);
        ok(&[
            "export",
            "openapi",
            "-o",
            "a.json",
            "--service",
            "s",
            "--service",
            "t",
        ]);
        bad(&["export", "nope"]);
        ok(&["export", "ir"]);
        ok(&["plugin", "new", "kotlin"]);
        ok(&["plugin", "test", "node", "dist/main.js"]);
    }

    #[test]
    fn generate_flags_parse_and_conflict() {
        ok(&["generate", "--check", "--diff"]);
        bad(&["generate", "--check", "--dry-run"]);
        bad(&["generate", "--check", "--force"]);
        bad(&["generate", "--strict", "--accept-breaking"]);
        ok(&["generate", "--setup", "--accept-breaking"]);
        bad(&[
            "generate",
            "--emitter",
            "node",
            "--role",
            "server",
            "--out",
            "d",
        ]);
        bad(&["generate", "--input", "a.veld"]);
        ok(&["watch", "api"]);
        ok(&["generate", "--watch"]);
    }

    #[test]
    fn check_clean_doctor_and_global_flags_parse() {
        ok(&["check", "a.veld", "contracts/b"]);
        ok(&["check", "--no-lint", "--strict"]);
        ok(&["check", "--graph"]);
        ok(&["check", "--graph", "dot"]);
        ok(&["check", "--graph=json", "x.veld"]);
        bad(&["check", "--graph", "yaml"]);
        ok(&["clean", "--lock", "--dry-run"]);
        ok(&["doctor"]);
        ok(&["completions", "zsh"]);
        bad(&["completions", "tcsh"]);
        let cli = Cli::try_parse_from(["veld", "-vv", "check"]).unwrap();
        assert_eq!(cli.verbose, 2);
        let cli = Cli::try_parse_from(["veld", "check", "-v"]).unwrap();
        assert_eq!(cli.verbose, 1);
        ok(&["init", "--template", "crud"]);
        ok(&[
            "init",
            "--workspace",
            "a,b",
            "--base-url",
            "a=http://h:1",
            "--consumes",
            "b=a",
        ]);
        bad(&["init", "--template", "nope"]);
    }
}

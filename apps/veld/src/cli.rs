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

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create veld.toml and an example contract
    Init(InitArgs),
    /// Compile all contracts and report diagnostics
    Check,
    /// Generate code for every target (or one, by name)
    Generate(GenerateArgs),
    /// Regenerate whenever a contract, veld.toml or veld.lock changes (same as `generate --watch`)
    Watch(WatchArgs),
    /// Remove generated files that you have not modified
    Clean(CleanArgs),
    /// Wire generated targets into your apps (package.json, pom.xml, build.gradle)
    Setup(SetupArgs),

    /// Format .veld files canonically
    Fmt(FmtArgs),
    /// Show breaking and non-breaking contract changes
    Diff(DiffArgs),
    /// Export the contracts as OpenAPI or as IR JSON
    #[command(subcommand)]
    Export(ExportCommand),
    /// Convert a v1 project (veld.config.json and .veld files) to v2
    Migrate(MigrateArgs),
    /// Create and test emitter plugins
    #[command(subcommand)]
    Plugin(PluginCommand),

    /// Internal commands for debugging the compiler
    #[command(hide = true, subcommand)]
    Debug(DebugCommand),
    /// Language server (not yet implemented)
    #[command(hide = true)]
    Lsp,

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
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Project name (default: the directory name)
    #[arg(long)]
    pub name: Option<String>,
    /// Scaffold a microservices workspace; optionally the comma-separated service names
    #[arg(long, value_name = "SERVICES", num_args = 0..=1, default_missing_value = "users,orders")]
    pub workspace: Option<String>,
    /// Server target: node[:express|fastify|hono|nest|node-http], java[:jdk|spring] or none.
    /// Prefix with `service=` to set one service of a workspace
    #[arg(long, value_name = "KIND[:FRAMEWORK]")]
    pub server: Vec<String>,
    /// Client targets (repeat or comma-separate): typescript[:react|vue], java or none
    #[arg(long, value_name = "KIND[:FRAMEWORK]", value_delimiter = ',')]
    pub client: Vec<String>,
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
            && self.description.is_none()
            && !self.yes
            && !self.dry_run
    }
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
    /// Generate only the target with this name
    pub target: Option<String>,
    /// Show what would change without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Overwrite existing files that Veld did not create
    #[arg(long)]
    pub force: bool,
    /// Regenerate whenever a .veld file or veld.toml changes
    #[arg(long)]
    pub watch: bool,
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

#[derive(Debug, Subcommand)]
pub enum ExportCommand {
    /// OpenAPI 3.1 JSON
    Openapi {
        /// Write to this file instead of stdout
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Export only this service
        #[arg(long)]
        service: Option<String>,
    },
    /// The compiled IR as JSON, for plugin authors
    Ir {
        /// Write to this file instead of stdout
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
pub struct MigrateArgs {
    /// Directory holding the v1 veld.config.json (default: ./ or ./veld)
    #[arg(long, value_name = "DIR")]
    pub from: Option<PathBuf>,
    /// Directory for the v2 contracts, relative to the v1 config directory
    #[arg(long, default_value = "contracts", value_name = "DIR")]
    pub out: String,
    /// Print what would be written without writing it
    #[arg(long)]
    pub dry_run: bool,
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
        for hidden in ["debug", "lsp"] {
            assert!(
                !help.lines().any(|l| l.trim_start().starts_with(hidden)),
                "`{hidden}` is visible:\n{help}"
            );
        }
        for shown in [
            "init", "check", "generate", "watch", "setup", "clean", "fmt", "diff", "export",
            "migrate", "plugin", "add", "install", "publish", "login", "logout",
        ] {
            assert!(help.contains(shown));
        }
    }

    #[test]
    fn registry_commands_parse() {
        assert!(Cli::try_parse_from(["veld", "publish", "--dry-run", "--package", "lib"]).is_ok());
        assert!(
            Cli::try_parse_from(["veld", "login", "--registry", "https://r", "--token", "t"])
                .is_ok()
        );
        assert!(Cli::try_parse_from(["veld", "add", "@acme/billing@^1.2"]).is_ok());
        assert!(Cli::try_parse_from(["veld", "add"]).is_err());
        assert!(Cli::try_parse_from(["veld", "install"]).is_ok());
        assert!(Cli::try_parse_from(["veld", "logout"]).is_ok());
    }

    #[test]
    fn new_commands_parse() {
        assert!(Cli::try_parse_from(["veld", "fmt", "--check", "x.veld"]).is_ok());
        assert!(Cli::try_parse_from(["veld", "diff", "--against", "main", "--strict"]).is_ok());
        assert!(Cli::try_parse_from(["veld", "diff", "--against", "main", "--lock"]).is_err());
        assert!(
            Cli::try_parse_from([
                "veld",
                "export",
                "openapi",
                "-o",
                "a.json",
                "--service",
                "s"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["veld", "export", "ir"]).is_ok());
        assert!(Cli::try_parse_from(["veld", "migrate", "--from", "d", "--dry-run"]).is_ok());
        assert!(Cli::try_parse_from(["veld", "plugin", "new", "kotlin"]).is_ok());
        assert!(Cli::try_parse_from(["veld", "plugin", "test", "node", "dist/main.js"]).is_ok());
    }
}

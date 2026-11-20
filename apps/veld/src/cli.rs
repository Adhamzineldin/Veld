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
    /// Remove generated files that you have not modified
    Clean(CleanArgs),

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

    #[command(hide = true)]
    Add(Reserved),
    #[command(hide = true)]
    Publish(Reserved),
    #[command(hide = true)]
    Login(Reserved),
    #[command(hide = true)]
    Logout(Reserved),
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Project name (default: the directory name)
    #[arg(long)]
    pub name: Option<String>,
    /// Scaffold a two-service workspace instead of a single service
    #[arg(long)]
    pub workspace: bool,
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

/// A command that is reserved but not built yet; it accepts and ignores any arguments.
#[derive(Debug, Args)]
pub struct Reserved {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub args: Vec<String>,
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
        for hidden in ["publish", "login", "logout", "add", "debug", "lsp"] {
            assert!(
                !help.lines().any(|l| l.trim_start().starts_with(hidden)),
                "`{hidden}` is visible:\n{help}"
            );
        }
        for shown in [
            "init", "check", "generate", "clean", "fmt", "diff", "export", "migrate", "plugin",
        ] {
            assert!(help.contains(shown));
        }
    }

    #[test]
    fn reserved_commands_swallow_arguments() {
        assert!(Cli::try_parse_from(["veld", "publish", "--check", "x"]).is_ok());
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

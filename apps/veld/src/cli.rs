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

    /// Internal commands for debugging the compiler
    #[command(hide = true, subcommand)]
    Debug(DebugCommand),
    /// Language server (not yet implemented)
    #[command(hide = true)]
    Lsp,

    #[command(hide = true)]
    Fmt(Reserved),
    #[command(hide = true)]
    Diff(Reserved),
    #[command(hide = true)]
    Export(Reserved),
    #[command(hide = true)]
    Migrate(Reserved),
    #[command(hide = true)]
    Add(Reserved),
    #[command(hide = true)]
    Publish(Reserved),
    #[command(hide = true)]
    Login(Reserved),
    #[command(hide = true)]
    Logout(Reserved),
    #[command(hide = true)]
    Plugin(Reserved),
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
        for hidden in ["fmt", "diff", "export", "migrate", "publish", "login", "plugin", "debug", "lsp"] {
            assert!(!help.lines().any(|l| l.trim_start().starts_with(hidden)), "`{hidden}` is visible:\n{help}");
        }
        for shown in ["init", "check", "generate", "clean"] {
            assert!(help.contains(shown));
        }
    }

    #[test]
    fn reserved_commands_swallow_arguments() {
        assert!(Cli::try_parse_from(["veld", "fmt", "--check", "x.veld"]).is_ok());
    }
}

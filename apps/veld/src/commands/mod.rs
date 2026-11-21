//! One module per command. `run` dispatches the parsed command line.

mod check;
mod clean;
mod debug;
mod diff;
mod export;
mod fmt;
mod generate;
mod init;
mod lsp;
mod migrate;
mod plugin;
mod registry;
mod setup;
mod watch;

use anyhow::Result;

use crate::cli::{Cli, Command};
use crate::error::Exit;
use crate::ui::Ui;

pub fn run(cli: &Cli, ui: &Ui) -> Result<Exit> {
    let config = cli.config.as_deref();
    match &cli.command {
        Command::Init(args) => init::run(ui, args),
        Command::Check => check::run(ui, config),
        Command::Watch(args) => watch::run(ui, config, &watch::Options::from(args)),
        Command::Generate(args) if args.watch => {
            watch::run(ui, config, &watch::Options::from(args))
        }
        Command::Generate(args) => generate::run(ui, config, args),
        Command::Clean(args) => clean::run(ui, config, args),
        Command::Setup(args) => setup::run(ui, config, args),
        Command::Debug(cmd) => debug::run(ui, config, cmd),
        Command::Lsp => lsp::run(),
        Command::Fmt(args) => fmt::run(ui, config, args),
        Command::Diff(args) => diff::run(ui, config, args),
        Command::Export(cmd) => export::run(ui, config, cmd),
        Command::Migrate(args) => migrate::run(ui, args),
        Command::Plugin(cmd) => plugin::run(ui, cmd),
        Command::Add(_)
        | Command::Install(_)
        | Command::Publish(_)
        | Command::Login(_)
        | Command::Logout(_) => {
            registry::run(ui, config, &cli.command).unwrap_or_else(|| Ok(Exit::Internal))
        }
    }
}

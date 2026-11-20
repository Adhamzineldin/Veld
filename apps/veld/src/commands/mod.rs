//! One module per command. `run` dispatches the parsed command line.

mod check;
mod clean;
mod debug;
mod diff;
mod export;
mod fmt;
mod generate;
mod init;
mod migrate;
mod plugin;
mod stubs;
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
        Command::Generate(args) if args.watch => watch::run(ui, config, args),
        Command::Generate(args) => generate::run(ui, config, args),
        Command::Clean(args) => clean::run(ui, config, args),
        Command::Debug(cmd) => debug::run(ui, config, cmd),
        Command::Lsp => Ok(stubs::not_yet(ui, "lsp", "the language server")),
        Command::Fmt(args) => fmt::run(ui, config, args),
        Command::Diff(args) => diff::run(ui, config, args),
        Command::Export(cmd) => export::run(ui, config, cmd),
        Command::Migrate(args) => migrate::run(ui, args),
        Command::Add(_) => Ok(stubs::coming_soon(ui, "add")),
        Command::Publish(_) => Ok(stubs::coming_soon(ui, "publish")),
        Command::Login(_) => Ok(stubs::coming_soon(ui, "login")),
        Command::Logout(_) => Ok(stubs::coming_soon(ui, "logout")),
        Command::Plugin(cmd) => plugin::run(ui, cmd),
    }
}

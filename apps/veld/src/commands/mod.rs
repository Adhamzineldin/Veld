//! One module per command. `run` dispatches the parsed command line.

mod check;
mod clean;
mod debug;
mod diff;
mod doctor;
mod export;
mod fmt;
mod generate;
mod hooks;
mod init;
mod lsp;
mod plugin;
mod registry;
mod setup;
mod test;
mod test_fuzz;
mod watch;

use anyhow::Result;
use clap::CommandFactory;

use crate::cli::{Cli, Command};
use crate::error::Exit;
use crate::ui::Ui;

pub fn run(cli: &Cli, ui: &Ui) -> Result<Exit> {
    let config = cli.config.as_deref();
    match &cli.command {
        Command::Init(args) => init::run(ui, args),
        Command::Check(args) => check::run(ui, config, args),
        Command::Watch(args) => watch::run(ui, config, &watch::Options::from(args)),
        Command::Generate(args) if args.watch => {
            watch::run(ui, config, &watch::Options::from(args))
        }
        Command::Generate(args) => generate::run(ui, config, args),
        Command::Clean(args) => clean::run(ui, config, args),
        Command::Setup(args) => setup::run(ui, config, args),
        Command::Doctor => doctor::run(ui, config),
        Command::Debug(cmd) => debug::run(ui, config, cmd),
        Command::Lsp => lsp::run(),
        Command::Completions { shell } => {
            let mut script = Vec::new();
            clap_complete::generate(*shell, &mut Cli::command(), "veld", &mut script);
            // A closed pipe (`veld completions bash | head`) is not an error.
            let _ = std::io::Write::write_all(&mut std::io::stdout(), &script);
            Ok(Exit::Ok)
        }
        Command::Fmt(args) => fmt::run(ui, config, args),
        Command::Diff(args) => diff::run(ui, config, args),
        Command::Test(args) => test::run(ui, config, args),
        Command::Export(cmd) => export::run(ui, config, cmd),
        Command::Plugin(cmd) => plugin::run(ui, cmd),
        Command::Add(_)
        | Command::Install(_)
        | Command::Publish(_)
        | Command::Login(_)
        | Command::Logout(_)
        | Command::Registry(_) => {
            registry::run(ui, config, &cli.command).unwrap_or_else(|| Ok(Exit::Internal))
        }
    }
}

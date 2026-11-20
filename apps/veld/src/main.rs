//! The `veld` command line. This file only parses arguments and maps outcomes to exit codes;
//! every command lives in `commands/`, and the shared pipeline in `pipeline.rs`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod cli;
mod commands;
mod error;
mod pipeline;
mod registry;
mod ui;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    let ui = ui::Ui::new(cli.no_color, cli.quiet);
    match commands::run(&cli, &ui) {
        Ok(code) => code.into(),
        Err(err) => error::report(&ui, &err).into(),
    }
}

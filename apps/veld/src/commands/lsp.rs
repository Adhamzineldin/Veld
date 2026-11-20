//! `veld lsp`: the language server on stdin and stdout. Editors start it; it is hidden from
//! `--help`. Nothing else may write to stdout while it runs.

use anyhow::{Context, Result};

use crate::error::Exit;

pub fn run() -> Result<Exit> {
    veld_lsp::run_stdio().context("the language server failed")?;
    Ok(Exit::Ok)
}

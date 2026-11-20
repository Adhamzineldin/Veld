//! Developer tasks for the Veld repository: `cargo xtask <task>`.
//!
//! - `new-emitter <name> [--roles server,client]`: scaffold a generator crate
//! - `codegen [--check]`: regenerate files derived from the Rust sources

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod codegen;
mod edit;
mod new_emitter;
mod templates;

use std::process::ExitCode;

use anyhow::{Result, bail};

const USAGE: &str = "usage:\n  cargo xtask new-emitter <name> [--roles server,client]\n  cargo xtask codegen [--check]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<()> {
    let root = new_emitter::repo_root();
    match args.first().map(String::as_str) {
        Some("new-emitter") => new_emitter_cmd(&root, &args[1..]),
        Some("codegen") => codegen_cmd(&root, &args[1..]),
        _ => bail!("{USAGE}"),
    }
}

fn new_emitter_cmd(root: &std::path::Path, args: &[String]) -> Result<()> {
    let mut name = None;
    let mut roles = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--roles" => {
                roles = Some(
                    it.next()
                        .ok_or_else(|| anyhow::anyhow!("`--roles` needs a value"))?
                        .as_str(),
                )
            }
            a if a.starts_with("--roles=") => roles = a.strip_prefix("--roles="),
            a if a.starts_with('-') => bail!("unknown option `{a}`\n{USAGE}"),
            a if name.is_none() => name = Some(a),
            _ => bail!("{USAGE}"),
        }
    }
    let Some(name) = name else { bail!("{USAGE}") };
    let spec = new_emitter::NewEmitter::parse(name, roles)?;
    let created = new_emitter::run(root, &spec, true)?;
    for path in &created {
        println!("created {path}");
    }
    println!(
        "registered `{name}` in Cargo.toml, apps/veld/Cargo.toml and apps/veld/src/registry.rs"
    );
    println!("next: open crates/emitters/{name}/README.md and follow the checklist");
    Ok(())
}

fn codegen_cmd(root: &std::path::Path, args: &[String]) -> Result<()> {
    let check = match args {
        [] => false,
        [a] if a == "--check" => true,
        _ => bail!("{USAGE}"),
    };
    let changed = codegen::run(root, check)?;
    if check {
        println!("generated files are up to date");
    } else if changed.is_empty() {
        println!("nothing to do: generated files are up to date");
    } else {
        changed.iter().for_each(|p| println!("wrote {p}"));
    }
    Ok(())
}

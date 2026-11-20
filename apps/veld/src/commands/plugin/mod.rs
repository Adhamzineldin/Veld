//! `veld plugin new` and `veld plugin test`.

mod templates;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_plugin_host::{CommandLine, HostConfig, Runner, conformance};

use crate::cli::PluginCommand;
use crate::error::{Exit, user};
use crate::ui::Ui;

const SAMPLE: &str = r#"/// A greeting.
model Greeting {
  message: string
  tags?: string[]
}

/// Says hello.
service Hello at "/hello" {
  /// Greets.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}
"#;

pub fn run(ui: &Ui, cmd: &PluginCommand) -> Result<Exit> {
    match cmd {
        PluginCommand::New { name, lang } => new(ui, name, lang),
        PluginCommand::Test { command } => test(ui, command),
    }
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn new(ui: &Ui, name: &str, lang: &str) -> Result<Exit> {
    if lang != "ts" {
        return Err(user(format!(
            "unsupported plugin language `{lang}`; only `ts` is available"
        )));
    }
    if !valid_name(name) {
        return Err(user(format!(
            "`{name}` is not a valid plugin name: use lowercase letters, digits and `-`, starting with a letter"
        )));
    }
    let dir = Path::new(name);
    if dir.exists() {
        return Err(user(format!(
            "`{name}` already exists; choose another name or remove it"
        )));
    }
    let ir_types = veld_ir::schema::typescript();
    let files: [(&str, &str); 9] = [
        ("package.json", templates::PACKAGE_JSON),
        ("tsconfig.json", templates::TSCONFIG),
        (".gitignore", templates::GITIGNORE),
        ("README.md", templates::README_MD),
        ("veld.plugin.toml", templates::PLUGIN_TOML),
        ("src/protocol.ts", templates::PROTOCOL_TS),
        ("src/main.ts", templates::MAIN_TS),
        ("src/generate.ts", templates::GENERATE_TS),
        ("src/ir.d.ts", &ir_types),
    ];
    for (rel, text) in files {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create `{}`", parent.display()))?;
        }
        let body = if rel == "src/ir.d.ts" {
            text.to_string()
        } else {
            text.replace("{{name}}", name)
        };
        fs::write(&path, body).with_context(|| format!("cannot write `{}`", path.display()))?;
    }
    ui.info(&format!("created plugin `{name}` in ./{name}"));
    ui.info(&format!(
        "next: cd {name} && npm install && npm run build && npm test"
    ));
    Ok(Exit::Ok)
}

fn sample_ir() -> Result<veld_ir::Ir> {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "hello".into(),
            kind: veld_ir::PackageKind::Service,
            files: vec![SourceFile {
                path: "hello.veld".into(),
                text: SAMPLE.into(),
            }],
            base_url: None,
        }],
        consumes: std::collections::BTreeMap::new(),
    });
    let rendered = out.render();
    out.ir.ok_or_else(|| {
        anyhow::anyhow!("the built-in sample contract does not compile:\n{rendered}")
    })
}

fn test(ui: &Ui, command: &[String]) -> Result<Exit> {
    let Some((program, args)) = command.split_first() else {
        return Err(user(
            "give the plugin command, for example `veld plugin test node dist/main.js`".to_string(),
        ));
    };
    let root = std::env::current_dir().context("cannot read the current directory")?;
    let runner = Runner {
        name: "plugin-under-test".into(),
        command: CommandLine::from_parts(program.clone(), args.to_vec()),
        config: HostConfig::new(root),
    };
    let checks = conformance::run(&runner, &sample_ir()?);
    for check in &checks {
        match &check.result {
            Ok(()) => ui.info(&format!("  ok    {}", check.name)),
            Err(why) => ui.info(&format!("  FAIL  {}: {why}", check.name)),
        }
    }
    let failed = checks.iter().filter(|c| c.result.is_err()).count();
    if failed == 0 {
        ui.info(&format!("{} checks passed", checks.len()));
        Ok(Exit::Ok)
    } else {
        ui.error(&format!("{failed} of {} checks failed", checks.len()));
        Ok(Exit::User)
    }
}

//! `veld export runtime-schema [<target>] [-o FILE]` (`spec/runtime-config.md`, section 6).
//!
//! The model of a target's runtime config comes from `veld_codegen::runtime_config`, the same one
//! the generators use, so the schema lists the settings the generated type has.

use anyhow::Result;
use veld_codegen::runtime_config::RuntimeModel;
use veld_project::config::TargetSpec;

use crate::cli::{DocOut, RuntimeSchemaArgs};
use crate::error::{Exit, user};
use crate::pipeline::{self, Compiled};
use crate::selection;
use crate::ui::Ui;

pub fn run(ui: &Ui, done: &Compiled, args: &RuntimeSchemaArgs) -> Result<Exit> {
    let Some(chosen) = selection::resolve(ui, &done.project, &args.select)? else {
        return Ok(Exit::Ok);
    };
    let targets = pipeline::select_targets(&done.project, &chosen, args.target.as_deref())?;
    let model = pick(done, &targets, args.target.as_deref())?;
    let mut text = serde_json::to_string_pretty(&veld_tools::runtime_schema(&model))?;
    text.push('\n');
    let out = DocOut {
        output: args.output.clone(),
        service: Vec::new(),
        force: args.force,
    };
    super::export::document(ui, &out, "runtime-schema.json", &text)
}

/// The model of the named target, or of the one target that has a runtime config. `E0803` when
/// there is none or more than one.
fn pick(done: &Compiled, targets: &[&TargetSpec], named: Option<&str>) -> Result<RuntimeModel> {
    let mut models = Vec::new();
    for target in targets {
        let (role, services, consumes) = pipeline::target_scope(&done.project, &done.ir, target)
            .map_err(|lines| user(lines.join("\n")))?;
        if let Some(m) = RuntimeModel::new(&done.ir, &target.name, role, &services, &consumes) {
            models.push(m);
        }
    }
    if let Some(name) = named {
        return models.pop().ok_or_else(|| {
            user(format!(
                "E0803: target `{name}` has no runtime config: a client needs a service to call, and a server needs rules or a service in `consumes`"
            ))
        });
    }
    let names: Vec<&str> = models.iter().map(RuntimeModel::target).collect();
    match models.len() {
        1 => models.pop().ok_or_else(|| user("no runtime config")),
        0 => Err(user(
            "E0803: no target of the selected members has a runtime config",
        )),
        n => Err(user(format!(
            "E0803: {n} targets have a runtime config ({}); name one: `veld export runtime-schema <target>`",
            names.join(", ")
        ))),
    }
}

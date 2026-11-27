//! `veld test --coverage [--fail-under N]` (`spec/rules.md` section 19.1).

use veld_ir::{Example, Ir};
use veld_testing::coverage::measure;

use crate::error::Exit;
use crate::ui::Ui;
use crate::ui::render;

/// Prints the coverage report of `examples` for the packages `include` accepts. Exits 1 when
/// `fail_under` is given and the percentage is below it.
pub fn run(
    ui: &Ui,
    ir: &Ir,
    examples: &[&Example],
    include: &dyn Fn(&str) -> bool,
    fail_under: Option<u8>,
) -> Exit {
    let coverage = measure(ir, examples, include);
    for line in coverage.report() {
        ui.data(&line);
    }
    let Some(limit) = fail_under else {
        return Exit::Ok;
    };
    let percent = coverage.percent();
    if percent >= u32::from(limit) {
        return Exit::Ok;
    }
    let text = format!("coverage is {percent}%, below --fail-under {limit}");
    eprintln!("{}", render::fail(&ui.theme_err(), &text));
    Exit::User
}
